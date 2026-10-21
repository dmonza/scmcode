// Package analytics is a no-op in scmCode.
//
// Upstream SideX shipped an optional PostHog client here (enabled by a
// POSTHOG_API_KEY environment variable). scmCode never sends product
// analytics, so every method below intentionally does nothing. The API is
// kept so callers in the agent server do not need to change.
package analytics

// Analytics is a disabled analytics sink. All methods are no-ops.
type Analytics struct{}

func NewAnalytics() *Analytics { return &Analytics{} }

func (a *Analytics) Close() {}

func (a *Analytics) IsEnabled() bool { return false }

func (a *Analytics) Identify(userID, email, plan string, props map[string]interface{}) {}

func (a *Analytics) TrackAgentRequest(userID string, props map[string]interface{}) {}

func (a *Analytics) TrackSessionStart(userID, sessionID, model string) {}

func (a *Analytics) TrackSessionEnd(userID string, props map[string]interface{}) {}

func (a *Analytics) TrackAgentTurn(userID string, props map[string]interface{}) {}

func (a *Analytics) TrackToolExecution(userID string, props map[string]interface{}) {}

func (a *Analytics) TrackToolError(userID string, props map[string]interface{}) {}

func (a *Analytics) TrackAIGeneration(userID string, props map[string]interface{}) {}

func (a *Analytics) TrackAISpan(userID string, props map[string]interface{}) {}

func (a *Analytics) TrackAIEmbedding(userID string, props map[string]interface{}) {}

func (a *Analytics) TrackLLMCall(userID string, traceID string, model string, inputTokens, outputTokens int, costUSD float64, latencySeconds float64, input, output string, tools []string, isError bool, errorMsg string) {
}

func (a *Analytics) TrackEmbeddingCall(userID, traceID, model string, inputTokens int, latencySeconds float64, dimensions int) {
}

func (a *Analytics) TrackIndexSync(userID string, props map[string]interface{}) {}

func (a *Analytics) TrackSemanticSearch(userID string, props map[string]interface{}) {}

func (a *Analytics) TrackEmbedding(userID string, props map[string]interface{}) {}

func (a *Analytics) TrackRerank(userID string, props map[string]interface{}) {}

func (a *Analytics) TrackContextAssembly(userID string, props map[string]interface{}) {}

func (a *Analytics) TrackMemoryLearned(userID string, props map[string]interface{}) {}

func (a *Analytics) TrackMemoryRecalled(userID string, props map[string]interface{}) {}

func (a *Analytics) TrackCost(userID string, props map[string]interface{}) {}

func (a *Analytics) TrackModelSwitch(userID, fromModel, toModel, sessionID string) {}

func (a *Analytics) TrackCompletion(userID string, props map[string]interface{}) {}

func (a *Analytics) TrackPlanEvent(userID, eventType string, props map[string]interface{}) {}

func (a *Analytics) TrackFeatureUsage(userID, feature string, props map[string]interface{}) {}

func (a *Analytics) TrackError(userID string, props map[string]interface{}) {}

func (a *Analytics) TrackLatency(userID, operation string, durationMs int64, props map[string]interface{}) {
}

func (a *Analytics) TrackRateLimitHit(userID string, props map[string]interface{}) {}
