package ai

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestParseProviderModelListDataAndModelsEnvelopes(t *testing.T) {
	anth := []byte(`{"data":[{"type":"model","id":"claude-opus-5","display_name":"Claude Opus 5"},{"id":"claude-sonnet-4-6","display_name":"Claude Sonnet 4.6"}]}`)
	got, err := parseProviderModelList("anthropic", anth)
	if err != nil {
		t.Fatal(err)
	}
	if len(got) != 2 || got[0].ID != "anthropic/claude-opus-5" || got[0].Name != "Claude Opus 5" {
		t.Fatalf("anthropic parse = %#v", got)
	}

	models := []byte(`{"models":[{"slug":"gpt-5.6-terra","context_window":400000},{"slug":"gpt-5.4-mini","display_name":"GPT-5.4 mini"}]}`)
	got, err = parseProviderModelList("openai", json.RawMessage(models))
	if err != nil {
		t.Fatal(err)
	}
	if len(got) != 2 || got[0].ID != "openai/gpt-5.6-terra" || got[0].ContextWindow != 400000 || got[1].Name != "GPT-5.4 mini" {
		t.Fatalf("models envelope parse = %#v", got)
	}
}

func TestListRemoteModelsUsesProviderHeaders(t *testing.T) {
	var gotKey, gotAuth, gotBeta, gotVersion, gotXApp string
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		gotKey = r.Header.Get("X-Api-Key")
		gotAuth = r.Header.Get("Authorization")
		gotBeta = r.Header.Get("anthropic-beta")
		gotVersion = r.Header.Get("anthropic-version")
		gotXApp = r.Header.Get("x-app")
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"data":[{"id":"claude-opus-5","display_name":"Claude Opus 5"}]}`))
	}))
	defer srv.Close()

	got, err := ListRemoteModels(ProviderConfig{
		Provider: "anthropic",
		APIKey:   "sk-ant-test",
		BaseURL:  srv.URL,
		Enabled:  true,
	})
	if err != nil {
		t.Fatal(err)
	}
	if gotKey != "sk-ant-test" || gotVersion != anthropicVersion {
		t.Fatalf("headers key=%q ver=%q", gotKey, gotVersion)
	}
	if gotAuth != "" || gotBeta != "" || gotXApp != "" {
		t.Errorf("unexpected headers auth=%q beta=%q x-app=%q", gotAuth, gotBeta, gotXApp)
	}
	if len(got) != 1 || got[0].ID != "anthropic/claude-opus-5" {
		t.Fatalf("models = %#v", got)
	}
}

func TestListRemoteModelsSendsBearerForOpenAICompatible(t *testing.T) {
	var gotAuth string
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		gotAuth = r.Header.Get("Authorization")
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"data":[{"id":"gpt-5.4-mini"}]}`))
	}))
	defer srv.Close()

	if _, err := ListRemoteModels(ProviderConfig{
		Provider: "openai",
		APIKey:   "sk-test",
		BaseURL:  srv.URL,
		Enabled:  true,
	}); err != nil {
		t.Fatal(err)
	}
	if gotAuth != "Bearer sk-test" {
		t.Errorf("Authorization = %q, want %q", gotAuth, "Bearer sk-test")
	}
}

func TestConfiguredLocalProvidersReadsEnv(t *testing.T) {
	t.Setenv("SIDEX_PROVIDER_ANTHROPIC_KEY", "tok")
	t.Setenv("SIDEX_PROVIDER_ANTHROPIC_BASE_URL", "https://api.anthropic.com/v1")

	cfgs := ConfiguredLocalProviders()
	var found bool
	for _, c := range cfgs {
		if c.Provider == "anthropic" && c.APIKey == "tok" {
			found = true
		}
	}
	if !found {
		t.Fatalf("anthropic missing from %#v", cfgs)
	}
}
