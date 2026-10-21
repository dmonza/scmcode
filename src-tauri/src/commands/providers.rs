//! Provider credentials and local model discovery.
//!
//! scmCode runs without an account. Every model request is made with credentials
//! the user already has, resolved from three sources in this precedence order:
//!
//!   1. `settings`     — a key the user typed into Settings → Models
//!   2. `env`          — `ANTHROPIC_API_KEY` and friends, already in their shell
//!   3. `local`        — an OpenAI-compatible server on loopback (no key needed)
//!
//! Nothing here talks to sidex.dev, and nothing is ever read from another
//! tool's credential store.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::secrets::SecretsStore;

/// Where a resolved credential came from. Surfaced in the UI so the user can
/// see *why* a provider is working without revealing the key itself.
pub const SOURCE_SETTINGS: &str = "settings";
pub const SOURCE_ENV: &str = "env";
pub const SOURCE_LOCAL: &str = "local";

/// Anthropic rejects every REST call that omits this -- proven against the
/// live API: an unversioned `curl .../v1/models` comes back `400
/// {"error":{"message":"anthropic-version: header is required"}}`. Kept as a
/// local const rather than imported: the Go server (internal/ai/client.go)
/// defines the same value independently, and this crate has no dependency on
/// it.
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Only the API key is a secret. Base URLs and the on/off switch are ordinary
/// settings and are kept out of the OS keyring — routing them through it buys
/// no protection and costs the user a keychain access prompt every time the
/// app is rebuilt and re-signed.
///
/// Secret-store key for a provider's user-entered API key.
fn api_key_slot(provider: &str) -> String {
    format!("sidex.apikey.{provider}")
}

/// Secret-store key for a provider's base-URL override.
fn base_url_slot(provider: &str) -> String {
    format!("sidex.baseurl.{provider}")
}

/// Secret-store key for the explicit per-provider off switch. Distinct from
/// simply having no credential: a keyless local provider (Ollama, …) is
/// otherwise always "configured", so without this the user has no way to
/// turn one off — see `providers_set_enabled`.
fn provider_off_slot(provider: &str) -> String {
    format!("sidex.provider-off.{provider}")
}

// ---------------------------------------------------------------------------
// Catalog
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    /// Stable id used everywhere else (`anthropic`, `openai`, …).
    pub id: &'static str,
    pub label: &'static str,
    /// Default OpenAI-compatible endpoint. Overridable per provider.
    pub default_base_url: &'static str,
    /// Environment variables checked, in order, for a key.
    pub env_vars: &'static [&'static str],
    /// Where the user gets a key. Empty for local/self-hosted providers.
    pub console_url: &'static str,
    /// True when the provider needs no key at all (loopback servers).
    pub keyless: bool,
}

