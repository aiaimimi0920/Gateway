// ---------------------------------------------------------------------------
// preset.rs — Provider preset templates with compile-time merging
//
// A preset defines the INVARIANT parts of a provider integration (adapter,
// protocol-required headers, forced body fields). An account provides the
// VARIABLE parts (base_url, api_key, account-specific headers like
// Chatgpt-Account-Id).
//
// `compile()` merges preset + account overrides into a final
// `ProviderAccountPayload` ONCE at load time. After that, every request
// uses the compiled payload with zero merge overhead.
// ---------------------------------------------------------------------------

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::credential_runtime::{KeepaliveConfig, SessionAuthConfig};
use crate::protocol::chataibot::CHATAIBOT_DEFAULT_MODEL;
use crate::protocol::gemini_business::NANO_BANANA_PRO_MODEL;
use crate::protocol::gemini_canvas::{
    GEMINI_CANVAS_DEFAULT_MODEL, GEMINI_CANVAS_DEFAULT_TEXT_MODEL,
};
use crate::protocol::kiro::{KIRO_DEFAULT_MODEL, KIRO_GENERATE_ASSISTANT_RESPONSE_PATH};
use crate::protocol::lumalabs::LUMALABS_DEFAULT_MODEL;
use crate::protocol::producer::PRODUCER_DEFAULT_MODEL;
use crate::protocol::suno::SUNO_DEFAULT_MODEL;
use crate::protocol::udio::UDIO_DEFAULT_MODEL;
use crate::routing::candidate::{
    ProviderAccountPayload, ProviderExecutionMode, SEARCH_API_COMPATIBLE_ADAPTER,
};

// ---------------------------------------------------------------------------
// Preset definition
// ---------------------------------------------------------------------------

/// A reusable template for a class of providers. Defines everything that is
/// the SAME across all accounts of this type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPreset {
    /// Unique identifier, e.g. `"codex"`, `"azure-openai"`, `"anthropic"`.
    pub id: String,

    /// Adapter type inherited by accounts using this preset.
    pub adapter: String,

    /// Headers injected into every request. Account headers are merged ON TOP
    /// (account wins on conflict).
    #[serde(default)]
    pub headers: HashMap<String, String>,

    /// Body fields injected into every request. Account extra_body merges on
    /// top (account wins on conflict).
    #[serde(default)]
    pub extra_body: HashMap<String, Value>,

    // ── Optional defaults (account can override) ─────────────────────────
    #[serde(default)]
    pub default_model: Option<String>,

    #[serde(default)]
    pub auth_mode: Option<String>,

    #[serde(default)]
    pub anthropic_version: Option<String>,

    #[serde(default)]
    pub beta_headers: Option<Vec<String>>,

    #[serde(default)]
    pub auth_header_name: Option<String>,

    // ── Path overrides ───────────────────────────────────────────────────
    #[serde(default)]
    pub responses_path: Option<String>,

    #[serde(default)]
    pub chat_completions_path: Option<String>,

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

    #[serde(default)]
    pub session_auth: Option<SessionAuthConfig>,

    #[serde(default)]
    pub execution_mode: Option<ProviderExecutionMode>,

    #[serde(default)]
    pub endpoint_execution_modes: Option<HashMap<String, ProviderExecutionMode>>,
}

// ---------------------------------------------------------------------------
// Account overrides (the per-site / per-credential part)
// ---------------------------------------------------------------------------

/// The variable parts of a provider account. Combined with a preset to
/// produce a complete `ProviderAccountPayload`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountOverrides {
    /// Required: upstream base URL.
    pub base_url: String,

    /// Required: API key or access token.
    pub api_key: String,

    /// Account-specific headers (merged on top of preset headers).
    #[serde(default)]
    pub headers: HashMap<String, String>,

    /// Account-specific body fields (merged on top of preset extra_body).
    #[serde(default)]
    pub extra_body: HashMap<String, Value>,

    // ── Optional overrides ───────────────────────────────────────────────
    #[serde(default)]
    pub default_model: Option<String>,

    #[serde(default)]
    pub auth_mode: Option<String>,

    #[serde(default)]
    pub anthropic_version: Option<String>,

    #[serde(default)]
    pub beta_headers: Option<Vec<String>>,

    #[serde(default)]
    pub auth_header_name: Option<String>,

    #[serde(default)]
    pub auth_token: Option<String>,

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
}

// ---------------------------------------------------------------------------
// Compile: preset + overrides → ProviderAccountPayload (one-time)
// ---------------------------------------------------------------------------

/// Merge a preset and account overrides into a final `ProviderAccountPayload`.
/// This runs ONCE when the account is loaded. The result is cached and used
/// directly for every request — no per-request merge overhead.
pub fn compile_provider_account(
    preset: &ProviderPreset,
    account: &AccountOverrides,
) -> ProviderAccountPayload {
    // Headers: preset as base, account overrides on top
    let mut headers = preset.headers.clone();
    for (k, v) in &account.headers {
        headers.insert(k.clone(), v.clone());
    }

    // Extra body: preset as base, account overrides on top
    let mut extra_body = preset.extra_body.clone();
    for (k, v) in &account.extra_body {
        extra_body.insert(k.clone(), v.clone());
    }

    ProviderAccountPayload {
        adapter: preset.adapter.clone(),
        base_url: account.base_url.clone(),
        api_key: account.api_key.clone(),
        credential_id: None,
        expires_at: account.expires_at.clone(),
        runtime_state_object_key: account.runtime_state_object_key.clone(),
        account_name: account.account_name.clone(),
        execution_mode: account.execution_mode.or(preset.execution_mode),
        endpoint_execution_modes: account
            .endpoint_execution_modes
            .clone()
            .or_else(|| preset.endpoint_execution_modes.clone()),
        default_model: account
            .default_model
            .clone()
            .or_else(|| preset.default_model.clone()),
        headers,
        auth_mode: account
            .auth_mode
            .clone()
            .or_else(|| preset.auth_mode.clone()),
        anthropic_version: account
            .anthropic_version
            .clone()
            .or_else(|| preset.anthropic_version.clone()),
        beta_headers: account
            .beta_headers
            .clone()
            .or_else(|| preset.beta_headers.clone()),
        auth_header_name: account
            .auth_header_name
            .clone()
            .or_else(|| preset.auth_header_name.clone()),
        auth_token: account.auth_token.clone(),
        responses_path: preset.responses_path.clone(),
        chat_completions_path: preset.chat_completions_path.clone(),
        completions_path: None,
        embeddings_path: None,
        audio_transcriptions_path: None,
        audio_speech_path: None,
        messages_path: preset.messages_path.clone(),
        search_path: account
            .search_path
            .clone()
            .or_else(|| preset.search_path.clone()),
        fetch_path: account
            .fetch_path
            .clone()
            .or_else(|| preset.fetch_path.clone()),
        research_path: account
            .research_path
            .clone()
            .or_else(|| preset.research_path.clone()),
        balance_path: account
            .balance_path
            .clone()
            .or_else(|| preset.balance_path.clone()),
        search_query_field: account
            .search_query_field
            .clone()
            .or_else(|| preset.search_query_field.clone()),
        fetch_urls_field: account
            .fetch_urls_field
            .clone()
            .or_else(|| preset.fetch_urls_field.clone()),
        extra_body: if extra_body.is_empty() {
            None
        } else {
            Some(extra_body)
        },
        session_auth: account
            .session_auth
            .clone()
            .or_else(|| preset.session_auth.clone()),
        keepalive: account.keepalive.clone(),
    }
}

// ---------------------------------------------------------------------------
// Built-in presets (compiled into the binary)
// ---------------------------------------------------------------------------

