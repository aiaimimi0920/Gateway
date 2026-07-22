// ---------------------------------------------------------------------------
// routing/config.rs — Route configuration store
//
// Loads provider accounts and model-routing rules from YAML or Redis, then
// exposes fast candidate resolution and model listing.
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

use crate::credential_runtime::{KeepaliveConfig, SessionAuthConfig};
use crate::preset::{compile_provider_account, get_builtin_preset, AccountOverrides};
use crate::protocol::registry::{
    canonicalize_protocol_family_key, canonicalize_protocol_profile_key,
    default_protocol_profile_for_preset, infer_protocol_family, infer_protocol_profile,
};
use crate::protocol::search_api::resolve_legacy_search_route_model_alias;
use crate::routing::candidate::{
    canonicalize_adapter_name, ProviderAccountPayload, ProviderExecutionMode, RouteCandidate,
};
use crate::routing::protocol_resolution::surface_supported_wire_protocol_families;

// ---------------------------------------------------------------------------
// Public types used by callers
// ---------------------------------------------------------------------------

/// A compiled provider ready for routing — all preset merging done at load time.
#[derive(Debug, Clone)]
pub struct CompiledProvider {
    pub id: String,
    pub label: String,
    pub payload: ProviderAccountPayload,
    pub protocol_family: String,
    pub protocol_profile: String,
    /// Explicit supported-model list.  Empty means the provider accepts any model.
    pub supported_models: Vec<String>,
    /// Per-provider model name translation.
    /// Key = canonical model name, Value = what this provider actually calls it.
    /// E.g., `{"claude-opus-4-6": "opus4.6"}` if this provider uses a different name.
    pub model_map: HashMap<String, String>,
    /// Credential pool for this provider.
    /// Empty = single credential baked into `payload`.
    /// Non-empty = select one at request time (round-robin).
    pub credential_pool: Vec<CompiledCredential>,
    /// Atomic counter for round-robin credential selection.
    /// Stored in an `Arc` so that cloning `CompiledProvider` shares the counter.
    pub credential_counter: std::sync::Arc<AtomicUsize>,
}

/// A single credential within a provider's credential pool.
/// Contains a complete `ProviderAccountPayload` with the credential's auth
/// merged in — ready to use with no per-request merging needed.
#[derive(Debug, Clone)]
pub struct CompiledCredential {
    pub id: String,
    pub payload: ProviderAccountPayload,
    /// Models this credential can serve. Empty = any model the provider supports.
    pub supported_models: Vec<String>,
    /// OAuth token refresh config. When present, access_token is auto-refreshed.
    pub refresh_config: Option<Arc<TokenRefreshState>>,
}

/// Holds the mutable state for OAuth token refresh.
pub struct TokenRefreshState {
    pub refresh_token: parking_lot::Mutex<String>,
    pub refresh_endpoint: String,
    pub client_id: String,
    pub expires_in_secs: u64,
    pub expires_at: parking_lot::Mutex<std::time::Instant>,
    pub expires_at_iso: parking_lot::Mutex<Option<String>>,
    /// Overrides payload.api_key when a refreshed token is available.
    pub api_key_override: parking_lot::Mutex<Option<String>>,
}

impl std::fmt::Debug for TokenRefreshState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenRefreshState")
            .field("endpoint", &self.refresh_endpoint)
            .field("expires_in_secs", &self.expires_in_secs)
            .finish()
    }
}

pub fn system_time_to_rfc3339_millis(time: std::time::SystemTime) -> String {
    let duration = time
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = duration.as_secs() as i64;
    let millis = duration.subsec_millis();

    let days = total_secs / 86_400;
    let secs_of_day = total_secs % 86_400;
    let hour = secs_of_day / 3_600;
    let minute = (secs_of_day % 3_600) / 60;
    let second = secs_of_day % 60;

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        year, month, day, hour, minute, second, millis
    )
}

pub fn future_rfc3339_after_secs(offset_secs: u64) -> String {
    system_time_to_rfc3339_millis(
        std::time::SystemTime::now() + std::time::Duration::from_secs(offset_secs),
    )
}

/// A model-routing rule: a glob pattern maps to one or more provider IDs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRoute {
    /// Pattern matched against the requested model name.
    /// Supports simple prefix globs: `"claude-*"`, `"gpt-*"`, `"o*"`,
    /// or exact names.  Higher `priority` values take precedence.
    pub pattern: String,
    pub provider_ids: Vec<String>,
    #[serde(default)]
    pub priority: i32,
}

/// Model entry returned by `GET /v1/models`.
#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub id: String,
    /// Always `"model"` (OpenAI compatibility).
    pub object: String,
    pub created: i64,
    pub owned_by: String,
}

// ---------------------------------------------------------------------------
// YAML-serialisable config schema
// ---------------------------------------------------------------------------

/// Top-level YAML configuration document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteConfigYaml {
    pub providers: Vec<ProviderConfigYaml>,
    #[serde(default)]
    pub model_routes: Vec<ModelRoute>,
    /// Model aliases: `"sonnet"` → `"claude-sonnet-4-6"`.
    #[serde(default)]
    pub aliases: HashMap<String, String>,
}

/// A single credential entry within a provider's `credentials` array.
/// Each credential inherits the provider's base config and overrides only
/// credential-specific fields (base_url, api_key, headers, extra_body).
///
/// This allows both:
/// - **Multi-credential**: same endpoint, different auth tokens (e.g. 28 Accio accounts)
/// - **Multi-endpoint**: different base_urls with same or different keys (e.g. 3 AI Hub mirrors)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCredentialYaml {
    /// Optional credential ID (auto-generated as `{provider_id}-cred-{index}` if absent).
    #[serde(default)]
    pub id: Option<String>,
    /// Endpoint URL for this credential (overrides provider's base_url).
    /// Use this when the same provider has multiple mirror/endpoint URLs.
    #[serde(default)]
    pub base_url: Option<String>,
    /// API key for this credential (overrides provider's api_key).
    #[serde(default)]
    pub api_key: Option<String>,
    /// Optional secondary auth token carried by cookie/header-based browser-session providers.
    #[serde(default, alias = "authToken")]
    pub auth_token: Option<String>,
    /// Credential-specific headers (merged on top of provider headers).
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Credential-specific body fields (merged on top of provider extra_body).
    #[serde(default)]
    pub extra_body: HashMap<String, Value>,
    #[serde(default)]
    pub session_auth: Option<SessionAuthConfig>,
    #[serde(default)]
    pub keepalive: Option<KeepaliveConfig>,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub runtime_state_object_key: Option<String>,
    #[serde(default)]
    pub account_name: Option<String>,
    #[serde(default)]
    pub execution_mode: Option<ProviderExecutionMode>,
    #[serde(default)]
    pub endpoint_execution_modes: Option<HashMap<String, ProviderExecutionMode>>,
    /// Models this credential can serve. Empty = inherits provider's supported_models.
    /// When set, the credential is only selected for requests matching these models.
    #[serde(default)]
    pub supported_models: Vec<String>,

    // ── OAuth token refresh ──────────────────────────────────────────────
    /// OAuth refresh_token for auto-refreshing short-lived access_tokens.
    /// When set, the gateway runs a background task that refreshes the
    /// access_token before it expires. The api_key field holds the initial
    /// access_token.
    #[serde(default)]
    pub refresh_token: Option<String>,
    /// OAuth token endpoint for refreshing (e.g., "https://chat.qwen.ai/api/v1/oauth2/token")
    #[serde(default)]
    pub refresh_endpoint: Option<String>,
    /// OAuth client_id for the refresh request.
    #[serde(default)]
    pub refresh_client_id: Option<String>,
    /// Token lifetime in seconds (default: 21600 = 6 hours).
    #[serde(default)]
    pub token_expires_in_secs: Option<u64>,
}

/// Per-provider configuration block in the YAML file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfigYaml {
    pub id: String,
    pub label: Option<String>,
    /// Built-in preset name, e.g. `"codex"`, `"openai"`, `"xfyun"`, or `"anthropic"`.
    pub preset: Option<String>,
    pub base_url: String,
    /// API key. Required when `credentials` is absent; ignored when `credentials`
    /// is present (each credential provides its own auth).
    #[serde(default)]
    pub api_key: String,
    /// Optional secondary auth token carried by cookie/header-based browser-session providers.
    #[serde(default, alias = "authToken")]
    pub auth_token: Option<String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub extra_body: HashMap<String, Value>,
    #[serde(default)]
    pub session_auth: Option<SessionAuthConfig>,
    #[serde(default)]
    pub keepalive: Option<KeepaliveConfig>,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub runtime_state_object_key: Option<String>,
    #[serde(default)]
    pub account_name: Option<String>,
    #[serde(default)]
    pub execution_mode: Option<ProviderExecutionMode>,
    #[serde(default)]
    pub endpoint_execution_modes: Option<HashMap<String, ProviderExecutionMode>>,
    #[serde(default)]
    pub default_model: Option<String>,
    /// Override the adapter name from the preset (rarely needed).
    #[serde(default)]
    pub adapter: Option<String>,
    #[serde(default)]
    pub protocol_family: Option<String>,
    #[serde(default)]
    pub protocol_profile: Option<String>,
    #[serde(default)]
    pub supported_models: Vec<String>,
    // ── Path overrides ───────────────────────────────────────────────────
    #[serde(default)]
    pub responses_path: Option<String>,
    #[serde(default)]
    pub chat_completions_path: Option<String>,
    #[serde(default)]
    pub completions_path: Option<String>,
    #[serde(default)]
    pub embeddings_path: Option<String>,
    #[serde(default)]
    pub audio_transcriptions_path: Option<String>,
    #[serde(default)]
    pub audio_speech_path: Option<String>,
    #[serde(default)]
    pub messages_path: Option<String>,
    #[serde(default)]
    pub search_path: Option<String>,
    #[serde(default)]
    pub fetch_path: Option<String>,
    #[serde(default)]
    pub research_path: Option<String>,
    #[serde(default)]
    pub balance_path: Option<String>,
    #[serde(default)]
    pub search_query_field: Option<String>,
    #[serde(default)]
    pub fetch_urls_field: Option<String>,
    /// Per-provider model name translation: canonical → provider-specific.
    /// E.g., `{"claude-opus-4-6": "opus4.6"}`.
    #[serde(default)]
    pub model_map: HashMap<String, String>,
    /// Optional pool of credentials for this provider.
    /// If present, the provider has multiple credentials that are selected at
    /// request time (round-robin).  If absent, `api_key` is used as a single
    /// credential.
    #[serde(default)]
    pub credentials: Vec<ProviderCredentialYaml>,
}