/// Every provider SideX knows how to talk to out of the box.
///
/// This is not a closed set — `providers_save` accepts any id with a custom
/// base URL, which is how "some OpenAI-compatible thing I self-host" works.
pub const PROVIDERS: &[ProviderInfo] = &[
    ProviderInfo {
        id: "anthropic",
        label: "Anthropic",
        default_base_url: "https://api.anthropic.com/v1",
        env_vars: &["ANTHROPIC_API_KEY"],
        console_url: "https://console.anthropic.com/settings/keys",
        keyless: false,
    },
    ProviderInfo {
        id: "openai",
        label: "OpenAI",
        default_base_url: "https://api.openai.com/v1",
        env_vars: &["OPENAI_API_KEY"],
        console_url: "https://platform.openai.com/account/api-keys",
        keyless: false,
    },
    ProviderInfo {
        id: "google",
        label: "Google AI Studio",
        default_base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
        env_vars: &["GEMINI_API_KEY", "GOOGLE_API_KEY"],
        console_url: "https://aistudio.google.com/app/apikey",
        keyless: false,
    },
    ProviderInfo {
        id: "openrouter",
        label: "OpenRouter",
        default_base_url: "https://openrouter.ai/api/v1",
        env_vars: &["OPENROUTER_API_KEY"],
        console_url: "https://openrouter.ai/keys",
        keyless: false,
    },
    ProviderInfo {
        id: "groq",
        label: "Groq",
        default_base_url: "https://api.groq.com/openai/v1",
        env_vars: &["GROQ_API_KEY"],
        console_url: "https://console.groq.com/keys",
        keyless: false,
    },
    ProviderInfo {
        id: "mistral",
        label: "Mistral",
        default_base_url: "https://api.mistral.ai/v1",
        env_vars: &["MISTRAL_API_KEY"],
        console_url: "https://console.mistral.ai/api-keys",
        keyless: false,
    },
    ProviderInfo {
        id: "deepseek",
        label: "DeepSeek",
        default_base_url: "https://api.deepseek.com/v1",
        env_vars: &["DEEPSEEK_API_KEY"],
        console_url: "https://platform.deepseek.com/api_keys",
        keyless: false,
    },
    ProviderInfo {
        id: "moonshot",
        label: "Moonshot (Kimi)",
        default_base_url: "https://api.moonshot.cn/v1",
        env_vars: &["MOONSHOT_API_KEY"],
        console_url: "https://platform.moonshot.cn/console/api-keys",
        keyless: false,
    },
    ProviderInfo {
        id: "zhipu",
        label: "Zhipu (GLM)",
        default_base_url: "https://open.bigmodel.cn/api/paas/v4",
        env_vars: &["ZHIPU_API_KEY"],
        console_url: "https://open.bigmodel.cn/usercenter/apikeys",
        keyless: false,
    },
    ProviderInfo {
        id: "xai",
        label: "xAI (Grok)",
        default_base_url: "https://api.x.ai/v1",
        env_vars: &["XAI_API_KEY"],
        console_url: "https://console.x.ai",
        keyless: false,
    },
    ProviderInfo {
        id: "together",
        label: "Together AI",
        default_base_url: "https://api.together.xyz/v1",
        env_vars: &["TOGETHER_API_KEY"],
        console_url: "https://api.together.xyz/settings/api-keys",
        keyless: false,
    },
    ProviderInfo {
        id: "cerebras",
        label: "Cerebras",
        default_base_url: "https://api.cerebras.ai/v1",
        env_vars: &["CEREBRAS_API_KEY"],
        console_url: "https://cloud.cerebras.ai",
        keyless: false,
    },
    ProviderInfo {
        id: "fireworks",
        label: "Fireworks AI",
        default_base_url: "https://api.fireworks.ai/inference/v1",
        env_vars: &["FIREWORKS_API_KEY"],
        console_url: "https://app.fireworks.ai/settings/users/api-keys",
        keyless: false,
    },
    ProviderInfo {
        id: "deepinfra",
        label: "DeepInfra",
        default_base_url: "https://api.deepinfra.com/v1/openai",
        env_vars: &["DEEPINFRA_API_KEY"],
        console_url: "https://deepinfra.com/dash/api_keys",
        keyless: false,
    },
    ProviderInfo {
        id: "perplexity",
        label: "Perplexity",
        // Perplexity is retiring its classic `/chat/completions` (Sonar)
        // surface into what it now calls the Agent API; `/v1` is the base
        // its own docs give for OpenAI-SDK-style requests going forward.
        default_base_url: "https://api.perplexity.ai/v1",
        env_vars: &["PERPLEXITY_API_KEY"],
        console_url: "https://console.perplexity.ai/project/keys",
        keyless: false,
    },
    ProviderInfo {
        id: "hyperbolic",
        label: "Hyperbolic",
        default_base_url: "https://api.hyperbolic.xyz/v1",
        env_vars: &["HYPERBOLIC_API_KEY"],
        console_url: "https://app.hyperbolic.ai/settings/api-keys",
        keyless: false,
    },
    ProviderInfo {
        id: "nebius",
        label: "Nebius Token Factory",
        default_base_url: "https://api.tokenfactory.nebius.com/v1",
        env_vars: &["NEBIUS_API_KEY"],
        console_url: "https://tokenfactory.nebius.com/project/api-keys",
        keyless: false,
    },
    ProviderInfo {
        id: "sambanova",
        label: "SambaNova Cloud",
        default_base_url: "https://api.sambanova.ai/v1",
        env_vars: &["SAMBANOVA_API_KEY"],
        console_url: "https://cloud.sambanova.ai/apis",
        keyless: false,
    },
    ProviderInfo {
        id: "novita",
        label: "Novita AI",
        default_base_url: "https://api.novita.ai/openai",
        env_vars: &["NOVITA_API_KEY"],
        console_url: "https://novita.ai/settings/key-management",
        keyless: false,
    },
    ProviderInfo {
        id: "anyscale",
        label: "Anyscale",
        // Anyscale's self-serve multi-tenant Endpoints product still
        // documents this base URL/key pair, though since Aug 2024 new
        // access goes through Anyscale's enterprise-hosted platform rather
        // than open signup -- kept here for the users who already have a key.
        default_base_url: "https://api.endpoints.anyscale.com/v1",
        env_vars: &["ANYSCALE_API_KEY"],
        console_url: "https://app.endpoints.anyscale.com/credentials",
        keyless: false,
    },
    ProviderInfo {
        id: "openai-compatible",
        label: "OpenAI-compatible (custom)",
        // Deliberately empty: this is the escape hatch for a self-hosted or
        // aggregator endpoint that isn't in the catalog above, and there is
        // no default that would be meaningful for it. An empty string here
        // (not merely a missing override) is what makes the base-URL
        // fallback in `resolve_ignoring_enabled` refuse to resolve this
        // provider until the user supplies one.
        default_base_url: "",
        env_vars: &[],
        console_url: "",
        keyless: false,
    },
    ProviderInfo {
        id: "bedrock",
        label: "AWS Bedrock",
        // Region-specific: there is no global Bedrock endpoint, and the region
        // is parsed back out of this host for SigV4's credential scope.
        default_base_url: "",
        // AWS credentials are a pair, not a single value, so they cannot be
        // picked up from one variable. The key is entered in Settings as
        // `AccessKeyID:SecretAccessKey` (plus `:SessionToken` for STS).
        env_vars: &[],
        console_url: "https://console.aws.amazon.com/bedrock",
        keyless: false,
    },
    ProviderInfo {
        id: "ollama",
        label: "Ollama",
        default_base_url: "http://127.0.0.1:11434/v1",
        env_vars: &[],
        console_url: "",
        keyless: true,
    },
    ProviderInfo {
        id: "lmstudio",
        label: "LM Studio",
        default_base_url: "http://127.0.0.1:1234/v1",
        env_vars: &[],
        console_url: "",
        keyless: true,
    },
    ProviderInfo {
        id: "llamacpp",
        label: "llama.cpp",
        default_base_url: "http://127.0.0.1:8080/v1",
        env_vars: &[],
        console_url: "",
        keyless: true,
    },
    ProviderInfo {
        id: "vllm",
        label: "vLLM",
        default_base_url: "http://127.0.0.1:8000/v1",
        env_vars: &[],
        console_url: "",
        keyless: true,
    },
];

