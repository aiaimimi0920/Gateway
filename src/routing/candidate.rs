// ---------------------------------------------------------------------------
// Route candidate types
//
// Describes a concrete upstream provider account that the gateway can dispatch
// a request to, together with the payload needed to authenticate and call that
// provider's API.
// ---------------------------------------------------------------------------

mod endpoint_policy;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::credential_runtime::{KeepaliveConfig, SessionAuthConfig};
use crate::protocol::registry::{canonicalize_protocol_family_key, is_search_protocol_family};

pub const SEARCH_API_COMPATIBLE_ADAPTER: &str = "search_api_compatible";
pub const LEGACY_LINKUP_COMPATIBLE_ADAPTER: &str = "linkup_compatible";
pub const SEARCH_API_PROTOCOL_FAMILY: &str = "search";
pub const LEGACY_LINKUP_PROTOCOL_FAMILY: &str = "linkup";
pub const GEMINI_CANVAS_COMPATIBLE_ADAPTER: &str = "gemini_canvas_compatible";
pub const GEMINI_CANVAS_PROTOCOL_FAMILY: &str = "gemini_canvas";
pub const GEMINI_WEB_COMPATIBLE_ADAPTER: &str = "gemini_web_compatible";
pub const GEMINI_WEB_PROTOCOL_FAMILY: &str = "gemini_web_chat";
pub const CHATGPT_WEB_REVERSE_MODULAR_COMPATIBLE_ADAPTER: &str = "chatgpt_web_reverse_compatible";
pub const CHATGPT_WEB_PROTOCOL_FAMILY: &str = "chatgpt_web_chat";
pub const GEMINI_API_MODULAR_COMPATIBLE_ADAPTER: &str = "gemini_api_modular_compatible";
pub const GEMINI_WEB_REVERSE_MODULAR_COMPATIBLE_ADAPTER: &str =
    "gemini_web_reverse_modular_compatible";
pub const GEMINI_CANVAS_WEB_REVERSE_MODULAR_COMPATIBLE_ADAPTER: &str =
    "gemini_canvas_web_reverse_compatible";
pub const AISTUDIO_WEB_REVERSE_COMPATIBLE_ADAPTER: &str = "aistudio_web_reverse_compatible";
pub const KIRO_COMPATIBLE_ADAPTER: &str = "kiro_compatible";
pub const KIRO_PROTOCOL_FAMILY: &str = "kiro";
pub const FREEBUFF_COMPATIBLE_ADAPTER: &str = "freebuff_compatible";
pub const FREEBUFF_PROTOCOL_FAMILY: &str = "freebuff";
pub const SUNO_COMPATIBLE_ADAPTER: &str = "suno_compatible";
pub const SUNO_PROTOCOL_FAMILY: &str = "suno";
pub const UDIO_COMPATIBLE_ADAPTER: &str = "udio_compatible";
pub const UDIO_PROTOCOL_FAMILY: &str = "udio";
pub const XFYUN_WEBSOCKET_COMPATIBLE_ADAPTER: &str = "xfyun_websocket_compatible";
pub const XFYUN_WEBSOCKET_PROTOCOL_FAMILY: &str = "xfyun_websocket";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderExecutionMode {
    DirectHttp,
    BrowserBacked,
}

pub fn canonicalize_adapter_name(adapter: &str) -> String {
    if adapter == LEGACY_LINKUP_COMPATIBLE_ADAPTER {
        SEARCH_API_COMPATIBLE_ADAPTER.to_string()
    } else {
        adapter.to_string()
    }
}

pub fn is_search_api_adapter_name(adapter: &str) -> bool {
    adapter == SEARCH_API_COMPATIBLE_ADAPTER || adapter == LEGACY_LINKUP_COMPATIBLE_ADAPTER
}

pub fn canonicalize_protocol_family_name(protocol_family: &str) -> String {
    canonicalize_protocol_family_key(protocol_family)
}

pub fn is_search_api_protocol_family_name(protocol_family: &str) -> bool {
    is_search_protocol_family(protocol_family)
}