// ---------------------------------------------------------------------------
// Internal store
// ---------------------------------------------------------------------------

struct RouteConfigInner {
    providers: Vec<CompiledProvider>,
    model_routes: Vec<ModelRoute>,
    /// Exact alias map: "opus" → "claude-opus-4-6"
    aliases: HashMap<String, String>,
    /// Pre-computed normalized alias map (built once at load time).
    /// Key = normalized alias key (no `-._`, lowercase), Value = canonical model name.
    /// Eliminates per-request string allocation during fuzzy matching.
    normalized_aliases: HashMap<String, String>,
    /// Pre-computed sorted normalized keys for prefix matching (built once).
    /// Each entry: (normalized_key, canonical_model_name).
    sorted_normalized_keys: Vec<(String, String)>,
}

/// Thread-safe route config store.  Wrap in [`Arc`] and share across threads.
pub struct RouteConfigStore {
    inner: RwLock<RouteConfigInner>,
}

// ---------------------------------------------------------------------------
// Construction helpers
// ---------------------------------------------------------------------------

/// Perform `${VAR}` environment-variable substitution in a string value.
/// Unknown variables are left as-is.
fn subst_env(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("${") {
        result.push_str(&rest[..start]);
        rest = &rest[start + 2..];
        if let Some(end) = rest.find('}') {
            let var_name = &rest[..end];
            rest = &rest[end + 1..];
            match std::env::var(var_name) {
                Ok(val) => result.push_str(&val),
                Err(_) => {
                    // Leave the placeholder intact so the operator can notice.
                    result.push_str("${");
                    result.push_str(var_name);
                    result.push('}');
                }
            }
        } else {
            // Unclosed `${` — emit literally and stop.
            result.push_str("${");
            result.push_str(rest);
            rest = "";
        }
    }
    result.push_str(rest);
    result
}

/// Apply env substitution to every string field in a `ProviderCredentialYaml`.
fn subst_credential(mut c: ProviderCredentialYaml) -> ProviderCredentialYaml {
    if let Some(ref mut url) = c.base_url {
        *url = subst_env(url);
    }
    if let Some(ref mut key) = c.api_key {
        *key = subst_env(key);
    }
    if let Some(ref mut token) = c.auth_token {
        *token = subst_env(token);
    }
    c.headers = c
        .headers
        .into_iter()
        .map(|(k, v)| (k, subst_env(&v)))
        .collect();
    if let Some(ref mut session) = c.session_auth {
        if let Some(ref mut name) = session.primary_cookie_name {
            *name = subst_env(name);
        }
        if let Some(ref mut name) = session.secondary_cookie_name {
            *name = subst_env(name);
        }
        if let Some(ref mut expires_at) = session.expires_at {
            *expires_at = subst_env(expires_at);
        }
    }
    if let Some(ref mut expires_at) = c.expires_at {
        *expires_at = subst_env(expires_at);
    }
    if let Some(ref mut object_key) = c.runtime_state_object_key {
        *object_key = subst_env(object_key);
    }
    if let Some(ref mut account_name) = c.account_name {
        *account_name = subst_env(account_name);
    }
    if let Some(ref mut keepalive) = c.keepalive {
        keepalive.service_url = subst_env(&keepalive.service_url);
        if let Some(ref mut path) = keepalive.ensure_path {
            *path = subst_env(path);
        }
        if let Some(ref mut token) = keepalive.auth_token {
            *token = subst_env(token);
        }
    }
    c
}

/// Apply env substitution to every string field in a `ProviderConfigYaml`.
fn subst_provider(mut p: ProviderConfigYaml) -> ProviderConfigYaml {
    p.base_url = subst_env(&p.base_url);
    p.api_key = subst_env(&p.api_key);
    if let Some(ref mut token) = p.auth_token {
        *token = subst_env(token);
    }
    let new_headers: HashMap<String, String> = p
        .headers
        .into_iter()
        .map(|(k, v)| (k, subst_env(&v)))
        .collect();
    p.headers = new_headers;
    if let Some(ref mut session) = p.session_auth {
        if let Some(ref mut name) = session.primary_cookie_name {
            *name = subst_env(name);
        }
        if let Some(ref mut name) = session.secondary_cookie_name {
            *name = subst_env(name);
        }
        if let Some(ref mut expires_at) = session.expires_at {
            *expires_at = subst_env(expires_at);
        }
    }
    if let Some(ref mut expires_at) = p.expires_at {
        *expires_at = subst_env(expires_at);
    }
    if let Some(ref mut object_key) = p.runtime_state_object_key {
        *object_key = subst_env(object_key);
    }
    if let Some(ref mut account_name) = p.account_name {
        *account_name = subst_env(account_name);
    }
    if let Some(ref mut keepalive) = p.keepalive {
        keepalive.service_url = subst_env(&keepalive.service_url);
        if let Some(ref mut path) = keepalive.ensure_path {
            *path = subst_env(path);
        }
        if let Some(ref mut token) = keepalive.auth_token {
            *token = subst_env(token);
        }
    }
    p.credentials = p.credentials.into_iter().map(subst_credential).collect();
    p
}

/// Build a `ProviderAccountPayload` from a `ProviderConfigYaml` (with preset
/// merging if applicable).  `api_key_override` lets callers supply a
/// credential-specific key that replaces the provider-level key.
fn build_base_payload(
    cfg: &ProviderConfigYaml,
    base_url_override: Option<&str>,
    api_key_override: Option<&str>,
    auth_token_override: Option<&str>,
    header_overrides: Option<&HashMap<String, String>>,
    extra_body_overrides: Option<&HashMap<String, Value>>,
    session_auth_override: Option<&SessionAuthConfig>,
    keepalive_override: Option<&KeepaliveConfig>,
    expires_at_override: Option<&str>,
    runtime_state_object_key_override: Option<&str>,
    account_name_override: Option<&str>,
    execution_mode_override: Option<ProviderExecutionMode>,
    endpoint_execution_modes_override: Option<&HashMap<String, ProviderExecutionMode>>,
    credential_id: Option<&str>,
) -> Result<ProviderAccountPayload, anyhow::Error> {
    let api_key = api_key_override.unwrap_or(&cfg.api_key).to_string();
    let auth_token = auth_token_override
        .map(str::to_string)
        .or_else(|| cfg.auth_token.clone());

    // Merge headers: provider base, then overrides on top.
    let mut merged_headers = cfg.headers.clone();
    if let Some(overrides) = header_overrides {
        for (k, v) in overrides {
            merged_headers.insert(k.clone(), v.clone());
        }
    }

    // Merge extra_body: provider base, then overrides on top.
    let mut merged_extra_body = cfg.extra_body.clone();
    if let Some(overrides) = extra_body_overrides {
        for (k, v) in overrides {
            merged_extra_body.insert(k.clone(), v.clone());
        }
    }

    if let Some(preset_id) = &cfg.preset {
        let preset = get_builtin_preset(preset_id).ok_or_else(|| {
            anyhow::anyhow!("Unknown preset '{}' for provider '{}'", preset_id, cfg.id)
        })?;

        let overrides = AccountOverrides {
            base_url: base_url_override.unwrap_or(&cfg.base_url).to_string(),
            api_key,
            headers: merged_headers,
            extra_body: merged_extra_body,
            default_model: cfg.default_model.clone(),
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            session_auth: session_auth_override
                .cloned()
                .or_else(|| cfg.session_auth.clone()),
            keepalive: keepalive_override
                .cloned()
                .or_else(|| cfg.keepalive.clone()),
            expires_at: expires_at_override
                .map(str::to_string)
                .or_else(|| cfg.expires_at.clone()),
            runtime_state_object_key: runtime_state_object_key_override
                .map(str::to_string)
                .or_else(|| cfg.runtime_state_object_key.clone()),
            account_name: account_name_override
                .map(str::to_string)
                .or_else(|| cfg.account_name.clone()),
            execution_mode: execution_mode_override.or(cfg.execution_mode),
            endpoint_execution_modes: endpoint_execution_modes_override
                .cloned()
                .or_else(|| cfg.endpoint_execution_modes.clone()),
        };

        let mut compiled = compile_provider_account(&preset, &overrides);
        compiled.credential_id = credential_id.map(str::to_string);

        if let Some(adapter_override) = &cfg.adapter {
            compiled.adapter = canonicalize_adapter_name(adapter_override);
        }
        // YAML path overrides win over preset defaults.
        if cfg.responses_path.is_some() {
            compiled.responses_path = cfg.responses_path.clone();
        }
        if cfg.chat_completions_path.is_some() {
            compiled.chat_completions_path = cfg.chat_completions_path.clone();
        }
        if cfg.completions_path.is_some() {
            compiled.completions_path = cfg.completions_path.clone();
        }
        if cfg.embeddings_path.is_some() {
            compiled.embeddings_path = cfg.embeddings_path.clone();
        }
        if cfg.audio_transcriptions_path.is_some() {
            compiled.audio_transcriptions_path = cfg.audio_transcriptions_path.clone();
        }
        if cfg.audio_speech_path.is_some() {
            compiled.audio_speech_path = cfg.audio_speech_path.clone();
        }
        if cfg.messages_path.is_some() {
            compiled.messages_path = cfg.messages_path.clone();
        }
        if cfg.search_path.is_some() {
            compiled.search_path = cfg.search_path.clone();
        }
        if cfg.fetch_path.is_some() {
            compiled.fetch_path = cfg.fetch_path.clone();
        }
        if cfg.research_path.is_some() {
            compiled.research_path = cfg.research_path.clone();
        }
        if cfg.balance_path.is_some() {
            compiled.balance_path = cfg.balance_path.clone();
        }
        if cfg.search_query_field.is_some() {
            compiled.search_query_field = cfg.search_query_field.clone();
        }
        if cfg.fetch_urls_field.is_some() {
            compiled.fetch_urls_field = cfg.fetch_urls_field.clone();
        }

        Ok(compiled)
    } else {
        let adapter = canonicalize_adapter_name(
            cfg.adapter
                .clone()
                .unwrap_or_else(|| "openai_compatible".to_string())
                .as_str(),
        );

        let extra_body_opt = if merged_extra_body.is_empty() {
            None
        } else {
            Some(merged_extra_body)
        };

        Ok(ProviderAccountPayload {
            adapter,
            base_url: base_url_override.unwrap_or(&cfg.base_url).to_string(),
            api_key,
            credential_id: credential_id.map(str::to_string),
            expires_at: expires_at_override
                .map(str::to_string)
                .or_else(|| cfg.expires_at.clone()),
            runtime_state_object_key: runtime_state_object_key_override
                .map(str::to_string)
                .or_else(|| cfg.runtime_state_object_key.clone()),
            account_name: account_name_override
                .map(str::to_string)
                .or_else(|| cfg.account_name.clone()),
            execution_mode: execution_mode_override.or(cfg.execution_mode),
            endpoint_execution_modes: endpoint_execution_modes_override
                .cloned()
                .or_else(|| cfg.endpoint_execution_modes.clone()),
            default_model: cfg.default_model.clone(),
            headers: merged_headers,
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token,
            responses_path: cfg.responses_path.clone(),
            chat_completions_path: cfg.chat_completions_path.clone(),
            completions_path: cfg.completions_path.clone(),
            embeddings_path: cfg.embeddings_path.clone(),
            audio_transcriptions_path: cfg.audio_transcriptions_path.clone(),
            audio_speech_path: cfg.audio_speech_path.clone(),
            messages_path: cfg.messages_path.clone(),
            search_path: cfg.search_path.clone(),
            fetch_path: cfg.fetch_path.clone(),
            research_path: cfg.research_path.clone(),
            balance_path: cfg.balance_path.clone(),
            search_query_field: cfg.search_query_field.clone(),
            fetch_urls_field: cfg.fetch_urls_field.clone(),
            extra_body: extra_body_opt,
            session_auth: session_auth_override
                .cloned()
                .or_else(|| cfg.session_auth.clone()),
            keepalive: keepalive_override
                .cloned()
                .or_else(|| cfg.keepalive.clone()),
        })
    }
}