fn provider_info(id: &str) -> Option<&'static ProviderInfo> {
    PROVIDERS.iter().find(|p| p.id == id)
}

/// Providers that are OpenAI-compatible servers on loopback, in probe order.
const LOCAL_PROVIDERS: &[&str] = &["ollama", "lmstudio", "llamacpp", "vllm"];

#[tauri::command]
pub fn providers_catalog() -> Vec<ProviderInfo> {
    PROVIDERS.to_vec()
}

// ---------------------------------------------------------------------------
// Resolution
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub id: String,
    pub label: String,
    pub base_url: String,
    /// True when a usable API key / keyless server was found. Independent of
    /// `enabled` — a disabled row still shows the typed key.
    pub configured: bool,
    /// Which of the three sources supplied it. `None` when unconfigured.
    pub source: Option<String>,
    pub keyless: bool,
    /// Set when `source == "env"`, so the UI can say which variable won.
    pub env_var: Option<String>,
    /// False when the user has explicitly switched this provider off via
    /// `providers_set_enabled`. Drives the settings toggle; `configured`
    /// drives whether the provider can actually be used.
    pub enabled: bool,
}

/// A credential ready to be handed to the model server. The key is only ever
/// passed to the loopback server process — it is never logged or returned to
/// the webview.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedProvider {
    pub provider: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub source: String,
}

/// Resolve one provider's credential without revealing it. A provider the
/// user has switched off (`provider_disabled`) never resolves, whatever
/// credentials it has.
fn resolve_inner(store: &SecretsStore, id: &str) -> Option<ResolvedProvider> {
    if provider_disabled(store, id) {
        return None;
    }
    resolve_ignoring_enabled(store, id)
}

fn provider_base_url(store: &SecretsStore, id: &str) -> Option<String> {
    let info = provider_info(id);
    store
        .inner
        .get_plain(&base_url_slot(id))
        .ok()
        .flatten()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            std::env::var(format!("SIDEX_PROVIDER_{}_BASE_URL", id.to_uppercase()))
                .ok()
                .filter(|s| !s.trim().is_empty())
        })
        // `openai-compatible`'s catalog entry deliberately has an empty
        // `default_base_url` (there is no sensible one) -- filtered out here
        // too, not just at the settings/env steps, so that empty string can
        // never stand in for "no base URL supplied" and the `?` below
        // actually fails resolution for it until the user provides one.
        .or_else(|| {
            info.map(|i| i.default_base_url.to_string())
                .filter(|s| !s.is_empty())
        })
}

