package ai

import (
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

func TestAnthropicModelID(t *testing.T) {
	cases := map[string]string{
		"anthropic/claude-opus-4.6":   "claude-opus-4-6",
		"anthropic/claude-sonnet-4.6": "claude-sonnet-4-6",
		"claude-haiku-4.5":            "claude-haiku-4-5",
		"claude-sonnet-4-6":           "claude-sonnet-4-6",
	}
	for in, want := range cases {
		if got := AnthropicModelID(in); got != want {
			t.Errorf("AnthropicModelID(%q) = %q, want %q", in, got, want)
		}
	}
}

func TestSystemPromptIsHoisted(t *testing.T) {
	msgs, system := buildAnthropicMessages([]Message{
		{Role: RoleSystem, Content: "stray system turn"},
		{Role: RoleUser, Content: "hello"},
	}, "base prompt")

	if !strings.Contains(system, "base prompt") || !strings.Contains(system, "stray system turn") {
		t.Fatalf("system prompt lost content: %q", system)
	}
	for _, m := range msgs {
		if m.Role == "system" {
			t.Fatal("system must not appear as a message role")
		}
	}
}

func TestUserSystemInstructionIsPeeledIntoSystem(t *testing.T) {
	msgs, system := buildAnthropicMessages([]Message{
		{Role: RoleUser, Content: "<system_instruction>\nYou have local tools.\n</system_instruction>\n\nhello"},
	}, "base prompt")

	if !strings.Contains(system, "base prompt") || !strings.Contains(system, "You have local tools.") {
		t.Fatalf("instruction not hoisted: %q", system)
	}
	if len(msgs) != 1 || msgs[0].Content[0].Text != "hello" {
		t.Fatalf("user turn should keep only the real message, got %+v", msgs)
	}
	for _, m := range msgs {
		for _, b := range m.Content {
			if strings.Contains(b.Text, "system_instruction") {
				t.Fatalf("tag leaked into user content: %q", b.Text)
			}
		}
	}
}

func TestToolResultBecomesUserBlock(t *testing.T) {
	msgs, _ := buildAnthropicMessages([]Message{
		{Role: RoleUser, Content: "read a file"},
		{Role: RoleAssistant, ToolCalls: []ToolCall{{
			ID: "tc_1", Type: "function",
			Function: ToolCallFunc{Name: "read_file", Arguments: `{"path":"a.txt"}`},
		}}},
		{Role: RoleTool, ToolCallID: "tc_1", Content: "file body"},
	}, "")

	if len(msgs) != 3 {
		t.Fatalf("expected 3 messages, got %d", len(msgs))
	}
	if msgs[1].Role != "assistant" || msgs[1].Content[0].Type != "tool_use" {
		t.Fatalf("assistant tool_use block missing: %+v", msgs[1])
	}
	// Anthropic requires the result to come back as a user-role tool_result.
	if msgs[2].Role != "user" || msgs[2].Content[0].Type != "tool_result" {
		t.Fatalf("tool result must be a user tool_result block: %+v", msgs[2])
	}
	if msgs[2].Content[0].ToolUseID != "tc_1" {
		t.Errorf("tool_use_id not carried through: %q", msgs[2].Content[0].ToolUseID)
	}
}

func TestConsecutiveToolResultsMerge(t *testing.T) {
	// Two results in a row must not become two user messages.
	msgs, _ := buildAnthropicMessages([]Message{
		{Role: RoleUser, Content: "go"},
		{Role: RoleAssistant, ToolCalls: []ToolCall{
			{ID: "a", Function: ToolCallFunc{Name: "x", Arguments: `{}`}},
			{ID: "b", Function: ToolCallFunc{Name: "y", Arguments: `{}`}},
		}},
		{Role: RoleTool, ToolCallID: "a", Content: "ra"},
		{Role: RoleTool, ToolCallID: "b", Content: "rb"},
	}, "")

	last := msgs[len(msgs)-1]
	if last.Role != "user" || len(last.Content) != 2 {
		t.Fatalf("expected both results merged into one user turn, got %+v", last)
	}
}

func TestMalformedToolArgumentsBecomeEmptyObject(t *testing.T) {
	// A truncated stream can leave invalid JSON; Anthropic would reject it.
	msgs, _ := buildAnthropicMessages([]Message{
		{Role: RoleUser, Content: "go"},
		{Role: RoleAssistant, ToolCalls: []ToolCall{
			{ID: "a", Function: ToolCallFunc{Name: "x", Arguments: `{"broken":`}},
		}},
	}, "")

	block := msgs[len(msgs)-1].Content[0]
	if string(block.Input) != "{}" {
		t.Errorf("invalid tool input should fall back to {}, got %s", block.Input)
	}
}

func TestConversationMustOpenWithUser(t *testing.T) {
	msgs, _ := buildAnthropicMessages([]Message{
		{Role: RoleAssistant, Content: "resuming"},
	}, "")
	if len(msgs) == 0 || msgs[0].Role != "user" {
		t.Fatalf("conversation must start with a user turn, got %+v", msgs)
	}
}

func TestParseAnthropicSSEEmitsTextToolsAndUsage(t *testing.T) {
	stream := strings.Join([]string{
		`data: {"type":"message_start","message":{"usage":{"input_tokens":11}}}`,
		`data: {"type":"content_block_start","index":0,"content_block":{"type":"text"}}`,
		`data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hel"}}`,
		`data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"lo"}}`,
		`data: {"type":"content_block_stop","index":0}`,
		`data: {"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"tc_9","name":"grep"}}`,
		`data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"q\":"}}`,
		`data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"\"x\"}"}}`,
		`data: {"type":"content_block_stop","index":1}`,
		`data: {"type":"message_delta","usage":{"output_tokens":7}}`,
		`data: {"type":"message_stop"}`,
	}, "\n")

	var text strings.Builder
	var tools []ToolCall
	var usage *Usage
	done := false

	err := parseAnthropicSSE(strings.NewReader(stream), func(c StreamChunk) {
		switch c.Type {
		case "text":
			text.WriteString(c.Content)
		case "tool_calls_complete":
			tools = c.ToolCalls
		case "usage":
			usage = c.TokensUsed
		case "done":
			done = true
		}
	})
	if err != nil {
		t.Fatalf("parse failed: %v", err)
	}
	if text.String() != "Hello" {
		t.Errorf("text = %q, want %q", text.String(), "Hello")
	}
	if len(tools) != 1 || tools[0].ID != "tc_9" || tools[0].Function.Name != "grep" {
		t.Fatalf("tool call not reassembled: %+v", tools)
	}
	// Fragments only form valid JSON once the block closes.
	if tools[0].Function.Arguments != `{"q":"x"}` {
		t.Errorf("arguments = %q, want %q", tools[0].Function.Arguments, `{"q":"x"}`)
	}
	if usage == nil || usage.PromptTokens != 11 || usage.CompletionTokens != 7 || usage.TotalTokens != 18 {
		t.Errorf("usage wrong: %+v", usage)
	}
	if !done {
		t.Error("stream never emitted done")
	}
}

func TestParseAnthropicSSESurfacesErrorEvent(t *testing.T) {
	stream := `data: {"type":"error","error":{"type":"overloaded_error","message":"upstream busy"}}`
	err := parseAnthropicSSE(strings.NewReader(stream), func(StreamChunk) {})
	if err == nil || !strings.Contains(err.Error(), "upstream busy") {
		t.Fatalf("expected the error event to surface, got %v", err)
	}
}

func TestLocalProviderKeyUsesDefaultBaseURL(t *testing.T) {
	t.Setenv("SIDEX_PROVIDER_ANTHROPIC_KEY", "sk-ant-test")
	cfg, ok := LocalProviderConfig("anthropic")
	if !ok {
		t.Fatal("anthropic should resolve from the environment")
	}
	if cfg.APIKey != "sk-ant-test" {
		t.Errorf("api key = %q, want %q", cfg.APIKey, "sk-ant-test")
	}
	if cfg.BaseURL != "https://api.anthropic.com/v1" {
		t.Errorf("base URL = %q", cfg.BaseURL)
	}
}

func TestLoopbackProviderNeedsNoKey(t *testing.T) {
	t.Setenv("SIDEX_PROVIDER_OLLAMA_BASE_URL", "http://127.0.0.1:11434/v1")
	if _, ok := LocalProviderConfig("ollama"); !ok {
		t.Error("a loopback provider must resolve without an API key")
	}
}

func TestRemoteProviderWithoutKeyIsRejected(t *testing.T) {
	t.Setenv("SIDEX_PROVIDER_OPENAI_BASE_URL", "https://api.openai.com/v1")
	t.Setenv("SIDEX_PROVIDER_OPENAI_KEY", "")
	if _, ok := LocalProviderConfig("openai"); ok {
		t.Error("a remote provider with no key must not resolve")
	}
}

// TestStreamAnthropicHTTPContract pins the wire format: URL, headers and the
// request body. A mock server makes this deterministic, unlike hitting the
// real API.
func TestStreamAnthropicHTTPContract(t *testing.T) {
	var gotPath, gotQuery, gotBeta, gotVersion, gotKey, gotAuth string
	var gotBody map[string]interface{}
	var gotHeader http.Header

	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		gotPath = r.URL.Path
		gotQuery = r.URL.RawQuery
		gotBeta = r.Header.Get("anthropic-beta")
		gotVersion = r.Header.Get("anthropic-version")
		gotKey = r.Header.Get("X-Api-Key")
		gotAuth = r.Header.Get("Authorization")
		gotHeader = r.Header.Clone()
		_ = json.NewDecoder(r.Body).Decode(&gotBody)

		w.Header().Set("Content-Type", "text/event-stream")
		fmt.Fprint(w, "data: {\"type\":\"content_block_delta\",\"index\":0,"+
			"\"delta\":{\"type\":\"text_delta\",\"text\":\"ok\"}}\n")
		fmt.Fprint(w, "data: {\"type\":\"message_stop\"}\n")
	}))
	defer srv.Close()

	c := (&Client{httpClient: srv.Client()}).WithModel("anthropic/claude-sonnet-4.6").
		WithProviderConfig(ProviderConfig{
			Provider: "anthropic",
			APIKey:   "sk-ant-test",
			BaseURL:  srv.URL,
			Enabled:  true,
		})

	var text strings.Builder
	err := c.StreamChatWithOptions(
		[]Message{{Role: RoleUser, Content: "hi"}}, nil, "be brief", nil,
		func(ch StreamChunk) {
			if ch.Type == "text" {
				text.WriteString(ch.Content)
			}
		})
	if err != nil {
		t.Fatalf("stream failed: %v", err)
	}

	if gotPath != "/messages" {
		t.Errorf("path = %q, want /messages", gotPath)
	}
	if gotQuery != "" {
		t.Errorf("query = %q, want empty", gotQuery)
	}
	if gotVersion != anthropicVersion {
		t.Errorf("anthropic-version = %q", gotVersion)
	}
	if gotKey != "sk-ant-test" {
		t.Errorf("X-Api-Key = %q, want %q", gotKey, "sk-ant-test")
	}
	if gotAuth != "" {
		t.Errorf("Authorization = %q, want empty for an API key", gotAuth)
	}
	if gotBeta != "" {
		t.Errorf("anthropic-beta = %q, want empty", gotBeta)
	}
	// scmCode presents only its own identity, never another client's.
	for _, h := range []string{"x-app", "X-Claude-Code-Session-Id", "anthropic-dangerous-direct-browser-access"} {
		if v := gotHeader.Get(h); v != "" {
			t.Errorf("%s = %q, want empty", h, v)
		}
	}
	if ua := gotHeader.Get("User-Agent"); strings.Contains(ua, "claude-cli") {
		t.Errorf("User-Agent = %q, must not impersonate a first-party CLI", ua)
	}
	if gotBody["system"] != "be brief" {
		t.Errorf("system = %v, want %q", gotBody["system"], "be brief")
	}
	// The catalog id must be normalised to Anthropic's spelling.
	if gotBody["model"] != "claude-sonnet-4-6" {
		t.Errorf("model = %v, want claude-sonnet-4-6", gotBody["model"])
	}
	if _, ok := gotBody["max_tokens"]; !ok {
		t.Error("max_tokens is required by the Messages API but was absent")
	}
	if text.String() != "ok" {
		t.Errorf("text = %q, want %q", text.String(), "ok")
	}
}

func TestAPIKeyKeepsMCPManagementTools(t *testing.T) {
	var gotBody map[string]interface{}
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		_ = json.NewDecoder(r.Body).Decode(&gotBody)
		w.Header().Set("Content-Type", "text/event-stream")
		fmt.Fprint(w, "data: {\"type\":\"message_stop\"}\n")
	}))
	defer srv.Close()

	c := (&Client{httpClient: srv.Client()}).WithModel("anthropic/claude-sonnet-4.6").
		WithProviderConfig(ProviderConfig{
			Provider: "anthropic",
			APIKey:   "sk-ant-test",
			BaseURL:  srv.URL,
			Enabled:  true,
		})
	if err := c.StreamChatWithOptions(
		[]Message{{Role: RoleUser, Content: "hi"}},
		[]ToolDef{{Function: ToolFuncDef{Name: "mcp_connect"}}},
		"", nil, func(StreamChunk) {}); err != nil {
		t.Fatal(err)
	}
	raw, _ := json.Marshal(gotBody["tools"])
	if !strings.Contains(string(raw), "mcp_connect") {
		t.Errorf("API key path should still send mcp_connect, got %s", raw)
	}
}