/// Compile a `ProviderConfigYaml` into a `CompiledProvider`.
///
/// If a `preset` name is given, we look it up in the built-in registry and
/// call `compile_provider_account`.  Otherwise we build the payload directly
/// from the YAML fields.
///
/// When `credentials` is non-empty, builds a credential pool where each
/// credential has a fully-merged payload.  The provider's `payload` field
/// contains the base (first credential or provider-level key), and
/// `credential_pool` stores all credential payloads for round-robin selection.
fn compile_provider(cfg: ProviderConfigYaml) -> Result<CompiledProvider, anyhow::Error> {
    let label = cfg.label.clone().unwrap_or_else(|| cfg.id.clone());

    // Build the base payload (used as-is when no credential pool exists).
    let payload = build_base_payload(
        &cfg, None, None, None, None, None, None, None, None, None, None, None, None, None,
    )?;

    // Derive protocol_family from adapter name if not explicitly set.
    let protocol_family = infer_protocol_family(
        cfg.protocol_family.as_deref(),
        &payload.adapter,
        cfg.protocol_profile
            .as_deref()
            .or(cfg.preset.as_deref())
            .or(cfg.label.as_deref())
            .or(Some(cfg.id.as_str())),
        Some(payload.base_url.as_str()),
    );
    let protocol_profile = cfg
        .protocol_profile
        .as_deref()
        .map(canonicalize_protocol_profile_key)
        .unwrap_or_else(|| {
            cfg.preset
                .as_deref()
                .map(default_protocol_profile_for_preset)
                .map(str::to_string)
                .unwrap_or_else(|| {
                    infer_protocol_profile(
                        &payload.adapter,
                        cfg.preset
                            .as_deref()
                            .or(cfg.label.as_deref())
                            .or(Some(cfg.id.as_str())),
                        Some(payload.base_url.as_str()),
                    )
                })
        });

    // Build credential pool if `credentials` array is present.
    let credential_pool = if cfg.credentials.is_empty() {
        Vec::new()
    } else {
        cfg.credentials
            .iter()
            .enumerate()
            .map(|(idx, cred)| {
                let cred_id = cred
                    .id
                    .clone()
                    .unwrap_or_else(|| format!("{}-cred-{}", cfg.id, idx));

                let cred_payload = build_base_payload(
                    &cfg,
                    cred.base_url.as_deref(),
                    cred.api_key.as_deref(),
                    cred.auth_token.as_deref(),
                    if cred.headers.is_empty() {
                        None
                    } else {
                        Some(&cred.headers)
                    },
                    if cred.extra_body.is_empty() {
                        None
                    } else {
                        Some(&cred.extra_body)
                    },
                    cred.session_auth.as_ref(),
                    cred.keepalive.as_ref(),
                    cred.expires_at.as_deref(),
                    cred.runtime_state_object_key.as_deref(),
                    cred.account_name.as_deref(),
                    cred.execution_mode,
                    cred.endpoint_execution_modes.as_ref(),
                    Some(&cred_id),
                )?;

                // Build OAuth refresh config if refresh_token is provided.
                let refresh_config = match (&cred.refresh_token, &cred.refresh_endpoint) {
                    (Some(rt), Some(ep)) => Some(Arc::new(TokenRefreshState {
                        refresh_token: parking_lot::Mutex::new(rt.clone()),
                        refresh_endpoint: ep.clone(),
                        client_id: cred.refresh_client_id.clone().unwrap_or_default(),
                        expires_in_secs: cred.token_expires_in_secs.unwrap_or(21600),
                        expires_at: parking_lot::Mutex::new(
                            std::time::Instant::now()
                                + std::time::Duration::from_secs(
                                    cred.token_expires_in_secs.unwrap_or(21600),
                                ),
                        ),
                        expires_at_iso: parking_lot::Mutex::new(Some(future_rfc3339_after_secs(
                            cred.token_expires_in_secs.unwrap_or(21600),
                        ))),
                        api_key_override: parking_lot::Mutex::new(None),
                    })),
                    _ => None,
                };

                Ok(CompiledCredential {
                    id: cred_id,
                    payload: cred_payload,
                    supported_models: cred.supported_models.clone(),
                    refresh_config,
                })
            })
            .collect::<Result<Vec<_>, anyhow::Error>>()?
    };

    Ok(CompiledProvider {
        id: cfg.id,
        label,
        payload,
        protocol_family: canonicalize_protocol_family_key(&protocol_family),
        protocol_profile,
        supported_models: cfg.supported_models,
        model_map: cfg.model_map,
        credential_pool,
        credential_counter: std::sync::Arc::new(AtomicUsize::new(0)),
    })
}

/// Convert a `RouteConfigYaml` into a `RouteConfigInner`.
fn compile_yaml(config: RouteConfigYaml) -> Result<RouteConfigInner, anyhow::Error> {
    let providers: Result<Vec<CompiledProvider>, _> = config
        .providers
        .into_iter()
        .map(|p| compile_provider(subst_provider(p)))
        .collect();

    let aliases = config.aliases;

    // Pre-compute normalized alias maps at load time (zero per-request allocation).
    let normalized_aliases: HashMap<String, String> = aliases
        .iter()
        .map(|(k, v)| (normalize_model_name(k), v.clone()))
        .collect();

    let mut sorted_normalized_keys: Vec<(String, String)> = aliases
        .iter()
        .map(|(k, v)| (normalize_model_name(k), v.clone()))
        .collect();
    sorted_normalized_keys.sort_by(|a, b| a.0.cmp(&b.0));

    Ok(RouteConfigInner {
        providers: providers?,
        model_routes: config.model_routes,
        aliases,
        normalized_aliases,
        sorted_normalized_keys,
    })
}

// ---------------------------------------------------------------------------
// Glob pattern matching
// ---------------------------------------------------------------------------

/// Returns `true` if `model` matches `pattern`.
///
/// Pattern rules (evaluated in order):
/// 1. `"*"` — matches everything.
/// 2. If the pattern ends with `"*"` → prefix match on `pattern[..len-1]`.
/// 3. If the pattern starts with `"*"` → suffix match on `pattern[1..]`.
/// 4. Otherwise → exact match.
fn glob_match(pattern: &str, model: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return model.starts_with(prefix);
    }
    if let Some(suffix) = pattern.strip_prefix('*') {
        return model.ends_with(suffix);
    }
    pattern == model
}