/// Returns the built-in site-backend compatibility preset for Codex traffic
/// served from `chatgpt.com/backend-api/codex`.
pub fn codex_preset() -> ProviderPreset {
    let mut headers = HashMap::new();
    headers.insert(
        "User-Agent".to_string(),
        "codex_cli_rs/0.116.0 (Mac OS 26.0.1; arm64) Apple_Terminal/464".to_string(),
    );
    headers.insert("Originator".to_string(), "codex_cli_rs".to_string());

    let mut extra_body = HashMap::new();
    extra_body.insert("store".to_string(), Value::Bool(false));

    ProviderPreset {
        id: crate::protocol::chatgpt::official_api::CHATGPT_CODEX_BACKEND_PRESET_ID.to_string(),
        adapter: "openai_compatible".to_string(),
        headers,
        extra_body,
        default_model: Some("gpt-5.4".to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: Some("/responses".to_string()),
        chat_completions_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

pub fn chatgpt_codex_oauth_official_api_preset() -> ProviderPreset {
    let mut preset = codex_preset();
    preset.id = "chatgpt-codex-oauth-official-api".to_string();
    preset
}

/// Returns the built-in preset for the OpenAI Platform official API line.
pub fn openai_preset() -> ProviderPreset {
    ProviderPreset {
        id: crate::protocol::chatgpt::official_api::CHATGPT_OFFICIAL_API_PRESET_ID.to_string(),
        adapter: "openai_compatible".to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: Some("gpt-4o".to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

fn generic_openai_compatible_profile_preset(id: &str) -> ProviderPreset {
    ProviderPreset {
        id: id.to_string(),
        adapter: "openai_compatible".to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: None,
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "groq-openai" preset for Groq's OpenAI-compatible API.
pub fn groq_openai_preset() -> ProviderPreset {
    ProviderPreset {
        chat_completions_path: Some("/chat/completions".to_string()),
        ..generic_openai_compatible_profile_preset("groq-openai")
    }
}

/// Returns the built-in "together-openai" preset for Together's OpenAI-compatible API.
pub fn together_openai_preset() -> ProviderPreset {
    ProviderPreset {
        chat_completions_path: Some("/chat/completions".to_string()),
        ..generic_openai_compatible_profile_preset("together-openai")
    }
}

/// Returns the built-in "openrouter-openai" preset for OpenRouter's OpenAI-compatible API.
pub fn openrouter_openai_preset() -> ProviderPreset {
    generic_openai_compatible_profile_preset("openrouter-openai")
}

/// Returns the built-in "muyuan-openai" preset for Muyuan's OpenAI-compatible API.
pub fn muyuan_openai_preset() -> ProviderPreset {
    generic_openai_compatible_profile_preset("muyuan-openai")
}

/// Returns the built-in "poe-openai" preset for Poe's OpenAI-compatible API.
pub fn poe_openai_preset() -> ProviderPreset {
    generic_openai_compatible_profile_preset("poe-openai")
}

/// Returns the built-in "longcat-openai" preset for LongCat's official model API.
pub fn longcat_openai_preset() -> ProviderPreset {
    generic_openai_compatible_profile_preset("longcat-openai")
}

/// Returns the built-in "deepseek-openai" preset for DeepSeek's OpenAI-compatible API.
pub fn deepseek_openai_preset() -> ProviderPreset {
    generic_openai_compatible_profile_preset("deepseek-openai")
}

/// Returns the built-in "mistral-openai" preset for Mistral's OpenAI-compatible API.
pub fn mistral_openai_preset() -> ProviderPreset {
    generic_openai_compatible_profile_preset("mistral-openai")
}

/// Returns the built-in "xai-openai" preset for xAI's official OpenAI-compatible API.
pub fn xai_openai_preset() -> ProviderPreset {
    generic_openai_compatible_profile_preset("xai-openai")
}

/// Returns the built-in "nvidia-openai" preset for NVIDIA's official OpenAI-compatible API.
pub fn nvidia_openai_preset() -> ProviderPreset {
    ProviderPreset {
        chat_completions_path: Some("/v1/chat/completions".to_string()),
        ..generic_openai_compatible_profile_preset("nvidia-openai")
    }
}

/// Returns the built-in "azure-openai" preset for Azure OpenAI's v1-style
/// OpenAI-compatible API surface.
///
/// Recommended base URL form:
/// `https://<resource>.openai.azure.com/openai/v1`
///
/// This preset uses Azure's `api-key` header mode by default and rewrites the
/// standard OpenAI endpoint paths so they work under the `/openai/v1` base URL.
pub fn azure_openai_preset() -> ProviderPreset {
    ProviderPreset {
        id: "azure-openai".to_string(),
        adapter: "openai_compatible".to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: None,
        auth_mode: Some("api-key".to_string()),
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: Some("/responses".to_string()),
        chat_completions_path: Some("/chat/completions".to_string()),
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "gemini-api" preset for Google's Generative Language API.
pub fn gemini_api_preset() -> ProviderPreset {
    ProviderPreset {
        id: "gemini-api".to_string(),
        adapter: "gemini_api_compatible".to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: None,
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: Some("x-goog-api-key".to_string()),
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "gemini-api-modular" preset for the parallel modular
/// Gemini API surface.
pub fn gemini_api_modular_preset() -> ProviderPreset {
    let mut preset = gemini_api_preset();
    preset.id = "gemini-api-modular".to_string();
    preset.adapter = "gemini_api_modular_compatible".to_string();
    preset
}

/// Returns the built-in "aistudio-official-api" preset for the canonical AI
/// Studio personal official API surface.
pub fn aistudio_official_api_preset() -> ProviderPreset {
    let mut preset = gemini_api_preset();
    preset.id = "aistudio-official-api".to_string();
    preset
}

/// Returns the built-in compatibility preset for the Google Agent Platform
/// publisher-model official surface.
///
/// The underlying transport still uses OAuth bearer auth against the
/// publisher/model REST routes historically associated with Vertex AI Gemini.
pub fn google_agent_platform_preset() -> ProviderPreset {
    ProviderPreset {
        id: "google-agent-platform".to_string(),
        adapter: "gemini_api_compatible".to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: None,
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: Some("authorization".to_string()),
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in canonical preset for the Google Agent Platform
/// official API surface.
pub fn google_agent_platform_official_api_preset() -> ProviderPreset {
    let mut preset = google_agent_platform_preset();
    preset.id = "google-agent-platform-official-api".to_string();
    preset
}

/// Compatibility wrapper for the historical Vertex Gemini preset id.
pub fn vertex_gemini_preset() -> ProviderPreset {
    let mut preset = google_agent_platform_preset();
    preset.id = "vertex-gemini".to_string();
    preset
}

/// Compatibility wrapper for the historical Vertex official API preset id.
pub fn vertex_official_api_preset() -> ProviderPreset {
    let mut preset = google_agent_platform_official_api_preset();
    preset.id = "vertex-official-api".to_string();
    preset
}

/// Returns the built-in "gemini-web-chat" preset for Gemini Web pure-HTTP replay providers.
pub fn gemini_web_chat_preset() -> ProviderPreset {
    let mut headers = HashMap::new();
    headers.insert(
        "User-Agent".to_string(),
        crate::protocol::gemini_web::GEMINI_WEB_DEFAULT_USER_AGENT.to_string(),
    );
    headers.insert("Accept".to_string(), "*/*".to_string());
    headers.insert(
        "Accept-Language".to_string(),
        crate::protocol::gemini_web::GEMINI_WEB_DEFAULT_ACCEPT_LANGUAGE.to_string(),
    );
    headers.insert(
        "Origin".to_string(),
        "https://gemini.google.com".to_string(),
    );
    headers.insert(
        "Referer".to_string(),
        "https://gemini.google.com/".to_string(),
    );
    headers.insert("X-Same-Domain".to_string(), "1".to_string());

    ProviderPreset {
        id: "gemini-web-chat".to_string(),
        adapter: "gemini_web_compatible".to_string(),
        headers,
        extra_body: HashMap::new(),
        default_model: Some("gemini-web-chat".to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: Some(
            crate::protocol::gemini_web::GEMINI_WEB_DEFAULT_STREAM_GENERATE_PATH.to_string(),
        ),
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: Some(SessionAuthConfig {
            transport: "cookie".to_string(),
            primary_cookie_name: Some("__Secure-1PSID".to_string()),
            secondary_cookie_name: Some("__Secure-1PSIDTS".to_string()),
            header_name: None,
            expires_at: None,
        }),
        execution_mode: Some(ProviderExecutionMode::DirectHttp),
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "gemini-web-chat-modular" preset for the parallel modular
/// Gemini Web reverse surface.
pub fn gemini_web_chat_modular_preset() -> ProviderPreset {
    let mut preset = gemini_web_chat_preset();
    preset.id = "gemini-web-chat-modular".to_string();
    preset.adapter = "gemini_web_reverse_modular_compatible".to_string();
    preset
}

pub fn chatgpt_web_reverse_preset() -> ProviderPreset {
    let mut headers = HashMap::new();
    headers.insert(
        "User-Agent".to_string(),
        crate::protocol::chatgpt::web_reverse::CHATGPT_WEB_DEFAULT_USER_AGENT.to_string(),
    );
    headers.insert(
        "Accept-Language".to_string(),
        crate::protocol::chatgpt::web_reverse::CHATGPT_WEB_DEFAULT_ACCEPT_LANGUAGE.to_string(),
    );
    headers.insert("Origin".to_string(), "https://chatgpt.com".to_string());
    headers.insert("Referer".to_string(), "https://chatgpt.com/".to_string());

    let mut extra_body = HashMap::new();
    extra_body.insert(
        "clientVersion".to_string(),
        Value::String(
            crate::protocol::chatgpt::web_reverse::CHATGPT_WEB_DEFAULT_CLIENT_VERSION.to_string(),
        ),
    );
    extra_body.insert(
        "clientBuildNumber".to_string(),
        Value::String(
            crate::protocol::chatgpt::web_reverse::CHATGPT_WEB_DEFAULT_CLIENT_BUILD_NUMBER
                .to_string(),
        ),
    );
    extra_body.insert(
        "timezone".to_string(),
        Value::String(
            crate::protocol::chatgpt::web_reverse::CHATGPT_WEB_DEFAULT_TIMEZONE.to_string(),
        ),
    );

    ProviderPreset {
        id: "chatgpt-web-reverse".to_string(),
        adapter: "chatgpt_web_reverse_compatible".to_string(),
        headers,
        extra_body,
        default_model: Some("gpt-5.4".to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: Some(SessionAuthConfig {
            transport: "bearer".to_string(),
            primary_cookie_name: None,
            secondary_cookie_name: None,
            header_name: Some("authorization".to_string()),
            expires_at: None,
        }),
        execution_mode: Some(ProviderExecutionMode::DirectHttp),
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "aistudio-web-reverse" preset for AI Studio browserless
/// reverse-web execution.
///
/// Browser/runtime materials may still be extracted ahead of time, but
/// request-time execution should stay on direct HTTP replay.
pub fn aistudio_web_reverse_preset() -> ProviderPreset {
    let mut headers = HashMap::new();
    headers.insert("Content-Type".to_string(), "application/json".to_string());

    let mut extra_body = HashMap::new();
    extra_body.insert(
        "appUrl".to_string(),
        Value::String(crate::protocol::aistudio_web::AISTUDIO_DEFAULT_APP_URL.to_string()),
    );

    ProviderPreset {
        id: "aistudio-web-reverse".to_string(),
        adapter: "aistudio_web_reverse_compatible".to_string(),
        headers,
        extra_body,
        default_model: Some("gemini-2.5-flash".to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: Some(ProviderExecutionMode::DirectHttp),
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "bedrock-converse" preset for AWS Bedrock Converse.
pub fn bedrock_converse_preset() -> ProviderPreset {
    ProviderPreset {
        id: "bedrock-converse".to_string(),
        adapter: "bedrock_converse_compatible".to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: None,
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: Some("authorization".to_string()),
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "cohere-chat" preset for Cohere Chat.
pub fn cohere_chat_preset() -> ProviderPreset {
    ProviderPreset {
        id: "cohere-chat".to_string(),
        adapter: "cohere_compatible".to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: None,
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "xfyun" preset for XFYun Xingchen's MaaS OpenAI-compatible API.
///
/// XFYun uses the standard OpenAI-style payload shape, but serves chat traffic
/// from `/v2/chat/completions` rather than the default `/v1/chat/completions`.
pub fn xfyun_preset() -> ProviderPreset {
    ProviderPreset {
        id: "xfyun".to_string(),
        adapter: "openai_compatible".to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: None,
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: Some("/v2/chat/completions".to_string()),
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "xfyun-websocket" preset for XFYun Spark/MaaS native
/// signed WebSocket chat transport.
///
/// This preset is distinct from the OpenAI-compatible HTTP surface:
/// - auth is query-string signing based on `apiKey + apiSecret + appId`
/// - transport is WebSocket upstream
/// - the gateway still bridges it back to the caller-visible protocol family
pub fn xfyun_websocket_preset() -> ProviderPreset {
    let mut extra_body = HashMap::new();
    extra_body.insert("uid".to_string(), Value::String("gateway".to_string()));

    ProviderPreset {
        id: "xfyun-websocket".to_string(),
        adapter: "xfyun_websocket_compatible".to_string(),
        headers: HashMap::new(),
        extra_body,
        default_model: None,
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: Some("/v1.1/chat".to_string()),
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "anthropic" preset for Anthropic Claude API.
pub fn anthropic_preset() -> ProviderPreset {
    ProviderPreset {
        id: "anthropic".to_string(),
        adapter: "anthropic_compatible".to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: Some("claude-sonnet-4-6".to_string()),
        auth_mode: None,
        anthropic_version: Some("2023-06-01".to_string()),
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "accio" preset for Alibaba's phoenix-gw (Accio) API.
///
/// The current baseline follows the locally maintained `accio-manager`
/// contract: auth credentials stay in the JSON body as `token`, runtime
/// request headers carry `utdid` + `version`, and optional legacy `appKey`
/// may still be forwarded when a credential explicitly supplies it.
pub fn accio_preset() -> ProviderPreset {
    let mut headers = HashMap::new();
    headers.insert("version".to_string(), "0.5.6".to_string());
    headers.insert("user-agent".to_string(), "node".to_string());

    ProviderPreset {
        id: "accio".to_string(),
        adapter: "accio_compatible".to_string(),
        headers,
        extra_body: HashMap::new(), // token injected per-account; utdid stays in headers
        default_model: Some("claude-sonnet-4-6".to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: Some("/api/adk/llm/generateContent".to_string()),
        chat_completions_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Legacy `qwen` alias preset.
///
/// The old OAuth-based `portal.qwen.ai` route is no longer the official
/// baseline. For compatibility, `preset: qwen` now resolves to the current
/// DashScope OpenAI-compatible API key line.
pub fn qwen_preset() -> ProviderPreset {
    ProviderPreset {
        id: "qwen".to_string(),
        adapter: "openai_compatible".to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: None,
        auth_mode: Some("bearer".to_string()),
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

pub fn qwen_dashscope_openai_preset() -> ProviderPreset {
    ProviderPreset {
        id: "qwen-dashscope-openai".to_string(),
        adapter: "openai_compatible".to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: None,
        auth_mode: Some("bearer".to_string()),
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        // DashScope official OpenAI-compatible base URLs already include `/v1`.
        responses_path: Some("/responses".to_string()),
        chat_completions_path: Some("/chat/completions".to_string()),
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

pub fn qwen_coding_plan_openai_preset() -> ProviderPreset {
    ProviderPreset {
        id: "qwen-coding-plan-openai".to_string(),
        adapter: "openai_compatible".to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: None,
        auth_mode: Some("bearer".to_string()),
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        // Coding Plan OpenAI-compatible base URLs already include `/v1`.
        responses_path: Some("/responses".to_string()),
        chat_completions_path: Some("/chat/completions".to_string()),
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

pub fn qwen_coding_plan_anthropic_preset() -> ProviderPreset {
    ProviderPreset {
        id: "qwen-coding-plan-anthropic".to_string(),
        adapter: "anthropic_compatible".to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: None,
        auth_mode: Some("x-api-key".to_string()),
        anthropic_version: Some("2023-06-01".to_string()),
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

pub fn qwen_web_chat_preset() -> ProviderPreset {
    let mut headers = HashMap::new();
    headers.insert(
        "User-Agent".to_string(),
        crate::protocol::qwen_web::QWEN_WEB_DEFAULT_USER_AGENT.to_string(),
    );
    headers.insert("Connection".to_string(), "keep-alive".to_string());
    headers.insert("Accept".to_string(), "application/json".to_string());
    headers.insert(
        "Accept-Encoding".to_string(),
        "gzip, deflate, br, zstd".to_string(),
    );
    headers.insert(
        "Timezone".to_string(),
        crate::protocol::qwen_web::QWEN_WEB_DEFAULT_TIMEZONE.to_string(),
    );
    headers.insert(
        "sec-ch-ua".to_string(),
        crate::protocol::qwen_web::QWEN_WEB_DEFAULT_SEC_CH_UA.to_string(),
    );
    headers.insert("source".to_string(), "web".to_string());
    headers.insert(
        "Version".to_string(),
        crate::protocol::qwen_web::QWEN_WEB_DEFAULT_VERSION.to_string(),
    );
    headers.insert(
        "bx-v".to_string(),
        crate::protocol::qwen_web::QWEN_WEB_DEFAULT_BX_VERSION.to_string(),
    );
    headers.insert("Sec-Fetch-Site".to_string(), "same-origin".to_string());
    headers.insert("Sec-Fetch-Mode".to_string(), "cors".to_string());
    headers.insert("Sec-Fetch-Dest".to_string(), "empty".to_string());
    headers.insert(
        "Accept-Language".to_string(),
        crate::protocol::qwen_web::QWEN_WEB_DEFAULT_ACCEPT_LANGUAGE.to_string(),
    );

    ProviderPreset {
        id: "qwen-web-chat".to_string(),
        adapter: "qwen_web_compatible".to_string(),
        headers,
        extra_body: HashMap::new(),
        default_model: None,
        auth_mode: Some("bearer".to_string()),
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: Some(
            crate::protocol::qwen_web::QWEN_WEB_DEFAULT_CHAT_COMPLETIONS_PATH.to_string(),
        ),
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: Some(SessionAuthConfig {
            transport: "bearer".to_string(),
            primary_cookie_name: None,
            secondary_cookie_name: None,
            header_name: Some("authorization".to_string()),
            expires_at: None,
        }),
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "grok" preset for Grok's web chat API.
///
/// Grok uses the `grok_compatible` adapter and cookie auth derived from the
/// account's SSO token. The request still travels over JSON POST, but the
/// response body is NDJSON that the gateway translates back to OpenAI SSE.
pub fn grok_preset() -> ProviderPreset {
    let mut headers = HashMap::new();
    headers.insert("Accept".to_string(), "text/event-stream".to_string());
    headers.insert("Origin".to_string(), "https://grok.com".to_string());
    headers.insert("Referer".to_string(), "https://grok.com/".to_string());
    headers.insert(
        "User-Agent".to_string(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36".to_string(),
    );

    ProviderPreset {
        id: "grok".to_string(),
        adapter: "grok_compatible".to_string(),
        headers,
        extra_body: HashMap::new(),
        default_model: Some("grok-3".to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: Some("/rest/app-chat/conversations/new".to_string()),
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: Some(SessionAuthConfig::cookie_defaults()),
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "linkup" preset for Linkup's search API.
pub fn linkup_preset() -> ProviderPreset {
    ProviderPreset {
        id: "linkup".to_string(),
        adapter: SEARCH_API_COMPATIBLE_ADAPTER.to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: Some("linkup-search".to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: Some("/v1/search".to_string()),
        fetch_path: Some("/v1/fetch".to_string()),
        research_path: Some("/v1/research".to_string()),
        balance_path: Some("/v1/credits/balance".to_string()),
        search_query_field: Some("q".to_string()),
        fetch_urls_field: Some("url".to_string()),
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "perplexity" preset for Perplexity's OpenAI-compatible APIs.
pub fn perplexity_preset() -> ProviderPreset {
    ProviderPreset {
        id: "perplexity".to_string(),
        adapter: "openai_compatible".to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: Some("sonar".to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: Some("/v1/responses".to_string()),
        chat_completions_path: Some("/chat/completions".to_string()),
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "perplexity-search" preset for Perplexity's Search API.
pub fn perplexity_search_preset() -> ProviderPreset {
    ProviderPreset {
        id: "perplexity-search".to_string(),
        adapter: SEARCH_API_COMPATIBLE_ADAPTER.to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: Some("perplexity-search".to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: Some("/search".to_string()),
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: Some("query".to_string()),
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "tavily" preset for Tavily's search API.
pub fn tavily_preset() -> ProviderPreset {
    ProviderPreset {
        id: "tavily".to_string(),
        adapter: SEARCH_API_COMPATIBLE_ADAPTER.to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: Some("tavily-search".to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: Some("/search".to_string()),
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: Some("query".to_string()),
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "you" preset for You.com's search API.
pub fn you_preset() -> ProviderPreset {
    ProviderPreset {
        id: "you".to_string(),
        adapter: SEARCH_API_COMPATIBLE_ADAPTER.to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: Some("you-search".to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: Some("X-API-Key".to_string()),
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: Some("/v1/search".to_string()),
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: Some("query".to_string()),
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "exa" preset for Exa's search + contents API.
pub fn exa_preset() -> ProviderPreset {
    ProviderPreset {
        id: "exa".to_string(),
        adapter: SEARCH_API_COMPATIBLE_ADAPTER.to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: Some("exa-search".to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: Some("/search".to_string()),
        fetch_path: Some("/contents".to_string()),
        research_path: None,
        balance_path: None,
        search_query_field: Some("query".to_string()),
        fetch_urls_field: Some("urls".to_string()),
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "jina-search" preset for Jina Search Foundation SERP.
pub fn jina_search_preset() -> ProviderPreset {
    let mut headers = HashMap::new();
    headers.insert("Accept".to_string(), "application/json".to_string());

    ProviderPreset {
        id: "jina-search".to_string(),
        adapter: SEARCH_API_COMPATIBLE_ADAPTER.to_string(),
        headers,
        extra_body: HashMap::new(),
        default_model: Some("jina-search".to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: Some("/search".to_string()),
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: Some("q".to_string()),
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "jina-reader" preset for Jina Reader URL fetches.
pub fn jina_reader_preset() -> ProviderPreset {
    let mut headers = HashMap::new();
    headers.insert("Accept".to_string(), "application/json".to_string());

    ProviderPreset {
        id: "jina-reader".to_string(),
        adapter: SEARCH_API_COMPATIBLE_ADAPTER.to_string(),
        headers,
        extra_body: HashMap::new(),
        default_model: Some("jina-fetch".to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: Some("/".to_string()),
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: Some("url".to_string()),
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "websearchapi" preset for WebSearchAPI's AI search API.
pub fn websearchapi_preset() -> ProviderPreset {
    ProviderPreset {
        id: "websearchapi".to_string(),
        adapter: SEARCH_API_COMPATIBLE_ADAPTER.to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: Some("websearchapi-search".to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: Some("/ai-search".to_string()),
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: Some("query".to_string()),
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "gemini-business" preset for Gemini Business widget APIs.
pub fn gemini_business_preset() -> ProviderPreset {
    let mut headers = HashMap::new();
    headers.insert("Accept".to_string(), "*/*".to_string());
    headers.insert(
        "Origin".to_string(),
        "https://business.gemini.google".to_string(),
    );
    headers.insert(
        "Referer".to_string(),
        "https://business.gemini.google/".to_string(),
    );
    headers.insert(
        "User-Agent".to_string(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36".to_string(),
    );
    headers.insert("x-server-timeout".to_string(), "1800".to_string());
    headers.insert("accept-language".to_string(), "en-US,en;q=0.9".to_string());

    ProviderPreset {
        id: "gemini-business".to_string(),
        adapter: "gemini_business_compatible".to_string(),
        headers,
        extra_body: HashMap::new(),
        default_model: Some(NANO_BANANA_PRO_MODEL.to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: Some("/locations/global/widgetAddContextFile".to_string()),
        chat_completions_path: Some("/locations/global/widgetStreamAssist".to_string()),
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: Some(SessionAuthConfig {
            transport: "bearer".to_string(),
            primary_cookie_name: None,
            secondary_cookie_name: None,
            header_name: Some("Authorization".to_string()),
            expires_at: None,
        }),
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "chataibot" preset for chataibot.pro image APIs.
pub fn chataibot_preset() -> ProviderPreset {
    let mut headers = HashMap::new();
    headers.insert("Accept".to_string(), "*/*".to_string());
    headers.insert("Origin".to_string(), "https://chataibot.pro".to_string());
    headers.insert(
        "Referer".to_string(),
        "https://chataibot.pro/app/chat?chat_id=-2".to_string(),
    );
    headers.insert("x-distribution-channel".to_string(), "web".to_string());
    headers.insert(
        "User-Agent".to_string(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36".to_string(),
    );

    ProviderPreset {
        id: "chataibot".to_string(),
        adapter: "chataibot_compatible".to_string(),
        headers,
        extra_body: HashMap::new(),
        default_model: Some(CHATAIBOT_DEFAULT_MODEL.to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: Some(SessionAuthConfig {
            transport: "cookie".to_string(),
            primary_cookie_name: Some("token".to_string()),
            secondary_cookie_name: None,
            header_name: None,
            expires_at: None,
        }),
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "lumalabs" preset for LumaLabs `uni-1` board image generation.
pub fn lumalabs_preset() -> ProviderPreset {
    let mut headers = HashMap::new();
    headers.insert(
        "Accept".to_string(),
        "application/json, text/plain, */*".to_string(),
    );
    headers.insert("Origin".to_string(), "https://app.lumalabs.ai".to_string());
    headers.insert(
        "User-Agent".to_string(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36".to_string(),
    );
    headers.insert(
        "accept-language".to_string(),
        "zh-CN,zh;q=0.9,en;q=0.8".to_string(),
    );
    headers.insert(
        "x-client-capabilities".to_string(),
        "retry,upgrade_plan".to_string(),
    );

    ProviderPreset {
        id: "lumalabs".to_string(),
        adapter: "lumalabs_compatible".to_string(),
        headers,
        extra_body: HashMap::new(),
        default_model: Some(LUMALABS_DEFAULT_MODEL.to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: Some(SessionAuthConfig {
            transport: "cookie".to_string(),
            primary_cookie_name: Some("wos-session".to_string()),
            secondary_cookie_name: None,
            header_name: None,
            expires_at: None,
        }),
        execution_mode: Some(ProviderExecutionMode::BrowserBacked),
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "gemini-canvas" preset for Gemini Canvas Web
/// reverse-web browser-state providers using direct HTTP replay by default.
pub fn gemini_canvas_preset() -> ProviderPreset {
    let mut headers = HashMap::new();
    let mut extra_body = HashMap::new();
    headers.insert("Accept".to_string(), "application/json".to_string());

    ProviderPreset {
        id: "gemini-canvas".to_string(),
        adapter: "gemini_canvas_compatible".to_string(),
        headers,
        extra_body: {
            extra_body.insert(
                "shareId".to_string(),
                Value::String(
                    crate::protocol::gemini_canvas::GEMINI_CANVAS_DEFAULT_SHARE_ID.to_string(),
                ),
            );
            extra_body.insert(
                "shareUrl".to_string(),
                Value::String(
                    crate::protocol::gemini_canvas::GEMINI_CANVAS_DEFAULT_SHARE_URL.to_string(),
                ),
            );
            extra_body
        },
        default_model: Some(GEMINI_CANVAS_DEFAULT_MODEL.to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: Some(ProviderExecutionMode::DirectHttp),
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "gemini-canvas-browser-relay" preset for the parallel
/// true-canvas browser-owned surface.
pub fn gemini_canvas_browser_relay_preset() -> ProviderPreset {
    let mut preset = gemini_canvas_preset();
    preset.id = "gemini-canvas-browser-relay".to_string();
    preset.adapter = "gemini_canvas_web_reverse_compatible".to_string();
    preset.execution_mode = Some(ProviderExecutionMode::BrowserBacked);
    preset
}

/// Returns the built-in "gemini-canvas-program-relay" preset for the future
/// Canvas-program-owned quota lane.
pub fn gemini_canvas_program_relay_preset() -> ProviderPreset {
    let mut preset = gemini_canvas_browser_relay_preset();
    preset.id = "gemini-canvas-program-relay".to_string();
    preset.adapter = "gemini_canvas_program_web_reverse_compatible".to_string();
    preset.execution_mode = Some(ProviderExecutionMode::DirectHttp);
    preset.extra_body.insert(
        "pureHttpMode".to_string(),
        Value::String("preferred".to_string()),
    );
    preset.extra_body.insert(
        "canvasExecutionOwner".to_string(),
        Value::String("program_owned_relay".to_string()),
    );
    preset.extra_body.insert(
        "canvasQuotaMode".to_string(),
        Value::String("canvas_program".to_string()),
    );
    preset
}

/// Returns the built-in "gemini-canvas-chat" preset for the Gemini Canvas
/// Web reverse-web text surface.
pub fn gemini_canvas_chat_preset() -> ProviderPreset {
    let mut preset = gemini_canvas_preset();
    preset.id = "gemini-canvas-chat".to_string();
    preset.default_model = Some(GEMINI_CANVAS_DEFAULT_TEXT_MODEL.to_string());
    preset.execution_mode = Some(ProviderExecutionMode::DirectHttp);
    preset
}

/// Returns the built-in "kiro" preset for Kiro's session-backed assistant API.
pub fn kiro_preset() -> ProviderPreset {
    ProviderPreset {
        id: "kiro".to_string(),
        adapter: "kiro_compatible".to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: Some(KIRO_DEFAULT_MODEL.to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: Some(KIRO_GENERATE_ASSISTANT_RESPONSE_PATH.to_string()),
        chat_completions_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: Some(SessionAuthConfig {
            transport: "bearer".to_string(),
            primary_cookie_name: None,
            secondary_cookie_name: None,
            header_name: Some("authorization".to_string()),
            expires_at: None,
        }),
        execution_mode: Some(ProviderExecutionMode::DirectHttp),
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "freebuff" preset for FreeBuff's run-backed chat API.
pub fn freebuff_preset() -> ProviderPreset {
    ProviderPreset {
        id: "freebuff".to_string(),
        adapter: "freebuff_compatible".to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: None,
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: None,
        execution_mode: Some(ProviderExecutionMode::DirectHttp),
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "producer" preset for Producer.ai's session-backed
/// reverse-web media surfaces.
pub fn producer_preset() -> ProviderPreset {
    let mut headers = HashMap::new();
    headers.insert(
        "Accept".to_string(),
        "application/json, text/plain, */*".to_string(),
    );
    headers.insert(
        "Origin".to_string(),
        "https://www.flowmusic.app".to_string(),
    );
    headers.insert(
        "Referer".to_string(),
        "https://www.flowmusic.app/".to_string(),
    );
    headers.insert(
        "User-Agent".to_string(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36".to_string(),
    );

    ProviderPreset {
        id: "producer".to_string(),
        adapter: "producer_compatible".to_string(),
        headers,
        extra_body: HashMap::new(),
        default_model: Some(PRODUCER_DEFAULT_MODEL.to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: Some(SessionAuthConfig {
            transport: "bearer".to_string(),
            primary_cookie_name: None,
            secondary_cookie_name: None,
            header_name: Some("authorization".to_string()),
            expires_at: None,
        }),
        execution_mode: Some(ProviderExecutionMode::DirectHttp),
        endpoint_execution_modes: Some(HashMap::from([(
            "videos_generations".to_string(),
            ProviderExecutionMode::BrowserBacked,
        )])),
    }
}

/// Returns the built-in "suno" preset for Suno's session-backed reverse-web media contract.
pub fn suno_preset() -> ProviderPreset {
    let mut headers = HashMap::new();
    headers.insert(
        "Accept".to_string(),
        "application/json, text/plain, */*".to_string(),
    );
    headers.insert("Origin".to_string(), "https://suno.com".to_string());
    headers.insert("Referer".to_string(), "https://suno.com/".to_string());
    headers.insert("accept-language".to_string(), "en-US".to_string());
    headers.insert(
        "sec-ch-ua".to_string(),
        "\"Chromium\";v=\"146\", \"Not-A.Brand\";v=\"24\", \"Microsoft Edge\";v=\"146\""
            .to_string(),
    );
    headers.insert("sec-ch-ua-mobile".to_string(), "?0".to_string());
    headers.insert("sec-ch-ua-platform".to_string(), "\"Windows\"".to_string());
    headers.insert("sec-fetch-dest".to_string(), "empty".to_string());
    headers.insert("sec-fetch-mode".to_string(), "cors".to_string());
    headers.insert("sec-fetch-site".to_string(), "same-site".to_string());
    headers.insert(
        "User-Agent".to_string(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36 Edg/146.0.0.0".to_string(),
    );

    ProviderPreset {
        id: "suno".to_string(),
        adapter: "suno_compatible".to_string(),
        headers,
        extra_body: HashMap::new(),
        default_model: Some(SUNO_DEFAULT_MODEL.to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: Some(SessionAuthConfig {
            transport: "bearer".to_string(),
            primary_cookie_name: None,
            secondary_cookie_name: None,
            header_name: Some("authorization".to_string()),
            expires_at: None,
        }),
        execution_mode: None,
        endpoint_execution_modes: None,
    }
}

/// Returns the built-in "udio" preset for Udio's browser-backed media flow.
pub fn udio_preset() -> ProviderPreset {
    let mut headers = HashMap::new();
    headers.insert(
        "Accept".to_string(),
        "application/json, text/plain, */*".to_string(),
    );
    headers.insert("Origin".to_string(), "https://www.udio.com".to_string());
    headers.insert(
        "Referer".to_string(),
        "https://www.udio.com/create".to_string(),
    );
    headers.insert(
        "User-Agent".to_string(),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36"
            .to_string(),
    );
    headers.insert("Sec-Fetch-Site".to_string(), "same-origin".to_string());
    headers.insert("Sec-Fetch-Mode".to_string(), "cors".to_string());
    headers.insert("Sec-Fetch-Dest".to_string(), "empty".to_string());

    ProviderPreset {
        id: "udio".to_string(),
        adapter: "udio_compatible".to_string(),
        headers,
        extra_body: HashMap::new(),
        default_model: Some(UDIO_DEFAULT_MODEL.to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        session_auth: Some(SessionAuthConfig {
            transport: "cookie".to_string(),
            // Udio's current production frontend stores the Supabase SSR
            // session under this cookie name. Older wrappers used
            // `sb-api-auth-token`, which is now treated as a legacy alias in
            // docs/operator tooling instead of the default runtime name.
            primary_cookie_name: Some("sb-ssr-production-auth-token".to_string()),
            secondary_cookie_name: None,
            header_name: None,
            expires_at: None,
        }),
        execution_mode: Some(ProviderExecutionMode::BrowserBacked),
        endpoint_execution_modes: None,
    }
}

// ---------------------------------------------------------------------------
// Preset registry
// ---------------------------------------------------------------------------

/// Returns all built-in presets indexed by ID.
pub fn builtin_presets() -> HashMap<String, ProviderPreset> {
    let mut presets = vec![
        xai_openai_preset(),
        xfyun_preset(),
        xfyun_websocket_preset(),
        accio_preset(),
        perplexity_preset(),
        gemini_business_preset(),
        lumalabs_preset(),
        gemini_canvas_preset(),
        gemini_canvas_chat_preset(),
        freebuff_preset(),
        producer_preset(),
        suno_preset(),
        udio_preset(),
    ];
    #[cfg(feature = "line-chataibot-web-reverse")]
    {
        presets.push(chataibot_preset());
    }
    #[cfg(feature = "line-grok-web-reverse-api")]
    {
        presets.push(grok_preset());
    }
    #[cfg(feature = "line-perplexity-search-official-vendor-api")]
    {
        presets.push(perplexity_search_preset());
    }
    #[cfg(feature = "line-linkup-search-official-vendor-api")]
    {
        presets.push(linkup_preset());
    }
    #[cfg(feature = "line-tavily-search-official-vendor-api")]
    {
        presets.push(tavily_preset());
    }
    #[cfg(feature = "line-you-search-official-vendor-api")]
    {
        presets.push(you_preset());
    }
    #[cfg(feature = "line-exa-search-official-vendor-api")]
    {
        presets.push(exa_preset());
    }
    #[cfg(feature = "line-jina-search-official-vendor-api")]
    {
        presets.push(jina_search_preset());
    }
    #[cfg(feature = "line-jina-reader-official-vendor-api")]
    {
        presets.push(jina_reader_preset());
    }
    #[cfg(feature = "line-websearchapi-search-official-vendor-api")]
    {
        presets.push(websearchapi_preset());
    }
    #[cfg(feature = "line-kiro-official-vendor-api")]
    {
        presets.push(kiro_preset());
    }
    #[cfg(feature = "line-azure-openai-official-vendor-api")]
    {
        presets.push(azure_openai_preset());
    }
    #[cfg(feature = "line-anthropic-messages-official-model-api")]
    {
        presets.push(anthropic_preset());
    }
    #[cfg(feature = "line-aws-bedrock-converse-official-model-api")]
    {
        presets.push(bedrock_converse_preset());
    }
    #[cfg(feature = "line-chatgpt-official-api")]
    {
        presets.push(openai_preset());
    }
    #[cfg(feature = "line-chatgpt-codex-oauth-official")]
    {
        presets.push(codex_preset());
        presets.push(chatgpt_codex_oauth_official_api_preset());
    }
    #[cfg(feature = "line-chatgpt-web-reverse")]
    {
        presets.push(chatgpt_web_reverse_preset());
    }
    #[cfg(feature = "line-cohere-chat-official-model-api")]
    {
        presets.push(cohere_chat_preset());
    }
    #[cfg(feature = "line-groq-openai-official-vendor-api")]
    {
        presets.push(groq_openai_preset());
    }
    #[cfg(feature = "line-nvidia-openai-official-vendor-api")]
    {
        presets.push(nvidia_openai_preset());
    }
    #[cfg(feature = "line-together-openai-aggregator-api")]
    {
        presets.push(together_openai_preset());
    }
    #[cfg(feature = "line-openrouter-openai-aggregator-api")]
    {
        presets.push(openrouter_openai_preset());
    }
    #[cfg(feature = "line-muyuan-openai-aggregator-api")]
    {
        presets.push(muyuan_openai_preset());
    }
    #[cfg(feature = "line-poe-openai-aggregator-api")]
    {
        presets.push(poe_openai_preset());
    }
    #[cfg(feature = "line-longcat-openai-official-model-api")]
    {
        presets.push(longcat_openai_preset());
    }
    #[cfg(feature = "line-deepseek-openai-official-model-api")]
    {
        presets.push(deepseek_openai_preset());
    }
    #[cfg(feature = "line-mistral-openai-official-model-api")]
    {
        presets.push(mistral_openai_preset());
    }
    #[cfg(feature = "line-qwen-official-api")]
    {
        presets.push(qwen_preset());
        presets.push(qwen_dashscope_openai_preset());
        presets.push(qwen_coding_plan_openai_preset());
        presets.push(qwen_coding_plan_anthropic_preset());
    }
    #[cfg(feature = "line-qwen-web-reverse")]
    {
        presets.push(qwen_web_chat_preset());
    }
    #[cfg(feature = "line-aistudio-official")]
    {
        presets.push(gemini_api_preset());
        presets.push(aistudio_official_api_preset());
        presets.push(gemini_api_modular_preset());
    }
    #[cfg(feature = "line-google-agent-platform-official")]
    {
        presets.push(google_agent_platform_preset());
        presets.push(google_agent_platform_official_api_preset());
        presets.push(vertex_gemini_preset());
        presets.push(vertex_official_api_preset());
    }
    #[cfg(feature = "line-gemini-web-reverse")]
    {
        presets.push(gemini_web_chat_preset());
        presets.push(gemini_web_chat_modular_preset());
    }
    #[cfg(feature = "line-aistudio-web-reverse")]
    {
        presets.push(aistudio_web_reverse_preset());
    }
    #[cfg(feature = "line-gemini-canvas-program")]
    {
        presets.push(gemini_canvas_browser_relay_preset());
        presets.push(gemini_canvas_program_relay_preset());
    }
    presets.into_iter().map(|p| (p.id.clone(), p)).collect()
}

/// Look up a preset by ID from the built-in registry.
pub fn get_builtin_preset(id: &str) -> Option<ProviderPreset> {
    let canonical_id = match id.trim() {
        "qwen-web" | "qwen-webui" | "qwen-webui-replay" | "qwen-webui-replay-live" => {
            "qwen-web-chat"
        }
        other => other,
    };
    builtin_presets().get(canonical_id).cloned()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_codex_account_has_correct_headers_and_extra_body() {
        let preset = codex_preset();
        let mut account_headers = HashMap::new();
        account_headers.insert("Chatgpt-Account-Id".to_string(), "abc-123".to_string());

        let account = AccountOverrides {
            base_url: "https://chatgpt.com/backend-api/codex".to_string(),
            api_key: "tok_xxx".to_string(),
            headers: account_headers,
            extra_body: HashMap::new(),
            default_model: None,
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            session_auth: None,
            keepalive: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            execution_mode: None,
            endpoint_execution_modes: None,
        };

        let compiled = compile_provider_account(&preset, &account);
        assert_eq!(compiled.adapter, "openai_compatible");
        assert_eq!(compiled.base_url, "https://chatgpt.com/backend-api/codex");
        assert_eq!(compiled.api_key, "tok_xxx");
        assert_eq!(
            compiled.headers.get("User-Agent").unwrap(),
            "codex_cli_rs/0.116.0 (Mac OS 26.0.1; arm64) Apple_Terminal/464"
        );
        assert_eq!(compiled.headers.get("Originator").unwrap(), "codex_cli_rs");
        assert_eq!(
            compiled.headers.get("Chatgpt-Account-Id").unwrap(),
            "abc-123"
        );
        let extra = compiled.extra_body.as_ref().unwrap();
        assert_eq!(extra.get("store").unwrap(), &Value::Bool(false));
    }

    #[cfg(feature = "line-qwen-web-reverse")]
    #[test]
    fn get_builtin_preset_accepts_qwen_web_historical_aliases() {
        let legacy = get_builtin_preset("qwen-web").expect("legacy alias preset");
        assert_eq!(legacy.id, "qwen-web-chat");

        let webui = get_builtin_preset("qwen-webui").expect("legacy webui alias preset");
        assert_eq!(webui.id, "qwen-web-chat");

        let replay = get_builtin_preset("qwen-webui-replay").expect("historical alias preset");
        assert_eq!(replay.id, "qwen-web-chat");

        let replay_live =
            get_builtin_preset("qwen-webui-replay-live").expect("historical live alias preset");
        assert_eq!(replay_live.id, "qwen-web-chat");
    }

    #[test]
    fn compile_second_codex_site_only_differs_in_base_url() {
        let preset = codex_preset();

        let site_a = AccountOverrides {
            base_url: "https://chatgpt.com/backend-api/codex".to_string(),
            api_key: "tok_a".to_string(),
            headers: HashMap::new(),
            extra_body: HashMap::new(),
            default_model: None,
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            session_auth: None,
            keepalive: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            execution_mode: None,
            endpoint_execution_modes: None,
        };
        let site_b = AccountOverrides {
            base_url: "https://mirror.example.com/codex".to_string(),
            api_key: "tok_b".to_string(),
            ..site_a.clone()
        };

        let a = compile_provider_account(&preset, &site_a);
        let b = compile_provider_account(&preset, &site_b);

        // Same preset-derived config
        assert_eq!(a.adapter, b.adapter);
        assert_eq!(a.headers.get("User-Agent"), b.headers.get("User-Agent"));
        assert_eq!(a.headers.get("Originator"), b.headers.get("Originator"));
        assert_eq!(a.extra_body, b.extra_body);

        // Different account-specific config
        assert_ne!(a.base_url, b.base_url);
        assert_ne!(a.api_key, b.api_key);
    }

    #[test]
    fn account_headers_override_preset_headers() {
        let preset = codex_preset();
        let mut h = HashMap::new();
        h.insert("User-Agent".to_string(), "my-custom-agent/1.0".to_string());

        let account = AccountOverrides {
            base_url: "https://example.com".to_string(),
            api_key: "k".to_string(),
            headers: h,
            extra_body: HashMap::new(),
            default_model: None,
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            session_auth: None,
            keepalive: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            execution_mode: None,
            endpoint_execution_modes: None,
        };

        let compiled = compile_provider_account(&preset, &account);
        assert_eq!(
            compiled.headers.get("User-Agent").unwrap(),
            "my-custom-agent/1.0"
        );
        // Originator still inherited from preset
        assert_eq!(compiled.headers.get("Originator").unwrap(), "codex_cli_rs");
    }

    #[test]
    fn account_default_model_overrides_preset() {
        let preset = codex_preset();
        let account = AccountOverrides {
            base_url: "https://example.com".to_string(),
            api_key: "k".to_string(),
            headers: HashMap::new(),
            extra_body: HashMap::new(),
            default_model: Some("gpt-5.1-codex".to_string()),
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            session_auth: None,
            keepalive: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            execution_mode: None,
            endpoint_execution_modes: None,
        };

        let compiled = compile_provider_account(&preset, &account);
        assert_eq!(compiled.default_model.as_deref(), Some("gpt-5.1-codex"));
    }

    #[test]
    fn preset_default_model_used_when_account_has_none() {
        let preset = codex_preset();
        let account = AccountOverrides {
            base_url: "https://example.com".to_string(),
            api_key: "k".to_string(),
            headers: HashMap::new(),
            extra_body: HashMap::new(),
            default_model: None,
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            session_auth: None,
            keepalive: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            execution_mode: None,
            endpoint_execution_modes: None,
        };

        let compiled = compile_provider_account(&preset, &account);
        assert_eq!(compiled.default_model.as_deref(), Some("gpt-5.4"));
    }

    #[test]
    fn empty_extra_body_compiles_to_none() {
        let preset = openai_preset();
        let account = AccountOverrides {
            base_url: "https://api.openai.com".to_string(),
            api_key: "sk-xxx".to_string(),
            headers: HashMap::new(),
            extra_body: HashMap::new(),
            default_model: None,
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            session_auth: None,
            keepalive: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            execution_mode: None,
            endpoint_execution_modes: None,
        };

        let compiled = compile_provider_account(&preset, &account);
        assert!(compiled.extra_body.is_none());
    }

    #[test]
    fn builtin_presets_contains_all() {
        let presets = builtin_presets();
        assert!(presets.contains_key("codex"));
        assert!(presets.contains_key("openai"));
        assert!(presets.contains_key("groq-openai"));
        assert!(presets.contains_key("together-openai"));
        assert!(presets.contains_key("openrouter-openai"));
        assert!(presets.contains_key("deepseek-openai"));
        assert!(presets.contains_key("mistral-openai"));
        assert!(presets.contains_key("xai-openai"));
        assert!(presets.contains_key("nvidia-openai"));
        assert!(presets.contains_key("gemini-api"));
        assert!(presets.contains_key("gemini-api-modular"));
        assert!(presets.contains_key("google-agent-platform"));
        assert!(presets.contains_key("google-agent-platform-official-api"));
        assert!(presets.contains_key("vertex-gemini"));
        assert!(presets.contains_key("gemini-web-chat"));
        assert!(presets.contains_key("gemini-web-chat-modular"));
        assert!(presets.contains_key("bedrock-converse"));
        assert!(presets.contains_key("cohere-chat"));
        assert!(presets.contains_key("xfyun"));
        assert!(presets.contains_key("xfyun-websocket"));
        assert!(presets.contains_key("anthropic"));
        assert!(presets.contains_key("accio"));
        assert!(presets.contains_key("qwen"));
        assert!(presets.contains_key("qwen-dashscope-openai"));
        assert!(presets.contains_key("qwen-coding-plan-openai"));
        assert!(presets.contains_key("qwen-coding-plan-anthropic"));
        assert!(presets.contains_key("qwen-web-chat"));
        assert!(presets.contains_key("grok"));
        assert!(presets.contains_key("perplexity"));
        assert!(presets.contains_key("perplexity-search"));
        assert!(presets.contains_key("linkup"));
        assert!(presets.contains_key("tavily"));
        assert!(presets.contains_key("you"));
        assert!(presets.contains_key("exa"));
        assert!(presets.contains_key("jina-search"));
        assert!(presets.contains_key("jina-reader"));
        assert!(presets.contains_key("websearchapi"));
        assert!(presets.contains_key("gemini-business"));
        assert!(presets.contains_key("chataibot"));
        assert!(presets.contains_key("lumalabs"));
        assert!(presets.contains_key("gemini-canvas"));
        assert!(presets.contains_key("gemini-canvas-browser-relay"));
        assert!(presets.contains_key("gemini-canvas-program-relay"));
        assert!(presets.contains_key("kiro"));
        assert!(presets.contains_key("freebuff"));
        assert!(presets.contains_key("producer"));
    }

    #[test]
    fn openai_profile_presets_use_openai_compatible_adapter() {
        for preset in [
            groq_openai_preset(),
            together_openai_preset(),
            openrouter_openai_preset(),
            deepseek_openai_preset(),
            mistral_openai_preset(),
            xai_openai_preset(),
        ] {
            assert_eq!(preset.adapter, "openai_compatible");
            assert!(preset.default_model.is_none());
        }
    }

    #[test]
    fn gemini_and_google_agent_platform_presets_use_expected_auth_headers() {
        let gemini = gemini_api_preset();
        assert_eq!(gemini.adapter, "gemini_api_compatible");
        assert_eq!(gemini.auth_header_name.as_deref(), Some("x-goog-api-key"));
        assert!(gemini.default_model.is_none());

        let aistudio = aistudio_official_api_preset();
        assert_eq!(aistudio.id, "aistudio-official-api");
        assert_eq!(aistudio.adapter, "gemini_api_compatible");
        assert_eq!(aistudio.auth_header_name.as_deref(), Some("x-goog-api-key"));

        let agent_platform = google_agent_platform_preset();
        assert_eq!(agent_platform.id, "google-agent-platform");
        assert_eq!(agent_platform.adapter, "gemini_api_compatible");
        assert_eq!(
            agent_platform.auth_header_name.as_deref(),
            Some("authorization")
        );
        assert!(agent_platform.default_model.is_none());

        let agent_platform_official = google_agent_platform_official_api_preset();
        assert_eq!(
            agent_platform_official.id,
            "google-agent-platform-official-api"
        );
        assert_eq!(agent_platform_official.adapter, "gemini_api_compatible");
        assert_eq!(
            agent_platform_official.auth_header_name.as_deref(),
            Some("authorization")
        );
    }

    #[test]
    fn bedrock_and_cohere_profile_presets_use_native_adapters() {
        let bedrock = bedrock_converse_preset();
        assert_eq!(bedrock.adapter, "bedrock_converse_compatible");
        assert_eq!(bedrock.auth_header_name.as_deref(), Some("authorization"));
        assert!(bedrock.default_model.is_none());

        let cohere = cohere_chat_preset();
        assert_eq!(cohere.adapter, "cohere_compatible");
        assert!(cohere.default_model.is_none());
    }

    #[test]
    fn freebuff_preset_uses_native_adapter() {
        let preset = freebuff_preset();
        assert_eq!(preset.id, "freebuff");
        assert_eq!(preset.adapter, "freebuff_compatible");
        assert_eq!(
            preset.execution_mode,
            Some(ProviderExecutionMode::DirectHttp)
        );
        assert!(preset.default_model.is_none());
    }

    #[test]
    fn xfyun_preset_uses_v2_chat_completions_path() {
        let preset = xfyun_preset();
        assert_eq!(preset.adapter, "openai_compatible");
        assert_eq!(
            preset.chat_completions_path.as_deref(),
            Some("/v2/chat/completions")
        );
        assert!(preset.responses_path.is_none());
        assert!(preset.default_model.is_none());
    }

    #[test]
    fn xfyun_websocket_preset_uses_native_ws_path() {
        let preset = xfyun_websocket_preset();
        assert_eq!(preset.adapter, "xfyun_websocket_compatible");
        assert_eq!(preset.chat_completions_path.as_deref(), Some("/v1.1/chat"));
        assert_eq!(
            preset.extra_body.get("uid"),
            Some(&Value::String("gateway".to_string()))
        );
        assert!(preset.responses_path.is_none());
    }

    #[test]
    fn groq_and_together_presets_use_nonduplicating_chat_paths() {
        let groq = groq_openai_preset();
        assert_eq!(groq.adapter, "openai_compatible");
        assert_eq!(
            groq.chat_completions_path.as_deref(),
            Some("/chat/completions")
        );
        assert!(groq.responses_path.is_none());

        let together = together_openai_preset();
        assert_eq!(together.adapter, "openai_compatible");
        assert_eq!(
            together.chat_completions_path.as_deref(),
            Some("/chat/completions")
        );
        assert!(together.responses_path.is_none());
    }

    #[test]
    fn perplexity_preset_uses_official_openai_compatible_paths() {
        let preset = perplexity_preset();
        assert_eq!(preset.adapter, "openai_compatible");
        assert_eq!(preset.default_model.as_deref(), Some("sonar"));
        assert_eq!(
            preset.chat_completions_path.as_deref(),
            Some("/chat/completions")
        );
        assert_eq!(preset.responses_path.as_deref(), Some("/v1/responses"));
    }

    #[test]
    fn nvidia_openai_preset_uses_openai_compatible_adapter() {
        let preset = nvidia_openai_preset();
        assert_eq!(preset.id, "nvidia-openai");
        assert_eq!(preset.adapter, "openai_compatible");
        assert_eq!(
            preset.chat_completions_path.as_deref(),
            Some("/v1/chat/completions")
        );
        assert!(preset.responses_path.is_none());
        assert!(preset.default_model.is_none());
    }

    #[test]
    fn perplexity_search_preset_uses_query_field() {
        let preset = perplexity_search_preset();
        assert_eq!(preset.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
        assert_eq!(preset.default_model.as_deref(), Some("perplexity-search"));
        assert_eq!(preset.search_path.as_deref(), Some("/search"));
        assert_eq!(preset.search_query_field.as_deref(), Some("query"));
        assert!(preset.fetch_path.is_none());
        assert!(preset.research_path.is_none());
        assert!(preset.balance_path.is_none());
    }

    #[test]
    fn preset_serialization_roundtrip() {
        let preset = codex_preset();
        let json = serde_json::to_string(&preset).unwrap();
        let restored: ProviderPreset = serde_json::from_str(&json).unwrap();
        assert_eq!(preset.id, restored.id);
        assert_eq!(preset.adapter, restored.adapter);
        assert_eq!(preset.headers, restored.headers);
    }

    // ── accio preset ─────────────────────────────────────────────────────

    #[test]
    fn accio_preset_has_correct_adapter_and_path() {
        let preset = accio_preset();
        assert_eq!(preset.id, "accio");
        assert_eq!(preset.adapter, "accio_compatible");
        assert_eq!(
            preset.responses_path.as_deref(),
            Some("/api/adk/llm/generateContent")
        );
        assert_eq!(preset.default_model.as_deref(), Some("claude-sonnet-4-6"));
    }

    #[test]
    fn accio_preset_has_required_headers() {
        let preset = accio_preset();
        assert_eq!(preset.headers.get("version").unwrap(), "0.5.6");
        assert!(preset.headers.get("appKey").is_none());
        assert_eq!(preset.headers.get("user-agent").unwrap(), "node");
    }

    #[test]
    fn compile_accio_account_merges_token_into_extra_body() {
        let preset = accio_preset();
        let mut extra_body = HashMap::new();
        extra_body.insert("token".to_string(), Value::String("abc123".to_string()));

        let mut account_headers = HashMap::new();
        account_headers.insert("utdid".to_string(), "utd-xxx".to_string());

        let account = AccountOverrides {
            base_url: "https://phoenix-gw.alibaba.com".to_string(),
            api_key: "unused".to_string(),
            headers: account_headers,
            extra_body,
            default_model: None,
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            session_auth: None,
            keepalive: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            execution_mode: None,
            endpoint_execution_modes: None,
        };

        let compiled = compile_provider_account(&preset, &account);
        assert_eq!(compiled.adapter, "accio_compatible");
        assert_eq!(compiled.base_url, "https://phoenix-gw.alibaba.com");
        assert_eq!(compiled.headers.get("utdid").unwrap(), "utd-xxx");
        assert_eq!(compiled.headers.get("version").unwrap(), "0.5.6");
        assert!(compiled.headers.get("appKey").is_none());

        let extra = compiled.extra_body.as_ref().unwrap();
        assert_eq!(
            extra.get("token").unwrap(),
            &Value::String("abc123".to_string())
        );
    }

    // ── qwen presets ────────────────────────────────────────────────────

    #[test]
    fn qwen_legacy_preset_points_to_dashscope_openai_line() {
        let preset = qwen_preset();
        assert_eq!(preset.id, "qwen");
        assert_eq!(preset.adapter, "openai_compatible");
        assert_eq!(preset.auth_mode.as_deref(), Some("bearer"));
        assert!(preset.session_auth.is_none());
        assert!(preset.headers.is_empty());
    }

    #[test]
    fn qwen_web_preset_uses_native_adapter_and_browser_headers() {
        let preset = qwen_web_chat_preset();
        assert_eq!(preset.id, "qwen-web-chat");
        assert_eq!(preset.adapter, "qwen_web_compatible");
        assert_eq!(
            preset.chat_completions_path.as_deref(),
            Some(crate::protocol::qwen_web::QWEN_WEB_DEFAULT_CHAT_COMPLETIONS_PATH)
        );
        assert_eq!(
            preset.headers.get("Accept").map(String::as_str),
            Some("application/json")
        );
        assert_eq!(
            preset.headers.get("User-Agent").map(String::as_str),
            Some(crate::protocol::qwen_web::QWEN_WEB_DEFAULT_USER_AGENT)
        );
        assert_eq!(
            preset.headers.get("source").map(String::as_str),
            Some("web")
        );
        let session_auth = preset.session_auth.as_ref().unwrap();
        assert_eq!(session_auth.transport, "bearer");
        assert_eq!(session_auth.header_name(), Some("authorization"));
    }

    #[test]
    fn gemini_web_preset_uses_cookie_session_auth_and_stream_generate_path() {
        let preset = gemini_web_chat_preset();
        assert_eq!(preset.id, "gemini-web-chat");
        assert_eq!(preset.adapter, "gemini_web_compatible");
        assert_eq!(
            preset.chat_completions_path.as_deref(),
            Some(crate::protocol::gemini_web::GEMINI_WEB_DEFAULT_STREAM_GENERATE_PATH)
        );
        assert_eq!(
            preset.headers.get("Origin").map(String::as_str),
            Some("https://gemini.google.com")
        );
        let session_auth = preset.session_auth.as_ref().unwrap();
        assert_eq!(session_auth.transport, "cookie");
        assert_eq!(session_auth.primary_cookie_name(), "__Secure-1PSID");
        assert_eq!(
            session_auth.secondary_cookie_name(),
            Some("__Secure-1PSIDTS")
        );
    }

    #[test]
    fn gemini_web_modular_preset_uses_parallel_adapter() {
        let preset = gemini_web_chat_modular_preset();
        assert_eq!(preset.id, "gemini-web-chat-modular");
        assert_eq!(preset.adapter, "gemini_web_reverse_modular_compatible");
        assert_eq!(
            preset.execution_mode,
            Some(ProviderExecutionMode::DirectHttp)
        );
    }

    #[test]
    fn chatgpt_web_reverse_preset_uses_bearer_session_auth() {
        let preset = chatgpt_web_reverse_preset();
        assert_eq!(preset.id, "chatgpt-web-reverse");
        assert_eq!(preset.adapter, "chatgpt_web_reverse_compatible");
        assert_eq!(
            preset.execution_mode,
            Some(ProviderExecutionMode::DirectHttp)
        );
        assert_eq!(
            preset.headers.get("Origin").map(String::as_str),
            Some("https://chatgpt.com")
        );
        let session_auth = preset.session_auth.as_ref().unwrap();
        assert_eq!(session_auth.transport, "bearer");
        assert_eq!(session_auth.header_name(), Some("authorization"));
    }

    #[test]
    fn aistudio_web_reverse_preset_is_direct_http_browserless_owner() {
        let preset = aistudio_web_reverse_preset();
        assert_eq!(preset.id, "aistudio-web-reverse");
        assert_eq!(preset.adapter, "aistudio_web_reverse_compatible");
        assert_eq!(
            preset.execution_mode,
            Some(ProviderExecutionMode::DirectHttp)
        );
        assert_eq!(
            preset.extra_body.get("appUrl").and_then(Value::as_str),
            Some(crate::protocol::aistudio_web::AISTUDIO_DEFAULT_APP_URL)
        );
    }

    #[test]
    fn qwen_official_api_presets_use_current_protocols() {
        let dashscope = qwen_dashscope_openai_preset();
        assert_eq!(dashscope.adapter, "openai_compatible");
        assert_eq!(dashscope.auth_mode.as_deref(), Some("bearer"));

        let coding_openai = qwen_coding_plan_openai_preset();
        assert_eq!(coding_openai.adapter, "openai_compatible");
        assert_eq!(coding_openai.auth_mode.as_deref(), Some("bearer"));

        let coding_anthropic = qwen_coding_plan_anthropic_preset();
        assert_eq!(coding_anthropic.adapter, "anthropic_compatible");
        assert_eq!(coding_anthropic.auth_mode.as_deref(), Some("x-api-key"));
        assert_eq!(
            coding_anthropic.anthropic_version.as_deref(),
            Some("2023-06-01")
        );
    }

    #[test]
    fn compile_qwen_web_account_uses_qwen_web_adapter() {
        let preset = qwen_web_chat_preset();
        let account = AccountOverrides {
            base_url: "https://chat.qwen.ai".to_string(),
            api_key: "access-token-xxx".to_string(),
            headers: HashMap::new(),
            extra_body: HashMap::new(),
            default_model: None,
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            session_auth: None,
            keepalive: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            execution_mode: None,
            endpoint_execution_modes: None,
        };

        let compiled = compile_provider_account(&preset, &account);
        assert_eq!(compiled.adapter, "qwen_web_compatible");
        assert_eq!(compiled.api_key, "access-token-xxx");
        assert_eq!(
            compiled.headers.get("source").map(String::as_str),
            Some("web")
        );
        assert_eq!(
            compiled
                .session_auth
                .as_ref()
                .and_then(SessionAuthConfig::header_name),
            Some("authorization")
        );
    }

    // ── grok preset ─────────────────────────────────────────────────────

    #[test]
    fn grok_preset_has_adapter_path_and_browser_headers() {
        let preset = grok_preset();
        assert_eq!(preset.id, "grok");
        assert_eq!(preset.adapter, "grok_compatible");
        assert_eq!(
            preset.chat_completions_path.as_deref(),
            Some("/rest/app-chat/conversations/new")
        );
        assert_eq!(preset.default_model.as_deref(), Some("grok-3"));
        assert_eq!(
            preset
                .session_auth
                .as_ref()
                .map(SessionAuthConfig::primary_cookie_name),
            Some("sso")
        );
        assert_eq!(
            preset.headers.get("Origin").map(String::as_str),
            Some("https://grok.com")
        );
        assert_eq!(
            preset.headers.get("Referer").map(String::as_str),
            Some("https://grok.com/")
        );
    }

    #[test]
    fn compile_grok_account_keeps_sso_token_in_api_key() {
        let preset = grok_preset();
        let account = AccountOverrides {
            base_url: "https://grok.com".to_string(),
            api_key: "sso-token-xxx".to_string(),
            headers: HashMap::new(),
            extra_body: HashMap::new(),
            default_model: None,
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            session_auth: None,
            keepalive: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            execution_mode: None,
            endpoint_execution_modes: None,
        };

        let compiled = compile_provider_account(&preset, &account);
        assert_eq!(compiled.adapter, "grok_compatible");
        assert_eq!(compiled.base_url, "https://grok.com");
        assert_eq!(compiled.api_key, "sso-token-xxx");
        assert_eq!(
            compiled
                .session_auth
                .as_ref()
                .map(SessionAuthConfig::primary_cookie_name),
            Some("sso")
        );
        assert_eq!(
            compiled.chat_completions_path.as_deref(),
            Some("/rest/app-chat/conversations/new")
        );
        assert_eq!(
            compiled.headers.get("Accept").map(String::as_str),
            Some("text/event-stream")
        );
    }

    #[test]
    fn gemini_business_preset_uses_widget_paths_and_bearer_transport() {
        let preset = gemini_business_preset();
        assert_eq!(preset.id, "gemini-business");
        assert_eq!(preset.adapter, "gemini_business_compatible");
        assert_eq!(preset.default_model.as_deref(), Some(NANO_BANANA_PRO_MODEL));
        assert_eq!(
            preset.responses_path.as_deref(),
            Some("/locations/global/widgetAddContextFile")
        );
        assert_eq!(
            preset.chat_completions_path.as_deref(),
            Some("/locations/global/widgetStreamAssist")
        );
        assert_eq!(
            preset
                .session_auth
                .as_ref()
                .map(|cfg| cfg.transport.as_str()),
            Some("bearer")
        );
        assert_eq!(
            preset.headers.get("Origin").map(String::as_str),
            Some("https://business.gemini.google")
        );
    }

    #[test]
    fn chataibot_preset_uses_cookie_transport_and_token_cookie() {
        let preset = chataibot_preset();
        assert_eq!(preset.id, "chataibot");
        assert_eq!(preset.adapter, "chataibot_compatible");
        assert_eq!(
            preset.default_model.as_deref(),
            Some(CHATAIBOT_DEFAULT_MODEL)
        );
        assert_eq!(
            preset
                .session_auth
                .as_ref()
                .map(|cfg| cfg.transport.as_str()),
            Some("cookie")
        );
        assert_eq!(
            preset
                .session_auth
                .as_ref()
                .map(SessionAuthConfig::primary_cookie_name),
            Some("token")
        );
        assert_eq!(
            preset
                .session_auth
                .as_ref()
                .and_then(SessionAuthConfig::secondary_cookie_name),
            None
        );
        assert_eq!(
            preset
                .headers
                .get("x-distribution-channel")
                .map(String::as_str),
            Some("web")
        );
    }

    #[test]
    fn lumalabs_preset_uses_cookie_transport_and_uni_1_default_model() {
        let preset = lumalabs_preset();
        assert_eq!(preset.id, "lumalabs");
        assert_eq!(preset.adapter, "lumalabs_compatible");
        assert_eq!(
            preset.default_model.as_deref(),
            Some(LUMALABS_DEFAULT_MODEL)
        );
        assert_eq!(
            preset
                .session_auth
                .as_ref()
                .map(|cfg| cfg.transport.as_str()),
            Some("cookie")
        );
        assert_eq!(
            preset
                .session_auth
                .as_ref()
                .map(SessionAuthConfig::primary_cookie_name),
            Some("wos-session")
        );
        assert_eq!(
            preset.headers.get("Origin").map(String::as_str),
            Some("https://app.lumalabs.ai")
        );
        assert_eq!(
            preset
                .headers
                .get("x-client-capabilities")
                .map(String::as_str),
            Some("retry,upgrade_plan")
        );
    }

    #[test]
    fn gemini_canvas_preset_defaults_to_direct_http_reverse_web() {
        let preset = gemini_canvas_preset();
        assert_eq!(preset.id, "gemini-canvas");
        assert_eq!(preset.adapter, "gemini_canvas_compatible");
        assert_eq!(
            preset.default_model.as_deref(),
            Some(GEMINI_CANVAS_DEFAULT_MODEL)
        );
        assert!(preset.session_auth.is_none());
        assert_eq!(
            preset.execution_mode,
            Some(ProviderExecutionMode::DirectHttp)
        );
        assert_eq!(
            preset.headers.get("Accept").map(String::as_str),
            Some("application/json")
        );
        assert_eq!(
            preset.extra_body.get("shareId").and_then(Value::as_str),
            Some(crate::protocol::gemini_canvas::GEMINI_CANVAS_DEFAULT_SHARE_ID)
        );
        assert_eq!(
            preset.extra_body.get("shareUrl").and_then(Value::as_str),
            Some(crate::protocol::gemini_canvas::GEMINI_CANVAS_DEFAULT_SHARE_URL)
        );
    }

    #[test]
    fn gemini_canvas_chat_preset_defaults_to_direct_http_reverse_web() {
        let preset = gemini_canvas_chat_preset();
        assert_eq!(preset.id, "gemini-canvas-chat");
        assert_eq!(preset.adapter, "gemini_canvas_compatible");
        assert_eq!(
            preset.default_model.as_deref(),
            Some(GEMINI_CANVAS_DEFAULT_TEXT_MODEL)
        );
        assert_eq!(
            preset.execution_mode,
            Some(ProviderExecutionMode::DirectHttp)
        );
        assert!(preset.extra_body.get("pureHttpMode").is_none());
    }

    #[test]
    fn gemini_canvas_browser_relay_preset_defaults_to_browser_backed() {
        let preset = gemini_canvas_browser_relay_preset();
        assert_eq!(preset.id, "gemini-canvas-browser-relay");
        assert_eq!(preset.adapter, "gemini_canvas_web_reverse_compatible");
        assert_eq!(
            preset.execution_mode,
            Some(ProviderExecutionMode::BrowserBacked)
        );
        assert_eq!(
            preset.extra_body.get("shareId").and_then(Value::as_str),
            Some(crate::protocol::gemini_canvas::GEMINI_CANVAS_DEFAULT_SHARE_ID)
        );
        assert_eq!(
            preset.extra_body.get("shareUrl").and_then(Value::as_str),
            Some(crate::protocol::gemini_canvas::GEMINI_CANVAS_DEFAULT_SHARE_URL)
        );
    }

    #[test]
    fn gemini_canvas_program_relay_preset_marks_program_owner() {
        let preset = gemini_canvas_program_relay_preset();
        assert_eq!(preset.id, "gemini-canvas-program-relay");
        assert_eq!(
            preset.adapter,
            "gemini_canvas_program_web_reverse_compatible"
        );
        assert_eq!(
            preset.execution_mode,
            Some(ProviderExecutionMode::DirectHttp)
        );
        assert_eq!(
            preset
                .extra_body
                .get("pureHttpMode")
                .and_then(Value::as_str),
            Some("preferred")
        );
        assert_eq!(
            preset
                .extra_body
                .get("canvasExecutionOwner")
                .and_then(Value::as_str),
            Some("program_owned_relay")
        );
        assert_eq!(
            preset
                .extra_body
                .get("canvasQuotaMode")
                .and_then(Value::as_str),
            Some("canvas_program")
        );
    }

    #[test]
    fn compile_gemini_canvas_account_preserves_browser_runtime_metadata() {
        let preset = gemini_canvas_preset();
        let account = AccountOverrides {
            base_url: "https://gemini.google.com".to_string(),
            api_key: String::new(),
            headers: HashMap::new(),
            extra_body: HashMap::new(),
            default_model: None,
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            session_auth: None,
            keepalive: Some(KeepaliveConfig {
                service_url: "http://gateway.internal".to_string(),
                ensure_path: None,
                auth_token: None,
                timeout_secs: None,
                refresh_before_secs: Some(300),
            }),
            expires_at: Some("2099-01-01T00:00:00.000Z".to_string()),
            runtime_state_object_key: Some("objects/gemini-canvas/auth-1.json".to_string()),
            account_name: Some("canvas-main".to_string()),
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            execution_mode: None,
            endpoint_execution_modes: None,
        };

        let compiled = compile_provider_account(&preset, &account);
        assert_eq!(compiled.adapter, "gemini_canvas_compatible");
        assert_eq!(
            compiled.runtime_state_object_key.as_deref(),
            Some("objects/gemini-canvas/auth-1.json")
        );
        assert_eq!(compiled.account_name.as_deref(), Some("canvas-main"));
        assert_eq!(
            compiled.expires_at.as_deref(),
            Some("2099-01-01T00:00:00.000Z")
        );
    }

    #[test]
    fn kiro_preset_uses_bearer_runtime_and_default_generate_path() {
        let preset = kiro_preset();
        assert_eq!(preset.id, "kiro");
        assert_eq!(preset.adapter, "kiro_compatible");
        assert_eq!(preset.default_model.as_deref(), Some(KIRO_DEFAULT_MODEL));
        assert_eq!(
            preset.responses_path.as_deref(),
            Some(KIRO_GENERATE_ASSISTANT_RESPONSE_PATH)
        );
        assert_eq!(
            preset
                .session_auth
                .as_ref()
                .map(|cfg| cfg.transport.as_str()),
            Some("bearer")
        );
    }

    #[test]
    fn producer_preset_uses_session_bearer_auth() {
        let preset = producer_preset();
        assert_eq!(preset.id, "producer");
        assert_eq!(preset.adapter, "producer_compatible");
        assert_eq!(
            preset.default_model.as_deref(),
            Some(PRODUCER_DEFAULT_MODEL)
        );
        assert_eq!(
            preset
                .session_auth
                .as_ref()
                .map(|cfg| cfg.transport.as_str()),
            Some("bearer")
        );
        assert_eq!(
            preset
                .session_auth
                .as_ref()
                .and_then(|cfg| cfg.header_name.as_deref()),
            Some("authorization")
        );
        assert_eq!(
            preset.headers.get("Origin").map(String::as_str),
            Some("https://www.flowmusic.app")
        );
    }

    #[test]
    fn udio_preset_uses_cookie_session_auth() {
        let preset = udio_preset();
        assert_eq!(preset.id, "udio");
        assert_eq!(preset.adapter, "udio_compatible");
        assert_eq!(preset.default_model.as_deref(), Some(UDIO_DEFAULT_MODEL));
        assert_eq!(
            preset
                .session_auth
                .as_ref()
                .map(|cfg| cfg.transport.as_str()),
            Some("cookie")
        );
        assert_eq!(
            preset
                .session_auth
                .as_ref()
                .map(SessionAuthConfig::primary_cookie_name),
            Some("sb-ssr-production-auth-token")
        );
        assert_eq!(
            preset.headers.get("Origin").map(String::as_str),
            Some("https://www.udio.com")
        );
        assert_eq!(
            preset.headers.get("Referer").map(String::as_str),
            Some("https://www.udio.com/create")
        );
    }

    #[test]
    fn linkup_preset_has_search_adapter_and_path() {
        let preset = linkup_preset();
        assert_eq!(preset.id, "linkup");
        assert_eq!(preset.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
        assert_eq!(preset.default_model.as_deref(), Some("linkup-search"));
        assert_eq!(preset.search_path.as_deref(), Some("/v1/search"));
        assert_eq!(preset.fetch_path.as_deref(), Some("/v1/fetch"));
        assert_eq!(preset.research_path.as_deref(), Some("/v1/research"));
        assert_eq!(preset.balance_path.as_deref(), Some("/v1/credits/balance"));
        assert_eq!(preset.fetch_urls_field.as_deref(), Some("url"));
    }

    #[test]
    fn tavily_preset_has_search_adapter_and_path() {
        let preset = tavily_preset();
        assert_eq!(preset.id, "tavily");
        assert_eq!(preset.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
        assert_eq!(preset.default_model.as_deref(), Some("tavily-search"));
        assert_eq!(preset.search_path.as_deref(), Some("/search"));
        assert_eq!(preset.search_query_field.as_deref(), Some("query"));
        assert!(preset.fetch_path.is_none());
        assert!(preset.research_path.is_none());
        assert!(preset.balance_path.is_none());
    }

    #[test]
    fn exa_preset_has_search_and_contents_paths() {
        let preset = exa_preset();
        assert_eq!(preset.id, "exa");
        assert_eq!(preset.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
        assert_eq!(preset.default_model.as_deref(), Some("exa-search"));
        assert_eq!(preset.search_path.as_deref(), Some("/search"));
        assert_eq!(preset.fetch_path.as_deref(), Some("/contents"));
        assert!(preset.research_path.is_none());
        assert!(preset.balance_path.is_none());
        assert_eq!(preset.search_query_field.as_deref(), Some("query"));
        assert_eq!(preset.fetch_urls_field.as_deref(), Some("urls"));
    }

    #[test]
    fn jina_search_preset_has_search_path_and_json_accept() {
        let preset = jina_search_preset();
        assert_eq!(preset.id, "jina-search");
        assert_eq!(preset.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
        assert_eq!(preset.default_model.as_deref(), Some("jina-search"));
        assert_eq!(preset.search_path.as_deref(), Some("/search"));
        assert!(preset.fetch_path.is_none());
        assert_eq!(preset.search_query_field.as_deref(), Some("q"));
        assert_eq!(
            preset.headers.get("Accept").map(String::as_str),
            Some("application/json")
        );
    }

    #[test]
    fn jina_reader_preset_has_fetch_path_and_json_accept() {
        let preset = jina_reader_preset();
        assert_eq!(preset.id, "jina-reader");
        assert_eq!(preset.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
        assert_eq!(preset.default_model.as_deref(), Some("jina-fetch"));
        assert_eq!(preset.fetch_path.as_deref(), Some("/"));
        assert!(preset.search_path.is_none());
        assert_eq!(preset.fetch_urls_field.as_deref(), Some("url"));
        assert_eq!(
            preset.headers.get("Accept").map(String::as_str),
            Some("application/json")
        );
    }

    #[test]
    fn you_preset_has_search_adapter_and_custom_auth_header() {
        let preset = you_preset();
        assert_eq!(preset.id, "you");
        assert_eq!(preset.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
        assert_eq!(preset.default_model.as_deref(), Some("you-search"));
        assert_eq!(preset.search_path.as_deref(), Some("/v1/search"));
        assert!(preset.fetch_path.is_none());
        assert!(preset.research_path.is_none());
        assert!(preset.balance_path.is_none());
        assert_eq!(preset.search_query_field.as_deref(), Some("query"));
        assert_eq!(preset.auth_header_name.as_deref(), Some("X-API-Key"));
    }

    #[test]
    fn websearchapi_preset_has_search_adapter_and_path() {
        let preset = websearchapi_preset();
        assert_eq!(preset.id, "websearchapi");
        assert_eq!(preset.adapter, SEARCH_API_COMPATIBLE_ADAPTER);
        assert_eq!(preset.default_model.as_deref(), Some("websearchapi-search"));
        assert_eq!(preset.search_path.as_deref(), Some("/ai-search"));
        assert_eq!(preset.search_query_field.as_deref(), Some("query"));
        assert!(preset.fetch_path.is_none());
        assert!(preset.research_path.is_none());
        assert!(preset.balance_path.is_none());
        assert!(preset.fetch_urls_field.is_none());
    }
}
