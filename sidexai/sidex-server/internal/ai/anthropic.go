package ai

import (
	"bufio"
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"
)

// Native Anthropic Messages API support.
//
// The rest of this package speaks OpenAI-compatible `/chat/completions`, which
// is what OpenRouter and every other provider here accepts. Anthropic's own API
// is a different shape — `/v1/messages`, content blocks instead of flat strings,
// its own SSE event names — so it needs a dedicated path rather than a base-URL
// swap.
//
// The only credential is a console API key from console.anthropic.com, sent as
// `x-api-key`. scmCode does not reuse another app's login or present itself as
// any other client.

const anthropicVersion = "2023-06-01"

// defaultAnthropicMaxTokens is used when the caller sets no ceiling. Anthropic
// requires max_tokens on every request, unlike the OpenAI-shaped API.
const defaultAnthropicMaxTokens = 8192

// IsAnthropicNative reports whether a provider must use the Messages API.
func IsAnthropicNative(provider string) bool {
	return provider == "anthropic"
}

// AnthropicModelID converts a catalog model id into the name Anthropic expects.
//
// The catalog uses OpenRouter-style ids (`anthropic/claude-opus-4.6`) because
// that is what every other provider here consumes. Anthropic spells the same
// model `claude-opus-4-6`.
func AnthropicModelID(modelID string) string {
	id := modelID
	if parts := strings.SplitN(id, "/", 2); len(parts) == 2 {
		id = parts[1]
	}
	// Anthropic uses dashes where the catalog uses dots for the version.
	return strings.ReplaceAll(id, ".", "-")
}

// ---------------------------------------------------------------------------
// Request shape
// ---------------------------------------------------------------------------

type anthropicBlock struct {
	Type string `json:"type"`
	Text string `json:"text,omitempty"`

	// tool_use
	ID    string          `json:"id,omitempty"`
	Name  string          `json:"name,omitempty"`
	Input json.RawMessage `json:"input,omitempty"`

	// tool_result
	ToolUseID string `json:"tool_use_id,omitempty"`
	Content   string `json:"content,omitempty"`
	IsError   bool   `json:"is_error,omitempty"`
}

type anthropicMessage struct {
	Role    string           `json:"role"`
	Content []anthropicBlock `json:"content"`
}

type anthropicTool struct {
	Name        string                 `json:"name"`
	Description string                 `json:"description,omitempty"`
	InputSchema map[string]interface{} `json:"input_schema"`
}

type anthropicThinking struct {
	Type         string `json:"type"`
	BudgetTokens int    `json:"budget_tokens,omitempty"`
}

type anthropicRequest struct {
	Model        string                 `json:"model"`
	MaxTokens    int                    `json:"max_tokens"`
	System       interface{}            `json:"system,omitempty"`
	Messages     []anthropicMessage     `json:"messages"`
	Tools        []anthropicTool        `json:"tools,omitempty"`
	Stream       bool                   `json:"stream"`
	Thinking     *anthropicThinking     `json:"thinking,omitempty"`
	OutputConfig *anthropicOutputConfig `json:"output_config,omitempty"`
}

// buildAnthropicMessages converts the internal history into Anthropic's
// content-block form.
//
// Two structural rules drive this: the system prompt is hoisted out of the
// message list into its own field, and a tool result is a `tool_result` block
// inside a *user* message rather than a role of its own.
func buildAnthropicMessages(messages []Message, systemPrompt string) ([]anthropicMessage, string) {
	system := systemPrompt
	out := make([]anthropicMessage, 0, len(messages))

	appendBlocks := func(role string, blocks []anthropicBlock) {
		if len(blocks) == 0 {
			return
		}
		// Anthropic rejects two consecutive messages with the same role, and
		// consecutive tool results are common, so merge them.
		if n := len(out); n > 0 && out[n-1].Role == role {
			out[n-1].Content = append(out[n-1].Content, blocks...)
			return
		}
		out = append(out, anthropicMessage{Role: role, Content: blocks})
	}

	for _, m := range messages {
		switch m.Role {
		case RoleSystem:
			// Fold stray system turns into the top-level prompt.
			if strings.TrimSpace(m.Content) != "" {
				if system != "" {
					system += "\n\n"
				}
				system += m.Content
			}

		case RoleUser:
			content := m.Content
			if peeled, rest := peelSystemInstruction(content); peeled != "" {
				if system != "" {
					system += "\n\n"
				}
				system += peeled
				content = rest
			}
			if strings.TrimSpace(content) != "" {
				appendBlocks("user", []anthropicBlock{{Type: "text", Text: content}})
			}

		case RoleAssistant:
			blocks := make([]anthropicBlock, 0, 1+len(m.ToolCalls))
			if strings.TrimSpace(m.Content) != "" {
				blocks = append(blocks, anthropicBlock{Type: "text", Text: m.Content})
			}
			for _, tc := range m.ToolCalls {
				input := json.RawMessage(tc.Function.Arguments)
				if len(input) == 0 || !json.Valid(input) {
					// A truncated or empty argument string would be rejected
					// outright; an empty object keeps the turn well-formed.
					input = json.RawMessage(`{}`)
				}
				blocks = append(blocks, anthropicBlock{
					Type:  "tool_use",
					ID:    tc.ID,
					Name:  tc.Function.Name,
					Input: input,
				})
			}
			appendBlocks("assistant", blocks)

		case RoleTool:
			appendBlocks("user", []anthropicBlock{{
				Type:      "tool_result",
				ToolUseID: m.ToolCallID,
				Content:   m.Content,
			}})
		}
	}

	// The conversation must open with a user turn.
	if len(out) > 0 && out[0].Role == "assistant" {
		out = append([]anthropicMessage{{
			Role:    "user",
			Content: []anthropicBlock{{Type: "text", Text: "(continuing)"}},
		}}, out...)
	}

	return out, system
}