pub fn serde_compatible_provider_payload_value(
    payload: &Value,
    adapter_hint: Option<&str>,
) -> Value {
    let mut payload = payload.clone();
    if let Some(object) = payload.as_object_mut() {
        if !object.contains_key("adapter") {
            if let Some(adapter) = adapter_hint
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                object.insert("adapter".to_string(), Value::String(adapter.to_string()));
            }
        }
        if !object.contains_key("baseUrl") && !object.contains_key("base_url") {
            object.insert("baseUrl".to_string(), Value::String(String::new()));
        }
        if !object.contains_key("apiKey") && !object.contains_key("api_key") {
            object.insert("apiKey".to_string(), Value::String(String::new()));
        }
    }
    payload
}

pub fn deserialize_provider_payload(
    payload: &Value,
    adapter_hint: Option<&str>,
) -> Result<ProviderAccountPayload, serde_json::Error> {
    serde_json::from_value(serde_compatible_provider_payload_value(
        payload,
        adapter_hint,
    ))
}

// ---------------------------------------------------------------------------
// ProviderAccountPayload
// ---------------------------------------------------------------------------

/// Everything required to make an authenticated HTTP call to an upstream
/// provider.  The `adapter` field selects which HTTP adapter processes the
/// request; the remaining fields are adapter-specific.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderAccountPayload {
    /// Adapter identifier.  One of:
    /// - `"openai_compatible"` — OpenAI-style `/v1/chat/completions` endpoint
    /// - `"anthropic_compatible"` — Anthropic `/v1/messages` endpoint
    /// - `"gemini_api_compatible"` — Gemini API `generateContent / streamGenerateContent`
    /// - `"bedrock_converse_compatible"` — Bedrock Converse / ConverseStream
    /// - `"cohere_compatible"` — Cohere Chat v2
    /// - `"grok_compatible"` — Grok web chat endpoint returning NDJSON
    /// - `"gemini_business_compatible"` — Gemini Business widget image endpoints
    /// - `"chataibot_compatible"` — Chataibot image endpoints
    /// - `"lumalabs_compatible"` — LumaLabs board image endpoint + events stream
    /// - `"gemini_canvas_compatible"` — Gemini Canvas Web reverse-web browser-state provider with direct HTTP replay by default
    /// - `"gemini_web_compatible"` — Gemini Web reverse-chat API
    /// - `"chatgpt_web_reverse_compatible"` — ChatGPT Web reverse-chat API
    /// - `"kiro_compatible"` — Kiro/Amazon Q session-backed assistant API
    /// - `"freebuff_compatible"` — FreeBuff/Codebuff run-backed chat API
    /// - `"qwen_web_compatible"` — Qwen WebUI reverse-chat API
    /// - `"suno_compatible"` — Suno session-backed reverse-web image / audio / video provider
    /// - `"udio_compatible"` — Udio browser-backed reverse-web image / music / video provider
    /// - `"xfyun_websocket_compatible"` — XFYun native signed WebSocket chat API
    /// - `"search_api_compatible"` — search-style JSON passthrough endpoints
    /// - `"custom_http"` — arbitrary HTTP endpoint with configurable auth
    pub adapter: String,

    /// Base URL of the upstream API (no trailing slash).
    #[serde(alias = "baseUrl")]
    pub base_url: String,

    /// API key or credential token.
    #[serde(alias = "apiKey")]
    pub api_key: String,

    /// Optional credential ID carried through the request path.
    /// Redis-backed hosted credentials use this for keepalive write-back.
    #[serde(default)]
    #[serde(alias = "credentialId")]
    pub credential_id: Option<String>,

    /// Optional expiry timestamp for runtime material.
    /// Browser-state credentials may use this even when `session_auth` is absent.
    #[serde(default)]
    #[serde(alias = "expiresAt")]
    pub expires_at: Option<String>,

    /// Optional object-storage key for browser/runtime state bundles.
    /// Used by browser-gated providers such as Gemini Canvas.
    #[serde(default)]
    #[serde(alias = "runtimeStateObjectKey")]
    pub runtime_state_object_key: Option<String>,

    /// Optional human-readable account name carried with browser-state credentials.
    #[serde(default)]
    #[serde(alias = "accountName")]
    pub account_name: Option<String>,

    /// Default send-path mode for this provider account.
    #[serde(default)]
    #[serde(alias = "executionMode")]
    pub execution_mode: Option<ProviderExecutionMode>,

    /// Endpoint-specific execution-mode overrides.
    #[serde(default)]
    #[serde(alias = "endpointExecutionModes")]
    pub endpoint_execution_modes: Option<HashMap<String, ProviderExecutionMode>>,

    /// The default model to use when the caller does not specify one.
    #[serde(default)]
    #[serde(alias = "defaultModel")]
    pub default_model: Option<String>,

    /// Extra HTTP headers to forward verbatim to the upstream.
    #[serde(default)]
    pub headers: HashMap<String, String>,

    // ── OpenAI-compatible ─────────────────────────────────────────────────
    /// Authentication scheme for OpenAI-compatible adapters.
    /// `"bearer"` (default) sends `Authorization: Bearer <api_key>`.
    /// `"x-api-key"` sends `x-api-key: <api_key>`.
    /// `"api-key"` sends `api-key: <api_key>`.
    #[serde(default)]
    #[serde(alias = "authMode")]
    pub auth_mode: Option<String>,

    // ── Anthropic-compatible ──────────────────────────────────────────────
    /// `anthropic-version` header value (e.g. `"2023-06-01"`).
    #[serde(default)]
    #[serde(alias = "anthropicVersion")]
    pub anthropic_version: Option<String>,

    /// Optional `anthropic-beta` header values (e.g. `["tools-2024-04-04"]`).
    #[serde(default)]
    #[serde(alias = "betaHeaders")]
    pub beta_headers: Option<Vec<String>>,

    // ── Custom HTTP ───────────────────────────────────────────────────────
    /// Name of the authentication header for `custom_http` adapters.
    #[serde(default)]
    #[serde(alias = "authHeaderName")]
    pub auth_header_name: Option<String>,

    /// Authentication token for `custom_http` adapters (distinct from
    /// `api_key` to allow both a key and a token simultaneously).
    #[serde(default)]
    #[serde(alias = "authToken")]
    pub auth_token: Option<String>,

    // ── Path overrides ────────────────────────────────────────────────────
    /// Custom path for OpenAI Responses API endpoint (default: `/v1/responses`).
    /// Set to `/responses` for Codex-style providers.
    #[serde(default)]
    #[serde(alias = "responsesPath")]
    pub responses_path: Option<String>,

    /// Custom path for chat completions endpoint (default: `/v1/chat/completions`).
    #[serde(default)]
    #[serde(alias = "chatCompletionsPath")]
    pub chat_completions_path: Option<String>,

    /// Custom path for legacy completions endpoint (default: `/v1/completions`).
    #[serde(default)]
    #[serde(alias = "completionsPath")]
    pub completions_path: Option<String>,

    /// Custom path for embeddings endpoint (default: `/v1/embeddings`).
    #[serde(default)]
    #[serde(alias = "embeddingsPath")]
    pub embeddings_path: Option<String>,

    /// Custom path for audio transcription endpoint (default: `/v1/audio/transcriptions`).
    #[serde(default)]
    #[serde(alias = "audioTranscriptionsPath")]
    pub audio_transcriptions_path: Option<String>,

    /// Custom path for audio speech endpoint (default: `/v1/audio/speech`).
    #[serde(default)]
    #[serde(alias = "audioSpeechPath")]
    pub audio_speech_path: Option<String>,

    /// Custom path for Anthropic messages endpoint (default: `/v1/messages`).
    #[serde(default)]
    #[serde(alias = "messagesPath")]
    pub messages_path: Option<String>,

    /// Explicit path for search requests.
    #[serde(default)]
    #[serde(alias = "searchPath")]
    pub search_path: Option<String>,

    /// Explicit path for fetch/extract requests.
    #[serde(default)]
    #[serde(alias = "fetchPath")]
    pub fetch_path: Option<String>,

    /// Custom path for research endpoints (e.g. `/v1/research`).
    #[serde(default)]
    #[serde(alias = "researchPath")]
    pub research_path: Option<String>,

    /// Custom path for balance/credits endpoints (e.g. `/v1/credits/balance`).
    #[serde(default)]
    #[serde(alias = "balancePath")]
    pub balance_path: Option<String>,

    /// Preferred request-body field for search text (`q` vs `query`).
    #[serde(default)]
    #[serde(alias = "searchQueryField")]
    pub search_query_field: Option<String>,

    /// Preferred request-body field for fetch targets (`url` vs `urls`).
    #[serde(default)]
    #[serde(alias = "fetchUrlsField")]
    pub fetch_urls_field: Option<String>,

    // ── Data-driven provider quirks ──────────────────────────────────────
    /// Extra key-value pairs merged into the request body after protocol
    /// packing.  Use this for provider constraints that are pure data (e.g.
    /// Codex requiring `"store": false`).  Keys already present in the body
    /// are NOT overwritten.
    #[serde(default)]
    #[serde(alias = "extraBody")]
    pub extra_body: Option<HashMap<String, Value>>,

    /// Optional session-backed auth metadata carried alongside the normal
    /// credential payload. Adapters may use this to render cookie auth or to
    /// decide when a keepalive preflight is required.
    #[serde(default)]
    #[serde(alias = "sessionAuth")]
    pub session_auth: Option<SessionAuthConfig>,

    /// Optional external keepalive steward configuration.
    /// When present, the send stage may call the steward before dispatching
    /// the upstream request.
    #[serde(default)]
    #[serde(alias = "keepalive")]
    pub keepalive: Option<KeepaliveConfig>,
}