/// Settings / env / keyless resolution, deaf to the off switch. Used by
/// `providers_status` so the API Keys row still shows the typed key while
/// the toggle is off.
fn resolve_ignoring_enabled(store: &SecretsStore, id: &str) -> Option<ResolvedProvider> {
    let info = provider_info(id);
    let base_url = provider_base_url(store, id)?;

    // 1. Explicit key from Settings.
    if let Ok(Some(key)) = store.inner.get(&api_key_slot(id)) {
        if !key.trim().is_empty() {
            return Some(ResolvedProvider {
                provider: id.to_string(),
                base_url,
                api_key: Some(key),
                source: SOURCE_SETTINGS.to_string(),
            });
        }
    }

    // 2. Environment.
    if let Some(info) = info {
        for var in info.env_vars {
            if let Ok(val) = std::env::var(var) {
                if !val.trim().is_empty() {
                    return Some(ResolvedProvider {
                        provider: id.to_string(),
                        base_url,
                        api_key: Some(val),
                        source: SOURCE_ENV.to_string(),
                    });
                }
            }
        }
    }

    // 3. Loopback servers need no credential at all.
    if info.is_some_and(|i| i.keyless) {
        return Some(ResolvedProvider {
            provider: id.to_string(),
            base_url,
            api_key: None,
            source: SOURCE_LOCAL.to_string(),
        });
    }

    None
}

/// Status of every known provider, for the Settings UI. Never returns keys.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn providers_status(store: State<'_, Arc<SecretsStore>>) -> Vec<ProviderStatus> {
    PROVIDERS
        .iter()
        .map(|info| {
            // Deliberately not `resolve_inner`: the toggle needs to keep
            // showing what the provider would resolve to while it's off, so
            // flipping it back on doesn't come with a surprise.
            let resolved = resolve_ignoring_enabled(&store, info.id);
            let env_var = resolved.as_ref().and_then(|r| {
                (r.source == SOURCE_ENV)
                    .then(|| {
                        info.env_vars
                            .iter()
                            .find(|v| std::env::var(v).is_ok_and(|s| !s.trim().is_empty()))
                            .map(|v| (*v).to_string())
                    })
                    .flatten()
            });
            ProviderStatus {
                id: info.id.to_string(),
                label: info.label.to_string(),
                base_url: resolved
                    .as_ref()
                    .map_or_else(|| info.default_base_url.to_string(), |r| r.base_url.clone()),
                configured: resolved.is_some(),
                source: resolved.map(|r| r.source),
                keyless: info.keyless,
                env_var,
                enabled: !provider_disabled(&store, info.id),
            }
        })
        .collect()
}

/// Resolve the credential set the model server should run with.
///
/// Returned to the Rust sidecar launcher only — see `server.rs`.
pub fn resolve_all(store: &SecretsStore) -> Vec<ResolvedProvider> {
    PROVIDERS
        .iter()
        .filter_map(|info| resolve_inner(store, info.id))
        .collect()
}

// ---------------------------------------------------------------------------
// Writes
// ---------------------------------------------------------------------------