// peelSystemInstruction lifts a client-injected <system_instruction> block out
// of a user turn. Those tags in the user role look like a jailbreak to Claude
// and get ignored. The inner text belongs in the real system prompt.
func peelSystemInstruction(content string) (instruction, rest string) {
	const open, close = "<system_instruction>", "</system_instruction>"
	rest = content
	var parts []string
	for {
		start := strings.Index(rest, open)
		if start < 0 {
			break
		}
		relEnd := strings.Index(rest[start:], close)
		if relEnd < 0 {
			break
		}
		end := start + relEnd
		inner := strings.TrimSpace(rest[start+len(open) : end])
		if inner != "" {
			parts = append(parts, inner)
		}
		rest = rest[:start] + rest[end+len(close):]
	}
	return strings.Join(parts, "\n\n"), strings.TrimSpace(rest)
}

func buildAnthropicTools(tools []ToolDef) []anthropicTool {
	out := make([]anthropicTool, 0, len(tools))
	for _, t := range tools {
		params := t.Function.Parameters
		if params == nil {
			params = map[string]interface{}{"type": "object", "properties": map[string]interface{}{}}
		}
		out = append(out, anthropicTool{
			Name:        t.Function.Name,
			Description: t.Function.Description,
			InputSchema: params,
		})
	}
	return out
}

// ---------------------------------------------------------------------------
// Streaming
// ---------------------------------------------------------------------------

// streamAnthropic runs one turn against the Messages API, emitting the same
// StreamChunk sequence the OpenAI-compatible path produces so callers need no
// special case.
func (c *Client) streamAnthropic(
	messages []Message,
	tools []ToolDef,
	systemPrompt string,
	opts *StreamOptions,
	onChunk func(StreamChunk),
) error {
	maxTokens := defaultAnthropicMaxTokens
	if opts != nil && opts.MaxTokensOverride > 0 {
		maxTokens = opts.MaxTokensOverride
	}

	msgs, system := buildAnthropicMessages(messages, systemPrompt)
	body := anthropicRequest{
		Model:     AnthropicModelID(c.model),
		MaxTokens: maxTokens,
		Messages:  msgs,
		Tools:     buildAnthropicTools(tools),
		Stream:    true,
	}
	if system != "" {
		body.System = system
	}

	applyAnthropicThinking(&body, c.model, opts)

	payload, err := json.Marshal(body)
	if err != nil {
		return fmt.Errorf("failed to encode request: %w", err)
	}

	req, err := http.NewRequest("POST", c.baseURL+"/messages", bytes.NewReader(payload))
	if err != nil {
		return err
	}
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("anthropic-version", anthropicVersion)
	req.Header.Set("x-api-key", c.apiKey)

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != 200 {
		b, _ := io.ReadAll(resp.Body)
		return fmt.Errorf("Anthropic API error %d: %s", resp.StatusCode, sanitizeAPIErrorBody(resp.StatusCode, b))
	}

	return parseAnthropicSSE(resp.Body, onChunk)
}