// ---------------------------------------------------------------------------
// RouteCandidate
// ---------------------------------------------------------------------------

/// A fully-resolved upstream provider candidate ready for scoring and selection
/// by the routing layer.
#[derive(Debug, Clone)]
pub struct RouteCandidate {
    /// Unique identifier of the provider account in the platform.
    pub provider_account_id: String,

    /// Optional provider-credential row chosen under the provider account.
    pub provider_credential_id: Option<String>,

    /// Human-readable label for logging and metrics.
    pub label: String,

    /// The credential / endpoint payload needed to dispatch the request.
    pub payload: ProviderAccountPayload,

    /// Protocol family string, e.g. `"openai"` or `"anthropic"`.
    pub protocol_family: String,

    /// Provider-specific protocol profile under the selected family.
    pub protocol_profile: String,

    /// Broad protocol families allowed for this credential+model candidate
    /// before request-specific narrowing picks the final outbound family.
    pub supported_protocol_families: Vec<String>,

    /// Adapter identifier, mirrors `payload.adapter`.
    pub adapter: String,

    /// Optional alias used to rewrite the model name in the outgoing request.
    pub model_alias: Option<String>,

    /// The actual model name to send to the upstream provider, after alias
    /// resolution.
    pub upstream_model: Option<String>,

    /// Execution mode resolved for the current endpoint kind.
    pub resolved_execution_mode: ProviderExecutionMode,