/// Normalize a model name for fuzzy alias matching.
/// Strips `-`, `.`, `_`, and lowercases.
/// E.g., `"claude-opus-4-6"` → `"claudeopus46"`,
///       `"opus4.6"` → `"opus46"`,
///       `"Opus4-6"` → `"opus46"`.
fn normalize_model_name(name: &str) -> String {
    name.chars()
        .filter(|c| *c != '-' && *c != '.' && *c != '_')
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn resolve_alias_inner(guard: &RouteConfigInner, model: &str) -> Option<String> {
    if let Some(target) = guard.aliases.get(model) {
        return Some(target.clone());
    }

    if let Some(target) = resolve_legacy_search_route_model_alias(model) {
        return Some(target.to_string());
    }

    let norm_input = normalize_model_name(model);
    if let Some(target) = guard.normalized_aliases.get(&norm_input) {
        return Some(target.clone());
    }

    if norm_input.len() >= 3 {
        let keys = &guard.sorted_normalized_keys;
        let start = keys.partition_point(|(k, _)| k.as_str() < norm_input.as_str());
        let mut best: Option<&str> = None;
        for (k, v) in &keys[start..] {
            if k.starts_with(&norm_input) {
                if best.is_none() {
                    best = Some(v.as_str());
                }
            } else {
                break;
            }
        }
        if let Some(target) = best {
            return Some(target.to_string());
        }
    }

    None
}

// ---------------------------------------------------------------------------
// RouteConfigStore public API
// ---------------------------------------------------------------------------

impl RouteConfigStore {
    // ── Constructors ─────────────────────────────────────────────────────────

    /// Create an empty store (no providers, no routes).
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(RouteConfigInner {
                providers: Vec::new(),
                model_routes: Vec::new(),
                aliases: HashMap::new(),
                normalized_aliases: HashMap::new(),
                sorted_normalized_keys: Vec::new(),
            }),
        }
    }

    /// Load from a YAML file at `path`.
    ///
    /// Returns `Err` if the file cannot be read or parsed, or if any provider
    /// references an unknown preset.
    pub fn load_from_yaml(path: impl AsRef<std::path::Path>) -> Result<Self, anyhow::Error> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Cannot read route config '{}': {}", path.display(), e))?;
        let config: RouteConfigYaml = serde_yaml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("YAML parse error in '{}': {}", path.display(), e))?;
        let inner = compile_yaml(config)?;
        Ok(Self {
            inner: RwLock::new(inner),
        })
    }

    /// Load from Redis (key `gw:config:routes`), where the value is a
    /// JSON-encoded `RouteConfigYaml`.
    pub async fn load_from_redis(pool: &deadpool_redis::Pool) -> Result<Self, anyhow::Error> {
        use redis::AsyncCommands;

        let mut conn = pool
            .get()
            .await
            .map_err(|e| anyhow::anyhow!("Redis pool error: {}", e))?;

        let raw: String = conn
            .get("gw:config:routes")
            .await
            .map_err(|e| anyhow::anyhow!("Redis GET gw:config:routes: {}", e))?;

        let config: RouteConfigYaml = serde_json::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("JSON parse error from Redis: {}", e))?;

        let inner = compile_yaml(config)?;
        Ok(Self {
            inner: RwLock::new(inner),
        })
    }

    // ── Query API ─────────────────────────────────────────────────────────────

    /// Resolve the canonical model name through the alias map with smart
    /// fuzzy matching.
    ///
    /// Resolution strategy (in order):
    /// 1. Exact match: `"opus"` → aliases["opus"] — O(1) HashMap lookup
    /// 2. Normalized match: pre-computed normalized_aliases lookup — O(1)
    /// 3. Prefix match: binary search on sorted_normalized_keys — O(log N)
    ///
    /// **Zero heap allocation per request** — all normalization is done at
    /// config load time. The only allocation is the cloned result String.
    pub fn resolve_alias(&self, model: Option<&str>) -> Option<String> {
        let model = model?;
        let guard = self.inner.read();
        resolve_alias_inner(&guard, model)
    }

    /// Resolve the ordered list of [`RouteCandidate`]s for `model`.
    ///
    /// Resolution algorithm:
    /// 1. Resolve the model name through the alias map.
    /// 2. Find all `model_routes` whose pattern matches the resolved model
    ///    (glob match).
    /// 3. Sort matching routes by `priority` descending (highest first).
    /// 4. Collect the provider IDs from those routes in priority order,
    ///    deduplicating while preserving order.
    /// 5. Map each provider ID to a `RouteCandidate`.
    /// 6. If no routes matched, prefer providers whose non-empty
    ///    `supported_models` exactly contains the resolved model.
    /// 7. If no provider explicitly supports the model, fall back to ALL
    ///    configured providers.
    /// 8. If `model` is `None`, return ALL providers.
    pub fn resolve_candidates(&self, model: Option<&str>) -> Vec<RouteCandidate> {
        let guard = self.inner.read();

        let provider_map: HashMap<&str, &CompiledProvider> =
            guard.providers.iter().map(|p| (p.id.as_str(), p)).collect();

        // Step 1: resolve alias
        let resolved_alias = model.and_then(|m| resolve_alias_inner(&guard, m));
        let alias_used = match (model, resolved_alias.as_deref()) {
            (Some(original), Some(resolved)) if resolved != original => Some(original),
            _ => None,
        };
        let resolved = match (resolved_alias.as_deref(), model) {
            (Some(target), _) => Some(target),
            (None, some_model) => some_model,
        };

        // Struct to carry provider id + priority from matched routes.
        struct MatchedProvider<'a> {
            id: &'a str,
            priority: i32,
        }

        let matched: Vec<MatchedProvider> = match resolved {
            None => {
                // No model specified → return all providers.
                guard
                    .providers
                    .iter()
                    .map(|p| MatchedProvider {
                        id: p.id.as_str(),
                        priority: 10,
                    })
                    .collect()
            }
            Some(m) => {
                // Collect matching routes sorted by priority descending.
                let mut matching: Vec<&ModelRoute> = guard
                    .model_routes
                    .iter()
                    .filter(|r| glob_match(&r.pattern, m))
                    .collect();

                if matching.is_empty() {
                    let explicitly_supported: Vec<MatchedProvider> = guard
                        .providers
                        .iter()
                        .filter(|p| {
                            let upstream_model = p.model_map.get(m).map(String::as_str);
                            !p.supported_models.is_empty()
                                && p.supported_models.iter().any(|supported| {
                                    supported == m || upstream_model == Some(supported.as_str())
                                })
                        })
                        .map(|p| MatchedProvider {
                            id: p.id.as_str(),
                            priority: 10,
                        })
                        .collect();

                    if explicitly_supported.is_empty() {
                        // No explicit provider support → preserve the broad fallback.
                        guard
                            .providers
                            .iter()
                            .map(|p| MatchedProvider {
                                id: p.id.as_str(),
                                priority: 10,
                            })
                            .collect()
                    } else {
                        explicitly_supported
                    }
                } else {
                    matching.sort_by(|a, b| b.priority.cmp(&a.priority));

                    // Flatten provider IDs, deduplicate preserving order.
                    let mut seen = std::collections::HashSet::new();
                    let mut result: Vec<MatchedProvider> = Vec::new();
                    for route in matching {
                        for id in &route.provider_ids {
                            if seen.insert(id.as_str()) {
                                result.push(MatchedProvider {
                                    id: id.as_str(),
                                    priority: route.priority,
                                });
                            }
                        }
                    }
                    result
                }
            }
        };

        // Convert to RouteCandidate; silently skip unknown provider IDs.
        matched
            .into_iter()
            .filter_map(|mp| {
                provider_map
                    .get(mp.id)
                    .map(|p| provider_to_candidate(p, resolved, alias_used, mp.priority))
            })
            .collect()
    }

    /// List all models available through this gateway (for `GET /v1/models`).
    ///
    /// Derived from:
    /// - Explicit `supported_models` on each provider.
    /// - Patterns from `model_routes` that are exact names (no `*`).
    /// - Alias keys (the short name) and their resolved targets.
    pub fn list_models(&self) -> Vec<ModelInfo> {
        let guard = self.inner.read();

        let mut model_ids: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

        // Provider supported_models lists.
        for p in &guard.providers {
            for m in &p.supported_models {
                model_ids.insert(m.clone());
            }
        }

        // Exact-match route patterns (no wildcard).
        for r in &guard.model_routes {
            if !r.pattern.contains('*') {
                model_ids.insert(r.pattern.clone());
            }
        }

        // Alias keys are the platform-facing names users should request
        // directly. Do not also expose alias targets here because they may be
        // provider-native model identifiers.
        for alias in guard.aliases.keys() {
            model_ids.insert(alias.clone());
        }

        let created = created_timestamp();

        model_ids
            .into_iter()
            .map(|id| ModelInfo {
                object: "model".to_string(),
                owned_by: "neuro-gateway".to_string(),
                created,
                id,
            })
            .collect()
    }

    /// Returns `true` if at least one provider is configured.
    pub fn has_routes(&self) -> bool {
        !self.inner.read().providers.is_empty()
    }

    /// Number of configured providers.
    pub fn provider_count(&self) -> usize {
        self.inner.read().providers.len()
    }

    /// Get a snapshot of all providers (for token refresh task).
    pub fn get_providers(&self) -> Vec<CompiledProvider> {
        self.inner.read().providers.clone()
    }
}