// anthropicEvent covers the fields we read across every event type.
type anthropicEvent struct {
	Type         string `json:"type"`
	Index        int    `json:"index"`
	ContentBlock *struct {
		Type string `json:"type"`
		ID   string `json:"id"`
		Name string `json:"name"`
		Text string `json:"text"`
	} `json:"content_block"`
	Delta *struct {
		Type        string `json:"type"`
		Text        string `json:"text"`
		PartialJSON string `json:"partial_json"`
		StopReason  string `json:"stop_reason"`
	} `json:"delta"`
	Message *struct {
		Usage *anthropicUsage `json:"usage"`
	} `json:"message"`
	Usage *anthropicUsage `json:"usage"`
	Error *struct {
		Type    string `json:"type"`
		Message string `json:"message"`
	} `json:"error"`
}

type anthropicUsage struct {
	InputTokens              int `json:"input_tokens"`
	OutputTokens             int `json:"output_tokens"`
	CacheCreationInputTokens int `json:"cache_creation_input_tokens"`
	CacheReadInputTokens     int `json:"cache_read_input_tokens"`
}

// parseAnthropicSSE translates Anthropic's event stream into StreamChunks.
//
// Tool arguments arrive as `input_json_delta` fragments that only form valid
// JSON once the block closes, so each tool call is accumulated by block index
// and emitted at `content_block_stop`.
func parseAnthropicSSE(r io.Reader, onChunk func(StreamChunk)) error {
	scanner := bufio.NewScanner(r)
	scanner.Buffer(make([]byte, 0, 64*1024), 8*1024*1024)

	type pendingTool struct {
		id   string
		name string
		args strings.Builder
	}
	pending := map[int]*pendingTool{}
	var completed []ToolCall
	usage := Usage{}

	for scanner.Scan() {
		line := scanner.Text()
		if !strings.HasPrefix(line, "data: ") {
			continue
		}
		data := strings.TrimPrefix(line, "data: ")
		if data == "" || data == "[DONE]" {
			continue
		}

		var ev anthropicEvent
		if err := json.Unmarshal([]byte(data), &ev); err != nil {
			continue // a malformed frame should not kill the turn
		}

		switch ev.Type {
		case "message_start":
			if ev.Message != nil && ev.Message.Usage != nil {
				usage.PromptTokens = ev.Message.Usage.InputTokens
				usage.CacheCreationInputTokens = ev.Message.Usage.CacheCreationInputTokens
				usage.CacheReadInputTokens = ev.Message.Usage.CacheReadInputTokens
			}

		case "content_block_start":
			if ev.ContentBlock == nil {
				continue
			}
			switch ev.ContentBlock.Type {
			case "tool_use":
				pending[ev.Index] = &pendingTool{id: ev.ContentBlock.ID, name: ev.ContentBlock.Name}
			case "text":
				if ev.ContentBlock.Text != "" {
					onChunk(StreamChunk{Type: "text", Content: ev.ContentBlock.Text})
				}
			}

		case "content_block_delta":
			if ev.Delta == nil {
				continue
			}
			switch ev.Delta.Type {
			case "text_delta":
				if ev.Delta.Text != "" {
					onChunk(StreamChunk{Type: "text", Content: ev.Delta.Text})
				}
			case "input_json_delta":
				if p := pending[ev.Index]; p != nil {
					p.args.WriteString(ev.Delta.PartialJSON)
				}
			}

		case "content_block_stop":
			p := pending[ev.Index]
			if p == nil {
				continue
			}
			delete(pending, ev.Index)
			args := p.args.String()
			if strings.TrimSpace(args) == "" {
				args = "{}"
			}
			tc := ToolCall{
				ID:       p.id,
				Type:     "function",
				Function: ToolCallFunc{Name: p.name, Arguments: args},
			}
			completed = append(completed, tc)
			onChunk(StreamChunk{Type: "tool_call", ToolCalls: []ToolCall{tc}})

		case "message_delta":
			if ev.Usage != nil {
				usage.CompletionTokens = ev.Usage.OutputTokens
			}

		case "error":
			if ev.Error != nil {
				return fmt.Errorf("Anthropic stream error: %s", ev.Error.Message)
			}
			return fmt.Errorf("Anthropic stream error")

		case "message_stop":
			// Terminal event; totals are emitted below.
		}
	}

	if err := scanner.Err(); err != nil {
		return fmt.Errorf("stream read failed: %w", err)
	}

	usage.TotalTokens = usage.PromptTokens + usage.CompletionTokens
	if usage.TotalTokens > 0 {
		u := usage
		onChunk(StreamChunk{Type: "usage", TokensUsed: &u})
	}
	if len(completed) > 0 {
		onChunk(StreamChunk{Type: "tool_calls_complete", ToolCalls: completed})
	}
	onChunk(StreamChunk{Type: "done", Done: true})
	return nil
}