    /// Static routing priority.  Higher values are preferred.
    pub priority: i32,

    /// Base weight used for weighted random / priority-weighted selection.
    pub weight: i32,

    /// Number of recent failures recorded for this candidate.
    pub failure_count: u32,

    /// RFC 3339 timestamp after which the candidate exits cooldown.
    /// `None` means not in cooldown.
    pub cooldown_until: Option<String>,

    /// Composite routing score used when ordering the candidate queue.
    pub routing_score: Option<f64>,

    /// Health contribution of the routing score.
    pub routing_health_weight: Option<f64>,

    /// Capacity contribution of the routing score.
    pub routing_capacity_weight: Option<f64>,

    /// Whether the routing layer considered this candidate degraded.
    pub routing_degraded: Option<bool>,

    /// Whether the circuit breaker was open when the queue was built.
    pub routing_breaker_open: Option<bool>,

    /// Human-readable degradation reasons recorded during routing.
    pub routing_degradation_reasons: Vec<String>,
}

impl RouteCandidate {
    /// Returns `true` if the candidate is currently in cooldown relative to
    /// `now` (formatted as an RFC 3339 string).
    ///
    /// For simplicity this does a lexicographic string comparison — valid as
    /// long as both strings are proper RFC 3339 timestamps.
    pub fn is_in_cooldown(&self, now_rfc3339: &str) -> bool {
        match &self.cooldown_until {
            None => false,
            Some(until) => until.as_str() > now_rfc3339,
        }
    }