/// Save a provider credential. An empty `api_key` clears it, which lets the
/// next source in the precedence chain take over.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn providers_save(
    store: State<'_, Arc<SecretsStore>>,
    provider: String,
    api_key: Option<String>,
    base_url: Option<String>,
) -> Result<(), String> {
    if provider.trim().is_empty() {
        return Err("provider id is required".to_string());
    }

    match api_key.as_deref().map(str::trim) {
        Some("") | None => {
            let _ = store.inner.delete(&api_key_slot(&provider));
        }
        Some(key) => store
            .inner
            .set(&api_key_slot(&provider), key)
            .map_err(|e| e.to_string())?,
    }

    match base_url.as_deref().map(str::trim) {
        Some("") | None => {
            let _ = store.inner.delete_plain(&base_url_slot(&provider));
        }
        Some(url) => {
            if !url.starts_with("http://") && !url.starts_with("https://") {
                return Err("base URL must start with http:// or https://".to_string());
            }
            store
                .inner
                .set_plain(&base_url_slot(&provider), url.trim_end_matches('/'))
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

/// The delete action itself, pulled out of `providers_delete` so it's callable
/// with a plain `&SecretsStore` — both from the command and from tests that
/// have no `tauri::State` to construct.
fn delete_provider(store: &SecretsStore, provider: &str) {
    let _ = store.inner.delete(&api_key_slot(provider));
    let _ = store.inner.delete_plain(&base_url_slot(provider));
    // Deleting must reset the provider to its true default (on), not leave
    // it stuck off with nothing left to show that in the UI.
    let _ = store.inner.delete_plain(&provider_off_slot(provider));
}

/// Forget everything stored for a provider, including its off switch.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn providers_delete(
    store: State<'_, Arc<SecretsStore>>,
    provider: String,
) -> Result<(), String> {
    delete_provider(&store, &provider);
    Ok(())
}

/// True when the user has explicitly switched a provider off. Distinct from
/// `configured`: a keyless provider is always "configured" (it needs no
/// credential), so without this separate flag there would be no way to stop
/// `SideX` from treating it as available.
fn provider_disabled(store: &SecretsStore, provider: &str) -> bool {
    matches!(
        store
            .inner
            .get_plain(&provider_off_slot(provider))
            .ok()
            .flatten()
            .as_deref(),
        Some("1")
    )
}

/// Flip the per-provider off switch. Note the polarity is inverted from the
/// slot itself: `enabled: true` clears the "off" row (the default state —
/// nothing stored means on), `enabled: false` writes it.
fn set_provider_enabled(store: &SecretsStore, provider: &str, enabled: bool) -> Result<(), String> {
    if enabled {
        let _ = store.inner.delete_plain(&provider_off_slot(provider));
        Ok(())
    } else {
        store
            .inner
            .set_plain(&provider_off_slot(provider), "1")
            .map_err(|e| e.to_string())
    }
}

/// Turn a provider on or off.
///
/// This is the fix for keyless local providers (Ollama, LM Studio, …) always
/// showing "configured": they need no credential, so before this switch
/// existed there was nothing to clear that would ever make the toggle stick
/// in the off position. `resolve_inner` checks this before every other
/// credential source, so a disabled provider is invisible to `server_env`
/// regardless of what credentials it has.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn providers_set_enabled(
    store: State<'_, Arc<SecretsStore>>,
    provider: String,
    enabled: bool,
) -> Result<(), String> {
    set_provider_enabled(&store, &provider, enabled)
}

// ---------------------------------------------------------------------------
// Local model discovery
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalServer {
    pub provider: String,
    pub label: String,
    pub base_url: String,
    pub models: Vec<String>,
}

#[derive(Deserialize)]
struct ModelList {
    #[serde(default)]
    data: Vec<ModelEntry>,
}

/// One `/models` entry. Shared by both response shapes this file parses --
/// Anthropic's (`{"type":"model","id":...,"display_name":...,"created_at":...}`)
/// and the OpenAI-compatible one (`{"id":...,"object":"model",...}`) -- so it
/// only models the two fields either of them might carry. No
/// `#[serde(deny_unknown_fields)]` here: every other field either shape sends
/// (`object`, `type`, `created_at`, `owned_by`, ...) must be ignored, not
/// rejected.
#[derive(Deserialize)]
struct ModelEntry {
    id: String,
    #[serde(default)]
    display_name: Option<String>,
}

/// One model a provider offers, shaped for the Settings UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModel {
    /// Fully-qualified id, ALWAYS provider-prefixed (e.g. `anthropic/claude-opus-5`).
    pub id: String,
    /// Human label. Falls back to the bare model id when the API gives none.
    pub name: String,
}

/// The Go server derives a request's provider by splitting the model id on
/// `/` (`ProviderFromModelID`, internal/ai). Anthropic's `/models` endpoint
/// returns bare ids (`claude-opus-5`) with no prefix at all, so one of those
/// handed back unqualified would be unroutable; applied to every provider
/// uniformly since a custom OpenAI-compatible base URL could in principle
/// return pre-qualified ids too, and those must be left alone.
fn qualify_model_id(provider: &str, id: String) -> String {
    if id.contains('/') {
        id
    } else {
        format!("{provider}/{id}")
    }
}

/// Turn one raw `/models` entry into what the UI shows. Split out from
/// `providers_list_models` so the id-prefixing and name-fallback rules are
/// unit testable without a network call.
fn provider_model_from_entry(provider: &str, entry: ModelEntry) -> ProviderModel {
    let name = entry.display_name.unwrap_or_else(|| entry.id.clone());
    ProviderModel {
        id: qualify_model_id(provider, entry.id),
        name,
    }
}

/// Probe the usual loopback ports for an OpenAI-compatible `/models` endpoint.
///
/// Only 127.0.0.1 is contacted, with a short timeout, so a cold call costs
/// about a second even when nothing is listening.
#[tauri::command]
pub async fn providers_detect_local() -> Vec<LocalServer> {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_millis(600))
        .build()
    {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let probes = LOCAL_PROVIDERS.iter().filter_map(|id| {
        let info = provider_info(id)?;
        let client = client.clone();
        Some(async move {
            let url = format!("{}/models", info.default_base_url);
            let resp = client.get(&url).send().await.ok()?;
            if !resp.status().is_success() {
                return None;
            }
            let list: ModelList = resp.json().await.ok()?;
            Some(LocalServer {
                provider: info.id.to_string(),
                label: info.label.to_string(),
                base_url: info.default_base_url.to_string(),
                models: list.data.into_iter().map(|m| m.id).collect(),
            })
        })
    });

    futures_util::future::join_all(probes)
        .await
        .into_iter()
        .flatten()
        .collect()
}