impl Default for RouteConfigStore {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Select a credential's payload from the pool using model-aware round-robin.
///
/// If credentials have `supported_models`, only those matching the requested
/// model are eligible. This handles providers like xfyun where different
/// appIds are authorized for different models.
///
/// If the pool is empty, returns the provider's base payload.
/// Select a credential from the pool that supports the requested model.
///
/// `original_model` is the user-facing name (e.g. "qwen3.5-35b-a3b").
/// `translated_model` is the model_map-translated upstream name (e.g. "astron-code-latest").
///
/// A credential matches if its `supported_models` contains EITHER name.
/// This allows credentials to list user-facing names (xfyun-coding) or
/// upstream names (xfyun-maas) — both patterns work.
fn select_credential(
    provider: &CompiledProvider,
    original_model: Option<&str>,
    translated_model: Option<&str>,
) -> ProviderAccountPayload {
    if provider.credential_pool.is_empty() {
        return provider.payload.clone();
    }

    // Filter credentials that support the requested model.
    // Match against both the original (user-facing) and translated (upstream) name.
    let eligible: Vec<usize> = provider
        .credential_pool
        .iter()
        .enumerate()
        .filter(|(_, cred)| {
            if cred.supported_models.is_empty() {
                true // no restriction = supports all provider models
            } else if original_model.is_none() && translated_model.is_none() {
                true // no model specified = any credential
            } else {
                // Match if credential supports either the original or translated name
                let matches_original = original_model
                    .map(|m| cred.supported_models.iter().any(|sm| sm == m))
                    .unwrap_or(false);
                let matches_translated = translated_model
                    .map(|m| cred.supported_models.iter().any(|sm| sm == m))
                    .unwrap_or(false);
                matches_original || matches_translated
            }
        })
        .map(|(i, _)| i)
        .collect();

    if eligible.is_empty() {
        // No credential supports this model — fall back to round-robin over all
        let idx = provider.credential_counter.fetch_add(1, Ordering::Relaxed)
            % provider.credential_pool.len();
        return apply_token_override(&provider.credential_pool[idx]);
    }

    // Round-robin among eligible credentials
    let counter = provider.credential_counter.fetch_add(1, Ordering::Relaxed);
    let idx = eligible[counter % eligible.len()];
    apply_token_override(&provider.credential_pool[idx])
}

/// Apply OAuth token override if a refreshed access_token is available.
fn apply_token_override(cred: &CompiledCredential) -> ProviderAccountPayload {
    let mut payload = cred.payload.clone();
    if let Some(ref config) = cred.refresh_config {
        if let Some(ref override_key) = *config.api_key_override.lock() {
            payload.api_key = override_key.clone();
        }
        if let Some(ref expiry) = *config.expires_at_iso.lock() {
            if let Some(session_auth) = payload.session_auth.as_mut() {
                session_auth.expires_at = Some(expiry.clone());
            }
        }
    }
    payload
}

/// Convert a `CompiledProvider` to a `RouteCandidate`.
///
/// `model` is the resolved model name (after alias resolution) to send
/// upstream; `alias` is the original alias if one was used.
///
/// When the provider has a credential pool, the candidate's `payload` is set
/// to a round-robin-selected credential, but `provider_account_id` always
/// references the provider — one AIMD controller for the whole pool.
fn provider_to_candidate(
    p: &CompiledProvider,
    model: Option<&str>,
    alias: Option<&str>,
    priority: i32,
) -> RouteCandidate {
    // Select the payload — either base or a pooled credential.
    // Model-aware: only picks credentials authorized for the requested model.
    //
    // We pass BOTH the original (user-facing) and translated (upstream) model
    // names so credential matching works regardless of which naming scheme the
    // credential's `supported_models` list uses.
    //
    // Example 1 — xfyun-maas: user requests "hunyuan-mt-7b", model_map → "xophunyuan7bmt",
    //   credential lists "xophunyuan7bmt" → matches via translated name.
    // Example 2 — xfyun-coding: user requests "qwen3.5-35b-a3b", model_map → "astron-code-latest",
    //   credential lists "qwen3.5-35b-a3b" → matches via original name.
    let translated_model = model.and_then(|m| p.model_map.get(m).map(|s| s.as_str()));
    let selected_payload = select_credential(p, model, translated_model);

    // Apply per-provider model name translation.
    // If the provider has a model_map entry for this canonical model name,
    // use the provider-specific name for the upstream call.
    let upstream_model =
        model.map(|m| p.model_map.get(m).cloned().unwrap_or_else(|| m.to_string()));
    let resolved_execution_mode = selected_payload
        .resolve_execution_mode(crate::protocol::canonical::EndpointKind::ChatCompletions);

    RouteCandidate {
        provider_account_id: p.id.clone(),
        provider_credential_id: None,
        label: p.label.clone(),
        adapter: selected_payload.adapter.clone(),
        protocol_family: p.protocol_family.clone(),
        protocol_profile: p.protocol_profile.clone(),
        supported_protocol_families: surface_supported_wire_protocol_families(
            &selected_payload.adapter,
            &p.protocol_family,
        ),
        payload: selected_payload,
        model_alias: alias.map(|s| s.to_string()),
        upstream_model,
        resolved_execution_mode,
        priority,
        weight: 100,
        failure_count: 0,
        cooldown_until: None,
        routing_score: None,
        routing_health_weight: None,
        routing_capacity_weight: None,
        routing_degraded: None,
        routing_breaker_open: None,
        routing_degradation_reasons: Vec::new(),
    }
}

/// A fixed "created" timestamp (2026-01-01T00:00:00Z as UNIX seconds).
/// We use a constant rather than the current time so the response is stable.
fn created_timestamp() -> i64 {
    1_767_225_600 // 2026-01-01T00:00:00Z
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::registry::{LINKUP_SEARCH_FAMILY, PERPLEXITY_SEARCH_FAMILY};
    use crate::protocol::search_api::{
        BALANCE_ROUTE_MODEL, FETCH_ROUTE_MODEL, LEGACY_BALANCE_ROUTE_MODEL,
        LEGACY_FETCH_ROUTE_MODEL, LEGACY_RESEARCH_ROUTE_MODEL, LEGACY_SEARCH_ROUTE_MODEL,
        RESEARCH_ROUTE_MODEL, SEARCH_ROUTE_MODEL,
    };
    use crate::routing::candidate::SEARCH_API_COMPATIBLE_ADAPTER;
    use serde_json::json;

    // ── glob_match ────────────────────────────────────────────────────────────

    #[test]
    fn glob_star_matches_everything() {
        assert!(glob_match("*", "anything"));
        assert!(glob_match("*", ""));
    }

    #[test]
    fn glob_prefix_star_matches_suffix() {
        assert!(glob_match("claude-*", "claude-sonnet-4-6"));
        assert!(glob_match("claude-*", "claude-opus-4"));
        assert!(!glob_match("claude-*", "gpt-4o"));
        assert!(!glob_match("claude-*", "claude"));
    }

    #[test]
    fn glob_suffix_star_matches_prefix() {
        assert!(glob_match("*-mini", "gpt-4o-mini"));
        assert!(!glob_match("*-mini", "gpt-4o"));
    }

    #[test]
    fn glob_exact_match() {
        assert!(glob_match("gpt-4o", "gpt-4o"));
        assert!(!glob_match("gpt-4o", "gpt-4o-mini"));
    }

    #[test]
    fn glob_o_star_matches_o_models() {
        assert!(glob_match("o*", "o1"));
        assert!(glob_match("o*", "o3-mini"));
        assert!(!glob_match("o*", "gpt-o1"));
    }

    // ── subst_env ─────────────────────────────────────────────────────────────

    #[test]
    fn subst_env_replaces_known_var() {
        std::env::set_var("TEST_GW_KEY", "hello-world");
        let result = subst_env("Bearer ${TEST_GW_KEY}");
        assert_eq!(result, "Bearer hello-world");
        std::env::remove_var("TEST_GW_KEY");
    }

    #[test]
    fn subst_env_leaves_unknown_var_intact() {
        let result = subst_env("${DEFINITELY_NOT_SET_XYZ}");
        assert_eq!(result, "${DEFINITELY_NOT_SET_XYZ}");
    }

    #[test]
    fn subst_env_no_placeholders_unchanged() {
        let s = "https://api.openai.com";
        assert_eq!(subst_env(s), s);
    }

    // ── RouteConfigStore::new ─────────────────────────────────────────────────

    #[test]
    fn new_store_is_empty() {
        let store = RouteConfigStore::new();
        assert!(!store.has_routes());
        assert_eq!(store.provider_count(), 0);
        assert!(store.resolve_candidates(None).is_empty());
        assert!(store.list_models().is_empty());
    }

    // ── YAML parsing ──────────────────────────────────────────────────────────

    fn make_store_from_yaml(yaml: &str) -> RouteConfigStore {
        let config: RouteConfigYaml = serde_yaml::from_str(yaml).expect("parse yaml");
        let inner = compile_yaml(config).expect("compile");
        RouteConfigStore {
            inner: RwLock::new(inner),
        }
    }

    #[test]
    fn yaml_parsing_no_preset() {
        let yaml = r#"
providers:
  - id: my-openai
    base_url: "https://api.openai.com"
    api_key: "sk-test"
    supported_models:
      - gpt-4o
model_routes: []
aliases: {}
"#;
        let store = make_store_from_yaml(yaml);
        assert!(store.has_routes());
        assert_eq!(store.provider_count(), 1);
    }

    #[test]
    fn yaml_parsing_with_openai_preset() {
        let yaml = r#"
providers:
  - id: openai-default
    preset: openai
    base_url: "https://api.openai.com"
    api_key: "sk-test"
model_routes: []
"#;
        let store = make_store_from_yaml(yaml);
        assert_eq!(store.provider_count(), 1);
        let candidates = store.resolve_candidates(None);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].payload.adapter, "openai_compatible");
    }

    #[test]
    fn yaml_parsing_with_nvidia_openai_preset() {
        let yaml = r#"
providers:
  - id: nvidia-live
    preset: nvidia-openai
    base_url: "https://integrate.api.nvidia.com"
    api_key: "nvapi-test"
    supported_models: [meta/llama-3.3-70b-instruct]
model_routes:
  - pattern: "nvidia-*"
    provider_ids: [nvidia-live]
    priority: 10
"#;
        let store = make_store_from_yaml(yaml);
        let candidates = store.resolve_candidates(Some("nvidia-fixture"));
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].payload.adapter, "openai_compatible");
    }

    #[test]
    fn yaml_parsing_with_linkup_preset() {
        let yaml = r#"
providers:
  - id: linkup-main
    preset: linkup
    base_url: "https://api.linkup.so"
    api_key: "linkup-test"
    supported_models: [linkup-search]
model_routes:
  - pattern: "linkup-*"
    provider_ids: [linkup-main]
    priority: 10
"#;
        let store = make_store_from_yaml(yaml);
        let candidates = store.resolve_candidates(Some("linkup-search"));
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].payload.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
        assert_eq!(
            candidates[0].payload.search_path.as_deref(),
            Some("/v1/search")
        );
        assert_eq!(
            candidates[0].payload.fetch_path.as_deref(),
            Some("/v1/fetch")
        );
        assert_eq!(
            candidates[0].payload.research_path.as_deref(),
            Some("/v1/research")
        );
        assert_eq!(
            candidates[0].payload.balance_path.as_deref(),
            Some("/v1/credits/balance")
        );
        assert_eq!(candidates[0].protocol_family, LINKUP_SEARCH_FAMILY);
    }

    #[test]
    fn yaml_parsing_with_producer_preset() {
        let yaml = r#"
providers:
  - id: producer-main
    preset: producer
    base_url: "https://www.flowmusic.app"
    api_key: "producer-session"
    supported_models: ["producer:image", "producer:standard", "producer:music-video"]
model_routes:
  - pattern: "producer:*"
    provider_ids: [producer-main]
    priority: 10
"#;
        let store = make_store_from_yaml(yaml);
        let image_candidates = store.resolve_candidates(Some("producer:image"));
        assert_eq!(image_candidates.len(), 1);
        assert_eq!(image_candidates[0].payload.adapter, "producer_compatible");
        let candidates = store.resolve_candidates(Some("producer:standard"));
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].payload.adapter, "producer_compatible");
        assert_eq!(
            candidates[0]
                .payload
                .session_auth
                .as_ref()
                .map(|cfg| cfg.transport.as_str()),
            Some("bearer")
        );
        let video_candidates = store.resolve_candidates(Some("producer:music-video"));
        assert_eq!(video_candidates.len(), 1);
        assert_eq!(video_candidates[0].payload.adapter, "producer_compatible");
    }

    #[test]
    fn yaml_parsing_with_udio_preset() {
        let yaml = r#"
providers:
  - id: udio-main
    preset: udio
    base_url: "https://www.udio.com"
    api_key: "udio-session"
    supported_models: [udio-music]
model_routes:
  - pattern: "udio-*"
    provider_ids: [udio-main]
    priority: 10
"#;
        let store = make_store_from_yaml(yaml);
        let candidates = store.resolve_candidates(Some("udio-music"));
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].payload.adapter, "udio_compatible");
        assert_eq!(candidates[0].protocol_family, "udio_music");
        assert_eq!(
            candidates[0]
                .payload
                .session_auth
                .as_ref()
                .map(|cfg| cfg.transport.as_str()),
            Some("cookie")
        );
        assert_eq!(
            candidates[0]
                .payload
                .session_auth
                .as_ref()
                .map(|cfg| cfg.primary_cookie_name()),
            Some("sb-ssr-production-auth-token")
        );
    }

    #[test]
    fn yaml_parsing_with_perplexity_preset() {
        let yaml = r#"
providers:
  - id: perplexity-main
    preset: perplexity
    base_url: "https://api.perplexity.ai"
    api_key: "pplx-test"
    supported_models: [sonar]
model_routes:
  - pattern: "sonar*"
    provider_ids: [perplexity-main]
    priority: 10
"#;
        let store = make_store_from_yaml(yaml);
        let candidates = store.resolve_candidates(Some("sonar"));
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].payload.adapter, "openai_compatible");
        assert_eq!(
            candidates[0].payload.chat_completions_path.as_deref(),
            Some("/chat/completions")
        );
        assert_eq!(
            candidates[0].payload.responses_path.as_deref(),
            Some("/v1/responses")
        );
        assert_eq!(candidates[0].protocol_family, "openai");
    }

    #[test]
    fn yaml_parsing_with_perplexity_search_preset() {
        let yaml = r#"
providers:
  - id: perplexity-search-main
    preset: perplexity-search
    base_url: "https://api.perplexity.ai"
    api_key: "pplx-search-test"
    supported_models: [perplexity-search]
model_routes:
  - pattern: "perplexity-search"
    provider_ids: [perplexity-search-main]
    priority: 10
"#;
        let store = make_store_from_yaml(yaml);
        let candidates = store.resolve_candidates(Some("perplexity-search"));
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].payload.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
        assert_eq!(
            candidates[0].payload.search_path.as_deref(),
            Some("/search")
        );
        assert_eq!(
            candidates[0].payload.search_query_field.as_deref(),
            Some("query")
        );
        assert_eq!(candidates[0].protocol_family, PERPLEXITY_SEARCH_FAMILY);
    }

    #[test]
    fn yaml_legacy_search_adapter_alias_is_canonicalized() {
        let yaml = r#"
providers:
  - id: legacy-search
    adapter: linkup_compatible
    protocol_family: linkup
    base_url: "https://api.example.com"
    api_key: "legacy-key"
    search_path: "/search"
model_routes:
  - pattern: "legacy-search"
    provider_ids: [legacy-search]
    priority: 10
"#;
        let store = make_store_from_yaml(yaml);
        let candidates = store.resolve_candidates(Some("legacy-search"));
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].payload.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
        assert_eq!(candidates[0].adapter, SEARCH_API_COMPATIBLE_ADAPTER);
        assert_eq!(
            candidates[0].payload.search_path.as_deref(),
            Some("/search")
        );
        assert_eq!(candidates[0].protocol_family, LINKUP_SEARCH_FAMILY);
    }

    #[test]
    fn yaml_parsing_with_anthropic_preset() {
        let yaml = r#"
providers:
  - id: anthropic-default
    preset: anthropic
    base_url: "https://api.anthropic.com"
    api_key: "sk-ant-test"
model_routes: []
"#;
        let store = make_store_from_yaml(yaml);
        let candidates = store.resolve_candidates(None);
        assert_eq!(candidates[0].payload.adapter, "anthropic_compatible");
        assert_eq!(candidates[0].protocol_family, "anthropic");
    }

    #[test]
    fn yaml_parsing_with_codex_preset() {
        let yaml = r#"
providers:
  - id: codex-main
    preset: codex
    base_url: "https://chatgpt.com/backend-api/codex"
    api_key: "tok_xxx"
    headers:
      Chatgpt-Account-Id: "acc-123"
model_routes: []
"#;
        let store = make_store_from_yaml(yaml);
        let candidates = store.resolve_candidates(None);
        assert_eq!(candidates[0].payload.adapter, "openai_compatible");
        assert!(candidates[0]
            .payload
            .headers
            .contains_key("Chatgpt-Account-Id"));
        assert!(candidates[0].payload.headers.contains_key("User-Agent"));
    }

    #[test]
    fn yaml_parsing_with_xfyun_preset() {
        let yaml = r#"
providers:
  - id: xfyun-platform
    label: "xfyun platform"
    preset: xfyun
    base_url: "https://maas-api.cn-huabei-1.xf-yun.com"
    api_key: "xf-test"
    supported_models: [xophunyuan7bmt, xophunyuanocr]
    model_map:
      hunyuan-mt-7b: "xophunyuan7bmt"
      hunyuan-ocr: "xophunyuanocr"
model_routes:
  - pattern: "hunyuan-*"
    provider_ids: [xfyun-platform]
    priority: 10
"#;
        let store = make_store_from_yaml(yaml);
        let candidates = store.resolve_candidates(Some("hunyuan-mt-7b"));
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].label, "xfyun platform");
        assert_eq!(candidates[0].payload.adapter, "openai_compatible");
        assert_eq!(
            candidates[0].payload.chat_completions_path.as_deref(),
            Some("/v2/chat/completions")
        );
        assert_eq!(candidates[0].protocol_family, "openai");
        assert_eq!(
            candidates[0].upstream_model.as_deref(),
            Some("xophunyuan7bmt")
        );
    }

    #[test]
    fn yaml_parsing_with_xfyun_websocket_preset() {
        let yaml = r#"
providers:
  - id: xfyun-ws-platform
    label: "xfyun websocket"
    preset: xfyun-websocket
    base_url: "wss://maas-api.cn-huabei-1.xf-yun.com"
    api_key: "xf-test"
    auth_token: "xf-secret"
    extra_body:
      appId: "xf-app"
      uid: "gateway"
    supported_models: [qwen-native-ws]
    model_map:
      qwen-native-ws: "xop35qwen2b"
model_routes:
  - pattern: "qwen-native-ws"
    provider_ids: [xfyun-ws-platform]
    priority: 10
"#;
        let store = make_store_from_yaml(yaml);
        let candidates = store.resolve_candidates(Some("qwen-native-ws"));
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].payload.adapter, "xfyun_websocket_compatible");
        assert_eq!(
            candidates[0].payload.chat_completions_path.as_deref(),
            Some("/v1.1/chat")
        );
        assert_eq!(candidates[0].protocol_family, "xfyun_websocket");
        assert_eq!(candidates[0].upstream_model.as_deref(), Some("xop35qwen2b"));
        assert_eq!(
            candidates[0]
                .payload
                .extra_body
                .as_ref()
                .and_then(|extra| extra.get("appId")),
            Some(&json!("xf-app"))
        );
    }

    #[cfg(feature = "line-qwen-web-reverse")]
    #[test]
    fn yaml_parsing_accepts_qwen_web_historical_preset_aliases() {
        let yaml = r#"
providers:
  - id: qwen-web-live
    preset: qwen-webui
    base_url: "https://chat.qwen.ai"
    api_key: "qwen-session"
    supported_models: [qwen-web-model]
model_routes:
  - pattern: "qwen-web-model"
    provider_ids: [qwen-web-live]
    priority: 10
"#;
        let store = make_store_from_yaml(yaml);
        let candidates = store.resolve_candidates(Some("qwen-web-model"));
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].payload.adapter, "qwen_web_compatible");
        assert_eq!(candidates[0].protocol_profile, "qwen_web_chat");
        assert_eq!(candidates[0].protocol_family, "qwen_web_chat");
    }

    #[test]
    fn yaml_parsing_with_gemini_business_preset() {
        let yaml = r#"
providers:
  - id: gemini-business-main
    preset: gemini-business
    base_url: "https://biz-discoveryengine.googleapis.com/v1alpha"
    api_key: "jwt-token"
    supported_models: [nano-banana-pro, gemini-3-pro-image-preview]
    session_auth:
      transport: "bearer"
      header_name: "Authorization"
    extra_body:
      configId: "cfg-123"
      session: "projects/demo/sessions/123"
model_routes:
  - pattern: "nano-banana*"
    provider_ids: [gemini-business-main]
    priority: 10
  - pattern: "gemini-3-pro-image-preview"
    provider_ids: [gemini-business-main]
    priority: 10
"#;
        let store = make_store_from_yaml(yaml);
        let candidates = store.resolve_candidates(Some("nano-banana-pro"));
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].payload.adapter, "gemini_business_compatible");
        assert_eq!(candidates[0].protocol_family, "gemini_business_images");
        assert_eq!(
            candidates[0].payload.chat_completions_path.as_deref(),
            Some("/locations/global/widgetStreamAssist")
        );
        let extra = candidates[0].payload.extra_body.as_ref().unwrap();
        assert_eq!(extra.get("configId"), Some(&json!("cfg-123")));
        assert_eq!(
            extra.get("session"),
            Some(&json!("projects/demo/sessions/123"))
        );
    }

    #[test]
    fn yaml_parsing_with_gemini_canvas_preset() {
        let yaml = r#"
providers:
  - id: gemini-canvas-main
    preset: gemini-canvas
    base_url: "https://gemini.google.com"
    supported_models: [gemini-2.5-flash-image-preview]
    keepalive:
      service_url: "http://gateway.internal"
      refresh_before_secs: 300
    credentials:
      - id: gemini-canvas-account-1
        api_key: ""
        runtime_state_object_key: "objects/gemini-canvas/auth-1.json"
        account_name: "canvas-main"
model_routes:
  - pattern: "gemini-2.5-flash-image-preview"
    provider_ids: [gemini-canvas-main]
    priority: 10
"#;
        let store = make_store_from_yaml(yaml);
        let candidates = store.resolve_candidates(Some("gemini-2.5-flash-image-preview"));
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].payload.adapter, "gemini_canvas_compatible");
        assert_eq!(candidates[0].protocol_family, "gemini_canvas_images");
        assert_eq!(
            candidates[0].payload.runtime_state_object_key.as_deref(),
            Some("objects/gemini-canvas/auth-1.json")
        );
        assert_eq!(
            candidates[0].payload.account_name.as_deref(),
            Some("canvas-main")
        );
        assert_eq!(
            candidates[0]
                .payload
                .extra_body
                .as_ref()
                .and_then(|extra| extra.get("shareId")),
            Some(&json!(
                crate::protocol::gemini_canvas::GEMINI_CANVAS_DEFAULT_SHARE_ID
            ))
        );
    }

    #[test]
    fn yaml_parsing_with_gemini_canvas_browser_relay_preset() {
        let yaml = r#"
providers:
  - id: gemini-canvas-relay
    preset: gemini-canvas-browser-relay
    base_url: "https://gemini.google.com"
    supported_models: [gemini-3-flash-preview]
    credentials:
      - id: gemini-canvas-relay-account-1
        api_key: ""
        runtime_state_object_key: "objects/gemini-canvas/relay-auth-1.json"
        account_name: "canvas-relay"
model_routes:
  - pattern: "gemini-3-flash-preview"
    provider_ids: [gemini-canvas-relay]
    priority: 10
"#;
        let store = make_store_from_yaml(yaml);
        let candidates = store.resolve_candidates(Some("gemini-3-flash-preview"));
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].payload.adapter,
            "gemini_canvas_web_reverse_compatible"
        );
        assert_eq!(
            candidates[0].payload.execution_mode,
            Some(ProviderExecutionMode::BrowserBacked)
        );
        assert_eq!(
            candidates[0].payload.runtime_state_object_key.as_deref(),
            Some("objects/gemini-canvas/relay-auth-1.json")
        );
    }

    #[test]
    fn yaml_unknown_preset_returns_error() {
        let yaml = r#"
providers:
  - id: mystery
    preset: nonexistent-preset
    base_url: "https://example.com"
    api_key: "k"
model_routes: []
"#;
        let config: RouteConfigYaml = serde_yaml::from_str(yaml).unwrap();
        let result = compile_yaml(config);
        assert!(result.is_err());
    }

    // ── alias resolution ──────────────────────────────────────────────────────

    #[test]
    fn alias_resolution_known_alias() {
        let yaml = r#"
providers:
  - id: anthropic-default
    preset: anthropic
    base_url: "https://api.anthropic.com"
    api_key: "k"
model_routes: []
aliases:
  sonnet: claude-sonnet-4-6
"#;
        let store = make_store_from_yaml(yaml);
        assert_eq!(
            store.resolve_alias(Some("sonnet")),
            Some("claude-sonnet-4-6".to_string())
        );
    }

    #[test]
    fn alias_resolution_unknown_returns_none() {
        let store = RouteConfigStore::new();
        assert!(store.resolve_alias(Some("unknown")).is_none());
        assert!(store.resolve_alias(None).is_none());
    }

    #[test]
    fn alias_resolution_maps_search_api_virtual_models_to_legacy_route_models() {
        let store = RouteConfigStore::new();
        assert_eq!(
            store.resolve_alias(Some(SEARCH_ROUTE_MODEL)),
            Some(LEGACY_SEARCH_ROUTE_MODEL.to_string())
        );
        assert_eq!(
            store.resolve_alias(Some(FETCH_ROUTE_MODEL)),
            Some(LEGACY_FETCH_ROUTE_MODEL.to_string())
        );
        assert_eq!(
            store.resolve_alias(Some(RESEARCH_ROUTE_MODEL)),
            Some(LEGACY_RESEARCH_ROUTE_MODEL.to_string())
        );
        assert_eq!(
            store.resolve_alias(Some(BALANCE_ROUTE_MODEL)),
            Some(LEGACY_BALANCE_ROUTE_MODEL.to_string())
        );
    }

    // ── candidate resolution ──────────────────────────────────────────────────

    fn multi_provider_store() -> RouteConfigStore {
        let yaml = r#"
providers:
  - id: anthropic-default
    preset: anthropic
    base_url: "https://api.anthropic.com"
    api_key: "k-ant"
  - id: openai-default
    preset: openai
    base_url: "https://api.openai.com"
    api_key: "k-oai"
  - id: codex-main
    preset: codex
    base_url: "https://chatgpt.com/backend-api/codex"
    api_key: "tok"
    supported_models:
      - gpt-5.4
      - gpt-5.4-mini
      - gpt-5.3-codex
      - gpt-5.2

model_routes:
  - pattern: "claude-*"
    provider_ids: [anthropic-default]
    priority: 10
  - pattern: "gpt-*"
    provider_ids: [openai-default, codex-main]
    priority: 10
  - pattern: "gpt-5.3-codex*"
    provider_ids: [codex-main]
    priority: 20

aliases:
  sonnet: claude-sonnet-4-6
  codex: gpt-5-codex
  gpt-5: gpt-5.4
  gpt-5-mini: gpt-5.4-mini
  gpt-5-codex: gpt-5.3-codex
  gpt-5-classic: gpt-5.2
"#;
        make_store_from_yaml(yaml)
    }

    #[test]
    fn resolve_candidates_claude_model_routes_to_anthropic() {
        let store = multi_provider_store();
        let cands = store.resolve_candidates(Some("claude-sonnet-4-6"));
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].provider_account_id, "anthropic-default");
    }

    #[test]
    fn resolve_candidates_gpt_model_routes_to_openai_and_codex() {
        let store = multi_provider_store();
        let cands = store.resolve_candidates(Some("gpt-4o"));
        assert_eq!(cands.len(), 2);
        let ids: Vec<&str> = cands
            .iter()
            .map(|c| c.provider_account_id.as_str())
            .collect();
        assert!(ids.contains(&"openai-default"));
        assert!(ids.contains(&"codex-main"));
    }

    #[test]
    fn resolve_candidates_codex_model_higher_priority_route_first() {
        // "gpt-5.3-codex" matches both "gpt-*" (p=10) and "gpt-5.3-codex*" (p=20).
        // Higher priority route is returned first; deduplication keeps codex-main once.
        let store = multi_provider_store();
        let cands = store.resolve_candidates(Some("gpt-5.3-codex"));
        // codex-main comes first (from the p=20 route), then openai-default (from p=10).
        assert_eq!(cands[0].provider_account_id, "codex-main");
    }

    #[test]
    fn resolve_candidates_no_model_returns_all() {
        let store = multi_provider_store();
        let cands = store.resolve_candidates(None);
        assert_eq!(cands.len(), 3);
    }

    #[test]
    fn resolve_candidates_unrouted_model_prefers_exact_supported_provider() {
        let yaml = r#"
providers:
  - id: codex-main
    preset: codex
    base_url: "https://chatgpt.com/backend-api/codex"
    api_key: "tok"
    supported_models: [gpt-5.3-codex]
  - id: anthropic-default
    preset: anthropic
    base_url: "https://api.anthropic.com"
    api_key: "k-ant"
    supported_models: [claude-sonnet-4-6]
model_routes:
  - pattern: "gpt-5-codex-*"
    provider_ids: [codex-main]
    priority: 20
aliases:
  gpt-5-codex: gpt-5.3-codex
"#;
        let store = make_store_from_yaml(yaml);

        let cands = store.resolve_candidates(Some("gpt-5-codex"));

        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].provider_account_id, "codex-main");
        assert_eq!(cands[0].model_alias.as_deref(), Some("gpt-5-codex"));
        assert_eq!(cands[0].upstream_model.as_deref(), Some("gpt-5.3-codex"));
    }

    #[test]
    fn resolve_candidates_unrouted_model_includes_provider_supporting_mapped_upstream_name() {
        let yaml = r#"
providers:
  - id: xfyun-platform
    preset: xfyun
    base_url: "https://maas-api.cn-huabei-1.xf-yun.com"
    api_key: "xf-test"
    supported_models: [xophunyuan7bmt]
    model_map:
      hunyuan-mt-7b: "xophunyuan7bmt"
  - id: canonical-provider
    base_url: "https://api.example.com"
    api_key: "canonical-test"
    supported_models: [hunyuan-mt-7b]
model_routes: []
"#;
        let store = make_store_from_yaml(yaml);

        let cands = store.resolve_candidates(Some("hunyuan-mt-7b"));
        let ids: Vec<&str> = cands
            .iter()
            .map(|candidate| candidate.provider_account_id.as_str())
            .collect();

        assert_eq!(ids, vec!["xfyun-platform", "canonical-provider"]);
        assert_eq!(cands[0].upstream_model.as_deref(), Some("xophunyuan7bmt"));
    }

    #[test]
    fn resolve_candidates_unmatched_model_falls_back_to_all() {
        let store = multi_provider_store();
        // "llama-3" matches no route → all providers.
        let cands = store.resolve_candidates(Some("llama-3"));
        assert_eq!(cands.len(), 3);
    }

    // ── list_models ───────────────────────────────────────────────────────────

    #[test]
    fn list_models_includes_supported_models_and_alias_keys_only() {
        let store = multi_provider_store();
        let models: Vec<String> = store.list_models().into_iter().map(|m| m.id).collect();
        // From supported_models
        assert!(models.contains(&"gpt-5.4".to_string()));
        assert!(models.contains(&"gpt-5.3-codex".to_string()));
        // From alias keys
        assert!(models.contains(&"sonnet".to_string()));
        assert!(models.contains(&"codex".to_string()));
        // Alias targets should stay hidden from the public catalog.
        assert!(!models.contains(&"claude-sonnet-4-6".to_string()));
    }

    #[test]
    fn list_models_object_field_is_model() {
        let yaml = r#"
providers:
  - id: p
    base_url: "https://example.com"
    api_key: "k"
    supported_models: [my-model]
model_routes: []
"#;
        let store = make_store_from_yaml(yaml);
        let m = &store.list_models()[0];
        assert_eq!(m.object, "model");
        assert_eq!(m.owned_by, "neuro-gateway");
    }

    // ── credential pool ──────────────────────────────────────────────────────

    #[test]
    fn single_credential_backward_compatible() {
        // When `credentials` is absent, the provider behaves as before:
        // one payload, empty credential_pool.
        let yaml = r#"
providers:
  - id: ai-hub
    preset: openai
    base_url: "https://example.com"
    api_key: "key123"
model_routes: []
"#;
        let config: RouteConfigYaml = serde_yaml::from_str(yaml).expect("parse");
        let inner = compile_yaml(config).expect("compile");
        assert_eq!(inner.providers.len(), 1);
        let p = &inner.providers[0];
        assert!(p.credential_pool.is_empty());
        assert_eq!(p.payload.api_key, "key123");
    }

    #[test]
    fn multi_credential_provider_builds_pool() {
        let yaml = r#"
providers:
  - id: accio
    preset: accio
    base_url: "https://phoenix-gw.alibaba.com"
    supported_models: [claude-sonnet-4-6]
    credentials:
      - id: acc-001
        extra_body:
          token: "token_1"
        headers:
          utdid: "utd-001"
      - id: acc-002
        extra_body:
          token: "token_2"
        headers:
          utdid: "utd-002"
      - id: acc-003
        extra_body:
          token: "token_3"
        headers:
          utdid: "utd-003"
model_routes: []
"#;
        let config: RouteConfigYaml = serde_yaml::from_str(yaml).expect("parse");
        let inner = compile_yaml(config).expect("compile");
        assert_eq!(inner.providers.len(), 1, "only one provider, not three");
        let p = &inner.providers[0];
        assert_eq!(p.credential_pool.len(), 3);
        assert_eq!(p.credential_pool[0].id, "acc-001");
        assert_eq!(p.credential_pool[1].id, "acc-002");
        assert_eq!(p.credential_pool[2].id, "acc-003");
    }

    #[test]
    fn credential_merges_headers_and_extra_body_from_provider() {
        let yaml = r#"
providers:
  - id: accio
    preset: accio
    base_url: "https://phoenix-gw.alibaba.com"
    credentials:
      - id: acc-001
        extra_body:
          token: "token_1"
        headers:
          utdid: "utd-001"
model_routes: []
"#;
        let config: RouteConfigYaml = serde_yaml::from_str(yaml).expect("parse");
        let inner = compile_yaml(config).expect("compile");
        let cred = &inner.providers[0].credential_pool[0];

        // Credential-specific header present
        assert_eq!(cred.payload.headers.get("utdid").unwrap(), "utd-001");
        // Preset headers inherited (accio preset carries the runtime app version).
        assert_eq!(cred.payload.headers.get("version").unwrap(), "0.5.6");
        assert!(cred.payload.headers.get("appKey").is_none());

        // Credential-specific extra_body present
        let extra = cred.payload.extra_body.as_ref().unwrap();
        assert_eq!(
            extra.get("token").unwrap(),
            &Value::String("token_1".to_string())
        );
    }

    #[test]
    fn round_robin_cycles_through_credentials() {
        let yaml = r#"
providers:
  - id: pool-test
    base_url: "https://example.com"
    credentials:
      - id: c0
        api_key: "key-0"
      - id: c1
        api_key: "key-1"
      - id: c2
        api_key: "key-2"
model_routes: []
"#;
        let config: RouteConfigYaml = serde_yaml::from_str(yaml).expect("parse");
        let inner = compile_yaml(config).expect("compile");
        let p = &inner.providers[0];
        assert_eq!(p.credential_pool.len(), 3);

        // Call select_credential 6 times and verify round-robin cycling.
        let keys: Vec<String> = (0..6)
            .map(|_| select_credential(p, None, None).api_key.clone())
            .collect();
        assert_eq!(
            keys,
            vec!["key-0", "key-1", "key-2", "key-0", "key-1", "key-2"]
        );
    }

    #[test]
    fn credential_selection_matches_xfyun_translated_model_names() {
        let yaml = r#"
providers:
  - id: xfyun-platform
    preset: xfyun
    base_url: "https://maas-api.cn-huabei-1.xf-yun.com"
    credentials:
      - id: hunyuan-key
        api_key: "key-hunyuan"
        supported_models: [xophunyuan7bmt]
      - id: qwen-key
        api_key: "key-qwen"
        supported_models: [xop35qwen2b]
    model_map:
      hunyuan-mt-7b: "xophunyuan7bmt"
      qwen-2b: "xop35qwen2b"
model_routes:
  - pattern: "hunyuan-*"
    provider_ids: [xfyun-platform]
    priority: 10
  - pattern: "qwen-2b"
    provider_ids: [xfyun-platform]
    priority: 10
"#;
        let store = make_store_from_yaml(yaml);

        let hunyuan = store.resolve_candidates(Some("hunyuan-mt-7b"));
        assert_eq!(hunyuan.len(), 1);
        assert_eq!(hunyuan[0].payload.api_key, "key-hunyuan");
        assert_eq!(hunyuan[0].upstream_model.as_deref(), Some("xophunyuan7bmt"));

        let qwen = store.resolve_candidates(Some("qwen-2b"));
        assert_eq!(qwen.len(), 1);
        assert_eq!(qwen[0].payload.api_key, "key-qwen");
        assert_eq!(qwen[0].upstream_model.as_deref(), Some("xop35qwen2b"));
    }

    #[test]
    fn all_credentials_share_same_provider_account_id() {
        // Verifies that all candidates from a multi-credential provider
        // share the SAME provider_account_id → one AIMD controller.
        let yaml = r#"
providers:
  - id: shared-pool
    base_url: "https://example.com"
    credentials:
      - id: c0
        api_key: "key-0"
      - id: c1
        api_key: "key-1"
model_routes: []
"#;
        let store = make_store_from_yaml(yaml);
        // Resolve candidates multiple times — each call gets a different
        // credential, but the provider_account_id is always "shared-pool".
        let c1 = store.resolve_candidates(None);
        let c2 = store.resolve_candidates(None);
        assert_eq!(c1.len(), 1);
        assert_eq!(c2.len(), 1);
        assert_eq!(c1[0].provider_account_id, "shared-pool");
        assert_eq!(c2[0].provider_account_id, "shared-pool");
        // Different credentials selected (round-robin)
        assert_ne!(c1[0].payload.api_key, c2[0].payload.api_key);
    }

    #[test]
    fn credential_auto_id_when_absent() {
        let yaml = r#"
providers:
  - id: my-provider
    base_url: "https://example.com"
    credentials:
      - api_key: "key-a"
      - api_key: "key-b"
model_routes: []
"#;
        let config: RouteConfigYaml = serde_yaml::from_str(yaml).expect("parse");
        let inner = compile_yaml(config).expect("compile");
        let pool = &inner.providers[0].credential_pool;
        assert_eq!(pool[0].id, "my-provider-cred-0");
        assert_eq!(pool[1].id, "my-provider-cred-1");
    }

    #[test]
    fn compile_preset_plus_multi_credential() {
        // Verify that preset merging works with multi-credential.
        let yaml = r#"
providers:
  - id: codex-pool
    preset: codex
    base_url: "https://chatgpt.com/backend-api/codex"
    headers:
      Chatgpt-Account-Id: "shared-acc"
    credentials:
      - id: tok-a
        api_key: "tok_aaa"
      - id: tok-b
        api_key: "tok_bbb"
model_routes: []
"#;
        let config: RouteConfigYaml = serde_yaml::from_str(yaml).expect("parse");
        let inner = compile_yaml(config).expect("compile");
        let p = &inner.providers[0];
        assert_eq!(p.credential_pool.len(), 2);

        // Both credentials should have preset headers (User-Agent, Originator)
        // AND provider-level header (Chatgpt-Account-Id).
        for cred in &p.credential_pool {
            assert!(
                cred.payload.headers.contains_key("User-Agent"),
                "preset header missing"
            );
            assert!(
                cred.payload.headers.contains_key("Originator"),
                "preset header missing"
            );
            assert_eq!(
                cred.payload.headers.get("Chatgpt-Account-Id").unwrap(),
                "shared-acc"
            );
        }
        // Different api_keys
        assert_eq!(p.credential_pool[0].payload.api_key, "tok_aaa");
        assert_eq!(p.credential_pool[1].payload.api_key, "tok_bbb");
    }

    #[test]
    fn credential_pool_propagates_session_auth_and_keepalive() {
        let yaml = r#"
providers:
  - id: grok-pool
    preset: grok
    base_url: "https://grok.com"
    credentials:
      - id: grok-session-1
        api_key: "sso-token"
        keepalive:
          service_url: "http://grok-keeper:8080"
          refresh_before_secs: 120
model_routes:
  - pattern: "grok-*"
    provider_ids: [grok-pool]
    priority: 10
"#;
        let store = make_store_from_yaml(yaml);
        let cands = store.resolve_candidates(Some("grok-3"));
        assert_eq!(cands.len(), 1);
        let payload = &cands[0].payload;
        assert_eq!(payload.credential_id.as_deref(), Some("grok-session-1"));
        assert_eq!(
            payload
                .session_auth
                .as_ref()
                .map(|cfg| cfg.primary_cookie_name()),
            Some("sso")
        );
        assert_eq!(
            payload
                .keepalive
                .as_ref()
                .map(|cfg| cfg.service_url.as_str()),
            Some("http://grok-keeper:8080")
        );
    }

    #[test]
    fn provider_count_is_one_for_multi_credential() {
        // With 3 credentials, provider_count should still be 1.
        let yaml = r#"
providers:
  - id: multi
    base_url: "https://example.com"
    credentials:
      - api_key: "k1"
      - api_key: "k2"
      - api_key: "k3"
model_routes: []
"#;
        let store = make_store_from_yaml(yaml);
        assert_eq!(store.provider_count(), 1);
    }

    #[test]
    fn credential_api_key_overrides_provider_key() {
        // When credentials array has api_key, each credential uses its own.
        // When a credential omits api_key, it falls back to the provider's.
        let yaml = r#"
providers:
  - id: mixed
    base_url: "https://example.com"
    api_key: "provider-key"
    credentials:
      - id: with-key
        api_key: "cred-key"
      - id: without-key
model_routes: []
"#;
        let config: RouteConfigYaml = serde_yaml::from_str(yaml).expect("parse");
        let inner = compile_yaml(config).expect("compile");
        let pool = &inner.providers[0].credential_pool;
        assert_eq!(pool[0].payload.api_key, "cred-key");
        assert_eq!(pool[1].payload.api_key, "provider-key");
    }
}