    pub fn runtime_subject_id(&self) -> &str {
        self.provider_credential_id
            .as_deref()
            .unwrap_or(self.provider_account_id.as_str())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::EndpointKind;

    fn make_payload(adapter: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
            adapter: adapter.to_string(),
            base_url: "https://api.example.com".to_string(),
            api_key: "sk-test-1234".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: Some("gpt-4o".to_string()),
            headers: HashMap::new(),
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: None,
            chat_completions_path: None,
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            extra_body: None,
            session_auth: None,
            keepalive: None,
        }
    }

    fn make_candidate(id: &str, priority: i32, weight: i32) -> RouteCandidate {
        RouteCandidate {
            provider_account_id: id.to_string(),
            provider_credential_id: None,
            label: format!("provider-{}", id),
            payload: make_payload("openai_compatible"),
            protocol_family: "openai".to_string(),
            protocol_profile: "openai".to_string(),
            supported_protocol_families: vec!["openai_chat".to_string()],
            adapter: "openai_compatible".to_string(),
            model_alias: None,
            upstream_model: Some("gpt-4o".to_string()),
            resolved_execution_mode: ProviderExecutionMode::DirectHttp,
            priority,
            weight,
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

    // ── is_in_cooldown ────────────────────────────────────────────────────

    #[test]
    fn not_in_cooldown_when_none() {
        let c = make_candidate("a", 1, 10);
        assert!(!c.is_in_cooldown("2026-04-07T00:00:00Z"));
    }

    #[test]
    fn in_cooldown_when_until_is_in_future() {
        let mut c = make_candidate("a", 1, 10);
        c.cooldown_until = Some("2026-04-08T00:00:00Z".to_string());
        assert!(c.is_in_cooldown("2026-04-07T00:00:00Z"));
    }

    #[test]
    fn not_in_cooldown_when_until_is_in_past() {
        let mut c = make_candidate("a", 1, 10);
        c.cooldown_until = Some("2026-04-06T00:00:00Z".to_string());
        assert!(!c.is_in_cooldown("2026-04-07T00:00:00Z"));
    }

    // ── ProviderAccountPayload serialisation ──────────────────────────────

    #[test]
    fn payload_roundtrip_json() {
        let payload = make_payload("anthropic_compatible");
        let json = serde_json::to_string(&payload).unwrap();
        let decoded: ProviderAccountPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.adapter, "anthropic_compatible");
        assert_eq!(decoded.api_key, "sk-test-1234");
        assert_eq!(decoded.default_model.as_deref(), Some("gpt-4o"));
    }

    #[test]
    fn payload_headers_default_empty_when_absent() {
        let json = r#"{
            "adapter": "openai_compatible",
            "base_url": "https://api.example.com",
            "api_key": "sk-x"
        }"#;
        let decoded: ProviderAccountPayload = serde_json::from_str(json).unwrap();
        assert!(decoded.headers.is_empty());
        assert!(decoded.default_model.is_none());
    }

    #[test]
    fn deserialize_provider_payload_backfills_missing_runtime_state_transport_fields() {
        let value = serde_json::json!({
            "runtimeStateObjectKey": "credential-runtime/gemini-canvas/program/storage-state.json",
            "extraBody": {
                "shareId": "fe24c455a570"
            }
        });
        let decoded = deserialize_provider_payload(
            &value,
            Some("gemini_canvas_program_web_reverse_compatible"),
        )
        .expect("deserialize runtime-state payload");
        assert_eq!(
            decoded.adapter,
            "gemini_canvas_program_web_reverse_compatible"
        );
        assert_eq!(decoded.base_url, "");
        assert_eq!(decoded.api_key, "");
        assert_eq!(
            decoded.runtime_state_object_key.as_deref(),
            Some("credential-runtime/gemini-canvas/program/storage-state.json")
        );
    }

    #[test]
    fn resolves_default_execution_mode_to_direct_http() {
        let payload = make_payload("openai_compatible");
        assert_eq!(
            payload.resolve_execution_mode(EndpointKind::ChatCompletions),
            ProviderExecutionMode::DirectHttp
        );
    }