/// How a provider expects an authenticated request's headers built.
///
/// Everything defaults to `Bearer` -- the OpenAI-compatible convention every
/// provider in the catalog speaks except Anthropic's native REST API -- so
/// adding a new OpenAI-compatible provider (as this file now does nine times
/// over) never touches this type or `header_style` below; only a provider
/// with its own header scheme needs a new variant/arm, in one place instead
/// of a growing if-chain at the call site.
#[derive(Debug, PartialEq, Eq)]
enum HeaderStyle {
    /// `Authorization: Bearer <key>`, omitted entirely when there is no key
    /// (a keyless loopback provider).
    Bearer,
    /// Anthropic's native scheme -- see `apply_auth_headers` for the exact
    /// headers, which must stay byte-for-byte what they were before this
    /// type existed.
    AnthropicNative,
}

fn header_style(provider: &str) -> HeaderStyle {
    match provider {
        "anthropic" => HeaderStyle::AnthropicNative,
        _ => HeaderStyle::Bearer,
    }
}

/// Apply one provider's auth headers to an outgoing `/models` request.
///
/// Split out of `providers_list_models` so the per-provider header quirks
/// live in this one match rather than an if-chain that would otherwise grow
/// with every provider added to `PROVIDERS`.
fn apply_auth_headers(
    mut req: reqwest::RequestBuilder,
    provider: &str,
    resolved: &ResolvedProvider,
) -> reqwest::RequestBuilder {
    match header_style(provider) {
        HeaderStyle::AnthropicNative => {
            // Every Anthropic REST call needs this -- an unversioned request
            // is rejected before auth is even checked (see
            // `ANTHROPIC_VERSION`). Same pair the Go server sends.
            req = req.header("anthropic-version", ANTHROPIC_VERSION);
            if let Some(key) = &resolved.api_key {
                req = req.header("x-api-key", key);
            }
            req
        }
        HeaderStyle::Bearer => {
            // Keyless loopback providers (Ollama, ...) have no `api_key` here
            // at all, so this sends no `Authorization` header for them.
            if let Some(key) = &resolved.api_key {
                req = req.bearer_auth(key);
            }
            req
        }
    }
}

/// List the models a configured provider actually offers.
///
/// Used by Settings → Models so the catalog reflects what the user can really
/// reach, rather than a hardcoded list that drifts.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub async fn providers_list_models(
    store: State<'_, Arc<SecretsStore>>,
    provider: String,
) -> Result<Vec<ProviderModel>, String> {
    let resolved =
        resolve_inner(&store, &provider).ok_or_else(|| format!("{provider} is not configured"))?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let req = client.get(format!("{}/models", resolved.base_url));
    let req = apply_auth_headers(req, &provider, &resolved);

    let resp = req.send().await.map_err(|e| e.to_string())?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("{provider} returned HTTP {}", status.as_u16()));
    }
    let list: ModelList = resp.json().await.map_err(|e| e.to_string())?;
    // Preserve the API's own ordering (Anthropic returns newest first) --
    // this must not sort.
    Ok(provider_models_from_list(&provider, list))
}

fn provider_models_from_list(provider: &str, list: ModelList) -> Vec<ProviderModel> {
    list.data
        .into_iter()
        .map(|m| provider_model_from_entry(provider, m))
        .collect()
}

