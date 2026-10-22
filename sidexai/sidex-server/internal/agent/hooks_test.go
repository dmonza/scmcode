package agent

import (
	"os"
	"path/filepath"
	"testing"
)

func TestLoadFromConfig_DoesNotReplaceRegisteredHooks(t *testing.T) {
	r := NewHookRegistry()
	r.Register(HookBeforeToolUse, "security_gate", func(*HookContext) *HookResult {
		return &HookResult{Allow: false, Inject: "built-in gate"}
	}, 100)

	// The first entry tries to take over the built-in gate; the second is a
	// new name and must still load. Both use a builtin handler so the test
	// runs no command.
	cfg := `{"hooks":[
		{"name":"security_gate","events":["before_tool_use"],"builtin":"cost_alert"},
		{"name":"mine","events":["after_turn"],"builtin":"cost_alert"}
	]}`
	path := filepath.Join(t.TempDir(), "agent-hooks.json")
	if err := os.WriteFile(path, []byte(cfg), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := r.LoadFromConfig(path); err != nil {
		t.Fatalf("LoadFromConfig: %v", err)
	}

	res := r.Fire(&HookContext{Event: HookBeforeToolUse})
	if res.Allow || res.Inject != "built-in gate" {
		t.Errorf("built-in security_gate was replaced: got %+v", res)
	}
	if !r.HasHandlers(HookAfterTurn) {
		t.Error("a config hook with a new name was not loaded")
	}
}

func TestLoadFromConfig_MissingFileIsNoOp(t *testing.T) {
	r := NewHookRegistry()
	if err := r.LoadFromConfig(filepath.Join(t.TempDir(), "absent.json")); err != nil {
		t.Fatalf("missing file should be a no-op, got %v", err)
	}
}