    #[test]
    fn gemini_canvas_defaults_to_direct_http() {
        let payload = make_payload("gemini_canvas_compatible");
        assert_eq!(
            payload.resolve_execution_mode(EndpointKind::ChatCompletions),
            ProviderExecutionMode::DirectHttp
        );
    }

    #[test]
    fn resolves_endpoint_execution_override() {
        let mut payload = make_payload("producer_compatible");
        payload.execution_mode = Some(ProviderExecutionMode::DirectHttp);
        payload.endpoint_execution_modes = Some(HashMap::from([(
            "videos_generations".to_string(),
            ProviderExecutionMode::BrowserBacked,
        )]));
        assert_eq!(
            payload.resolve_execution_mode(EndpointKind::VideosGenerations),
            ProviderExecutionMode::BrowserBacked
        );
        assert_eq!(
            payload.resolve_execution_mode(EndpointKind::MusicGenerations),
            ProviderExecutionMode::DirectHttp
        );
    }

    #[test]
    fn canonicalizes_legacy_search_adapter_alias() {
        assert_eq!(
            canonicalize_adapter_name(LEGACY_LINKUP_COMPATIBLE_ADAPTER),
            SEARCH_API_COMPATIBLE_ADAPTER
        );
    }

    #[test]
    fn canonicalizes_legacy_protocol_family_alias() {
        assert_eq!(
            canonicalize_protocol_family_name(LEGACY_LINKUP_PROTOCOL_FAMILY),
            "linkup_search"
        );
    }

    #[test]
    fn search_endpoint_support_is_driven_by_explicit_paths() {
        let mut payload = make_payload(SEARCH_API_COMPATIBLE_ADAPTER);
        payload.search_path = Some("/search".to_string());
        payload.fetch_path = Some("/fetch".to_string());
        assert!(payload.supports_search_endpoint(EndpointKind::Search));
        assert!(payload.supports_search_endpoint(EndpointKind::Fetch));
        assert!(!payload.supports_search_endpoint(EndpointKind::ResearchCreate));
        assert!(!payload.supports_search_endpoint(EndpointKind::CreditsBalance));
    }

    #[test]
    fn responses_path_without_chat_path_bridges_openai_text_endpoints() {
        let mut payload = make_payload("openai_compatible");
        payload.responses_path = Some("/responses".to_string());
        assert!(payload.bridges_openai_text_endpoint_to_responses(EndpointKind::ChatCompletions));
        assert!(payload.bridges_openai_text_endpoint_to_responses(EndpointKind::Messages));
        assert!(payload.bridges_openai_text_endpoint_to_responses(EndpointKind::Completions));
        assert!(payload.prefers_forced_streaming_responses(EndpointKind::Responses));
    }

    #[test]
    fn custom_chat_completions_path_bridges_responses_when_responses_path_missing() {
        let mut payload = make_payload("openai_compatible");
        payload.chat_completions_path = Some("/compatible/chat".to_string());
        assert!(payload.bridges_openai_responses_to_chat_completions(EndpointKind::Responses));
        assert!(!payload.bridges_openai_text_endpoint_to_responses(EndpointKind::Responses));
    }

    #[test]
    fn qwen_style_openai_paths_prefer_same_protocol_direct_chat() {
        let mut payload = make_payload("openai_compatible");
        payload.chat_completions_path = Some("/chat/completions".to_string());
        payload.responses_path = Some("/responses".to_string());

        assert!(!payload.bridges_openai_text_endpoint_to_responses(EndpointKind::ChatCompletions));
        assert!(payload.bridges_openai_text_endpoint_to_responses(EndpointKind::Messages));
        assert!(!payload.bridges_openai_text_endpoint_to_responses(EndpointKind::Completions));
        assert!(!payload.prefers_forced_streaming_responses(EndpointKind::Responses));
        assert!(!payload.prefers_forced_streaming_responses(EndpointKind::ChatCompletions));
        assert!(!payload.bridges_openai_responses_to_chat_completions(EndpointKind::Responses));
    }
}