/// Env pairs handed to the model server process at spawn.
///
/// Keys travel through the process environment of a loopback-only child, which
/// keeps them out of the webview and out of any config file on disk.
pub fn server_env(store: &SecretsStore) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    for r in resolve_all(store) {
        let up = r.provider.to_uppercase();
        if let Some(key) = r.api_key {
            env.insert(format!("SIDEX_PROVIDER_{up}_KEY"), key);
        }
        env.insert(format!("SIDEX_PROVIDER_{up}_BASE_URL"), r.base_url);
    }
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_ids_are_unique() {
        let mut ids: Vec<_> = PROVIDERS.iter().map(|p| p.id).collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(before, ids.len(), "duplicate provider id in catalog");
    }

    #[test]
    fn local_providers_are_keyless_and_loopback() {
        for id in LOCAL_PROVIDERS {
            let info = provider_info(id).expect("local provider missing from catalog");
            assert!(info.keyless, "{id} should not require a key");
            assert!(
                info.default_base_url.starts_with("http://127.0.0.1:"),
                "{id} must probe loopback only, got {}",
                info.default_base_url
            );
        }
    }

    #[test]
    fn remote_providers_use_tls() {
        // `openai-compatible` is the one deliberate exception -- its
        // `default_base_url` is empty by design (there is no sensible
        // default for a user-supplied endpoint), so it's excluded here and
        // covered on its own by `generic_openai_compatible_has_no_default`.
        for info in PROVIDERS
            .iter()
            .filter(|p| !p.keyless && !p.default_base_url.is_empty())
        {
            assert!(
                info.default_base_url.starts_with("https://"),
                "{} must default to https, got {}",
                info.id,
                info.default_base_url
            );
        }
    }

    #[test]
    fn generic_openai_compatible_has_no_default() {
        let info = provider_info("openai-compatible").expect("must be in the catalog");
        assert!(
            info.default_base_url.is_empty(),
            "openai-compatible must carry no default -- there is no sensible one"
        );
        assert!(
            !info.keyless,
            "a user-supplied remote endpoint must require a key"
        );
    }

    #[test]
    fn newly_added_remote_providers_are_present_and_keyed() {
        for id in [
            "fireworks",
            "deepinfra",
            "perplexity",
            "hyperbolic",
            "nebius",
            "sambanova",
            "novita",
            "anyscale",
        ] {
            let info = provider_info(id).unwrap_or_else(|| panic!("{id} missing from catalog"));
            assert!(!info.keyless, "{id} is a hosted API and must require a key");
            assert!(
                !info.env_vars.is_empty(),
                "{id} must check at least one env var"
            );
            assert!(
                !info.console_url.is_empty(),
                "{id} must link to where a key is issued"
            );
        }
    }

    #[test]
    fn slots_are_namespaced_per_provider() {
        assert_eq!(api_key_slot("openai"), "sidex.apikey.openai");
        assert_eq!(base_url_slot("openai"), "sidex.baseurl.openai");
        assert_eq!(provider_off_slot("openai"), "sidex.provider-off.openai");
    }

    // -- explicit per-provider off switch --------------------------------

    /// A `SecretsStore` backed by its own throwaway `SQLite` file, with the OS
    /// keyring bypassed.
    ///
    /// The keyring is keyed by name alone, so it is shared by every store in
    /// the process — a unique database path alone would isolate nothing, tests
    /// would race each other, and a `cargo test` run could leave a provider
    /// switched off in the developer's real install.
    fn test_store() -> SecretsStore {
        let path =
            std::env::temp_dir().join(format!("sidex-providers-test-{}.db", uuid::Uuid::new_v4()));
        SecretsStore {
            inner: sidex_auth::SecretStorage::open_without_keyring(path)
                .expect("open test secret store"),
        }
    }

    /// The four disable behaviours share one test on purpose.
    ///
    /// `SecretStorage` reads the OS keyring before its SQLite fallback, and the
    /// keyring is keyed by name alone — so a per-test database file does NOT
    /// isolate anything. Run as separate `#[test]`s these race each other on
    /// the real keychain, and one test's "disabled" state breaks another's
    /// sanity check. Kept sequential, and cleaned up, so a `cargo test` run
    /// cannot leave a provider switched off in the developer's own install.
    #[test]
    fn disable_switch_governs_resolution_env_and_reset() {
        let store = test_store();
        let provider = "ollama";
        // Guard against debris from an interrupted earlier run.
        delete_provider(&store, provider);

        assert!(
            resolve_inner(&store, provider).is_some(),
            "sanity: a keyless provider resolves before being disabled"
        );
        assert!(
            server_env(&store)
                .keys()
                .any(|k| k.starts_with("SIDEX_PROVIDER_OLLAMA_")),
            "sanity: a keyless provider reaches server_env before being disabled"
        );

        set_provider_enabled(&store, provider, false).unwrap();
        assert!(
            resolve_inner(&store, provider).is_none(),
            "a disabled provider must not resolve just because it's keyless"
        );
        assert!(
            !server_env(&store)
                .keys()
                .any(|k| k.starts_with("SIDEX_PROVIDER_OLLAMA_")),
            "a disabled provider must never reach the server env the Go side reads"
        );

        set_provider_enabled(&store, provider, true).unwrap();
        assert!(
            resolve_inner(&store, provider).is_some(),
            "clearing the off switch must restore the provider's normal resolution"
        );

        set_provider_enabled(&store, provider, false).unwrap();
        assert!(provider_disabled(&store, provider));
        delete_provider(&store, provider);
        assert!(
            !provider_disabled(&store, provider),
            "deleting a provider must reset it to enabled, not leave it stuck off"
        );
    }

    #[test]
    fn server_env_only_carries_key_and_base_url() {
        let store = test_store();
        let provider = "openai-compatible";
        delete_provider(&store, provider);
        store.inner.set(&api_key_slot(provider), "sk-test").unwrap();
        store
            .inner
            .set_plain(&base_url_slot(provider), "https://example.com/v1")
            .unwrap();

        let env = server_env(&store);
        assert_eq!(
            env.get("SIDEX_PROVIDER_OPENAI-COMPATIBLE_KEY")
                .map(String::as_str),
            Some("sk-test")
        );
        assert_eq!(
            env.get("SIDEX_PROVIDER_OPENAI-COMPATIBLE_BASE_URL")
                .map(String::as_str),
            Some("https://example.com/v1")
        );
        for key in env.keys() {
            assert!(
                key.ends_with("_KEY") || key.ends_with("_BASE_URL"),
                "server_env must only emit _KEY / _BASE_URL pairs, got {key}"
            );
        }

        delete_provider(&store, provider);
    }

    // -- generic `openai-compatible` connector ---------------------------

    #[test]
    fn generic_openai_compatible_only_resolves_once_a_base_url_is_supplied() {
        let store = test_store();
        let provider = "openai-compatible";
        delete_provider(&store, provider);

        assert!(
            resolve_inner(&store, provider).is_none(),
            "must not resolve with nothing supplied -- it has no default base URL to fall back to"
        );

        // A key with no base URL still isn't enough: there is nothing to
        // fall back to for the URL half.
        store.inner.set(&api_key_slot(provider), "sk-test").unwrap();
        assert!(
            resolve_inner(&store, provider).is_none(),
            "a key alone must not resolve without a base URL"
        );

        store
            .inner
            .set_plain(&base_url_slot(provider), "https://example.com/v1")
            .unwrap();
        assert!(
            resolve_inner(&store, provider).is_some(),
            "must resolve once both a base URL and a key are supplied"
        );

        delete_provider(&store, provider);
    }

    // -- per-provider header strategy ------------------------------------

    #[test]
    fn header_style_is_anthropic_native_only_for_anthropic() {
        assert_eq!(header_style("anthropic"), HeaderStyle::AnthropicNative);
        for id in PROVIDERS
            .iter()
            .map(|p| p.id)
            .filter(|&id| id != "anthropic")
        {
            assert_eq!(
                header_style(id),
                HeaderStyle::Bearer,
                "{id} must use the default Bearer header style, not Anthropic's"
            );
        }
    }

    // -- providers_list_models: id prefixing / name fallback -------------

    #[test]
    fn qualify_model_id_prefixes_a_bare_id_only() {
        assert_eq!(
            qualify_model_id("anthropic", "claude-opus-5".to_string()),
            "anthropic/claude-opus-5"
        );
    }

    #[test]
    fn qualify_model_id_leaves_an_already_qualified_id_untouched() {
        // A custom OpenAI-compatible base URL (e.g. an OpenRouter-style
        // aggregator) can plausibly hand back ids that already carry a
        // provider prefix; those must not become doubly-prefixed.
        assert_eq!(
            qualify_model_id("openrouter", "anthropic/claude-opus-5".to_string()),
            "anthropic/claude-opus-5"
        );
    }

    #[test]
    fn provider_model_name_falls_back_to_the_id_when_no_display_name() {
        // OpenAI-compatible shape: no `display_name` field at all.
        let entry: ModelEntry =
            serde_json::from_str(r#"{"id":"gpt-4o","object":"model","owned_by":"openai"}"#)
                .unwrap();
        let model = provider_model_from_entry("openai", entry);
        assert_eq!(model.id, "openai/gpt-4o");
        assert_eq!(model.name, "gpt-4o");
    }

    #[test]
    fn provider_model_uses_display_name_when_present() {
        // Anthropic shape, with fields (`type`, `created_at`) ModelEntry
        // doesn't model.
        let entry: ModelEntry = serde_json::from_str(
            r#"{"type":"model","id":"claude-opus-5","display_name":"Claude Opus 5","created_at":"2026-01-01T00:00:00Z"}"#,
        )
        .unwrap();
        let model = provider_model_from_entry("anthropic", entry);
        assert_eq!(model.id, "anthropic/claude-opus-5");
        assert_eq!(model.name, "Claude Opus 5");
    }

    #[test]
    fn model_list_deserialize_tolerates_unknown_fields_in_both_shapes() {
        let openai_shape: ModelList = serde_json::from_str(
            r#"{"data":[{"id":"gpt-4o","object":"model","owned_by":"openai","created":123}]}"#,
        )
        .unwrap();
        assert_eq!(openai_shape.data.len(), 1);
        assert_eq!(openai_shape.data[0].id, "gpt-4o");
        assert!(openai_shape.data[0].display_name.is_none());

        let anthropic_shape: ModelList = serde_json::from_str(
            r#"{"data":[{"type":"model","id":"claude-opus-5","display_name":"Claude Opus 5","created_at":"2026-01-01T00:00:00Z"}],"has_more":false}"#,
        )
        .unwrap();
        assert_eq!(anthropic_shape.data.len(), 1);
        assert_eq!(
            anthropic_shape.data[0].display_name.as_deref(),
            Some("Claude Opus 5")
        );
    }
}
