//! Stable route-document and account-inventory serialization contracts.

use super::*;

fn model_route_enabled_default() -> bool {
    true
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_true(value: &bool) -> bool {
    *value
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
    /// Whether the rule dispatches. A disabled rule still *matches*, so it
    /// shadows the "no route matched" fallback: the model it names resolves to
    /// no candidate at all instead of quietly reaching every provider that
    /// declares support. Absent in YAML means enabled, and an enabled rule is
    /// not serialised back, so existing documents round-trip unchanged.
    #[serde(
        default = "model_route_enabled_default",
        skip_serializing_if = "is_true"
    )]
    pub enabled: bool,
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

/// Top-level YAML configuration document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteConfigYaml {
    pub providers: Vec<ProviderConfigYaml>,
    #[serde(default)]
    pub model_routes: Vec<ModelRoute>,
    /// Model aliases: `"sonnet"` → `"claude-sonnet-4-6"`.
    #[serde(default)]
    pub aliases: HashMap<String, String>,
    /// Optional account-group metadata used by the web console and trusted
    /// request-time routing selectors.
    #[serde(default)]
    pub account_groups: Vec<AccountGroupYaml>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountGroupYaml {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub billing_multiplier: Option<f64>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub provider_credential_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RouteAccountGroupInventory {
    pub account_groups: Vec<RouteAccountGroupView>,
    pub accounts: Vec<RouteAccountView>,
    pub providers: Vec<RouteAccountProviderView>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RouteAccountGroupView {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub billing_multiplier: f64,
    pub configured_billing_multiplier: Option<f64>,
    pub enabled: bool,
    pub notes: Option<String>,
    pub member_count: usize,
    pub provider_credential_ids: Vec<String>,
    pub providers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RouteAccountView {
    pub id: String,
    pub display_name: String,
    pub provider_id: String,
    pub provider_label: String,
    /// Stable service/vendor identifier supplied by the route document.
    /// This is intentionally distinct from `provider_id`: multiple providers
    /// can belong to the same vendor while retaining separate endpoints.
    pub vendor_key: Option<String>,
    /// Human-readable service/vendor name supplied by the route document.
    pub vendor_name: Option<String>,
    pub provider_preset: Option<String>,
    pub credential_id: Option<String>,
    pub base_url: Option<String>,
    pub mode: String,
    /// Effective runtime routing state. Disabled accounts remain visible in
    /// inventory so operators can re-enable them later.
    pub enabled: bool,
    pub supported_models: Vec<String>,
    pub group_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RouteAccountProviderView {
    pub id: String,
    pub label: String,
    /// Stable service/vendor identifier supplied by the route document.
    pub vendor_key: Option<String>,
    /// Human-readable service/vendor name supplied by the route document.
    pub vendor_name: Option<String>,
    pub preset: Option<String>,
    pub base_url: Option<String>,
    pub account_ids: Vec<String>,
    pub supported_models: Vec<String>,
}

#[derive(Debug, thiserror::Error, Clone, Eq, PartialEq)]
pub enum RouteAccountGroupSelectionError {
    #[error("requested account group '{0}' was not found")]
    NotFound(String),
    #[error("requested account group '{0}' is disabled")]
    Disabled(String),
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
    /// Optional provider-specific identity class used by automation drivers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_identity_category_id: Option<String>,
    /// Optional runtime switch for this credential. Missing means enabled for
    /// backwards compatibility with existing route documents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub execution_mode: Option<ProviderExecutionMode>,
    #[serde(default)]
    pub endpoint_execution_modes: Option<HashMap<String, ProviderExecutionMode>>,
    /// Models this credential can serve. Empty = inherits provider's supported_models.
    /// When set, the credential is only selected for requests matching these models.
    #[serde(default)]
    pub supported_models: Vec<String>,
    /// Enables a recurring connectivity probe for this credential.
    #[serde(default)]
    pub scheduled_probe_enabled: bool,
    /// Recurring probe interval in minutes. Values are clamped to 1..=10080.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduled_probe_interval_minutes: Option<u64>,

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
    /// Optional stable service/vendor identifier used by the management UI.
    /// Kept separate from `id` because several provider endpoints can belong
    /// to one vendor.  Missing fields remain compatible with legacy YAML.
    #[serde(default, alias = "vendorKey", skip_serializing_if = "Option::is_none")]
    pub vendor_key: Option<String>,
    /// Optional human-readable service/vendor name used by the management UI.
    #[serde(default, alias = "vendorName", skip_serializing_if = "Option::is_none")]
    pub vendor_name: Option<String>,
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
    /// Optional password used by an external provider credential storage/refill worker.
    /// This field is handled as a secret and never exposed in redacted views.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_storage_password: Option<String>,
    /// Provider-local refill material directory (local or mounted filesystem).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_storage_path: Option<String>,
    /// Provider-local directory for recoverable plaintext JSON archives.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_archive_path: Option<String>,
    /// Explicit connections override legacy local paths without migrating data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_storage_connection: Option<CredentialStorageConnection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_archive_connection: Option<CredentialStorageConnection>,
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
    /// Maximum available credentials (unobserved refill credentials reserve capacity). The legacy name remains compatible; defaults to 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pool_target_size: Option<usize>,
    /// Automatic refill starts strictly below this threshold; zero disables it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pool_min_size: Option<usize>,
    /// Durable hysteresis marker owned by refill commits, not an operator switch.
    #[serde(default, skip_serializing_if = "is_false")]
    pub pool_refill_in_progress: bool,
    /// Enables the trusted refill driver selected for this provider.
    #[serde(default)]
    pub auto_refill_enabled: bool,
    /// Enables permanent-failure pruning reported by the trusted driver.
    #[serde(default)]
    pub auto_prune_enabled: bool,
    /// When enabled, pruned credentials are removed without first writing an
    /// archive copy under the Gateway-managed credential archive directory.
    #[serde(default)]
    pub credential_permanent_delete_enabled: bool,
    /// Optional explicit reference into the backend-owned driver allowlist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_automation_driver_id: Option<String>,
    /// Optional UI-defined identity classes retained for provider-specific drivers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub credential_identity_categories: Vec<Value>,
    /// Enables a recurring connectivity probe for a provider-default credential.
    #[serde(default)]
    pub scheduled_probe_enabled: bool,
    /// Recurring probe interval in minutes. Values are clamped to 1..=10080.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduled_probe_interval_minutes: Option<u64>,
    /// Optional pool of credentials for this provider.
    /// If present, the provider has multiple credentials that are selected at
    /// request time (round-robin).  If absent, `api_key` is used as a single
    /// credential.
    #[serde(default)]
    pub credentials: Vec<ProviderCredentialYaml>,
}
