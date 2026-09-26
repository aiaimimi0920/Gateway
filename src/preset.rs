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
use crate::routing::candidate::{ProviderAccountPayload, ProviderExecutionMode};

mod google;
mod media;
mod native_api;
mod openai;
mod qwen;
mod registry;
mod search;
mod session_chat;

pub use google::{
    aistudio_official_api_preset, aistudio_web_reverse_preset, gemini_api_modular_preset,
    gemini_api_preset, gemini_business_preset, gemini_canvas_browser_relay_preset,
    gemini_canvas_chat_preset, gemini_canvas_preset, gemini_canvas_program_relay_preset,
    gemini_web_chat_modular_preset, gemini_web_chat_preset,
    google_agent_platform_official_api_preset, google_agent_platform_preset, vertex_gemini_preset,
    vertex_official_api_preset,
};
pub use media::{chataibot_preset, lumalabs_preset, producer_preset, suno_preset, udio_preset};
pub use native_api::{
    anthropic_preset, bedrock_converse_preset, cohere_chat_preset, xfyun_websocket_preset,
};
pub use openai::{
    azure_openai_preset, chatgpt_codex_oauth_official_api_preset, codex_preset,
    deepseek_openai_preset, groq_openai_preset, longcat_openai_preset, mistral_openai_preset,
    muyuan_openai_preset, nvidia_openai_preset, openai_preset, openrouter_openai_preset,
    perplexity_preset, poe_openai_preset, together_openai_preset, xai_openai_preset, xfyun_preset,
};
pub use qwen::{
    qwen_coding_plan_anthropic_preset, qwen_coding_plan_openai_preset,
    qwen_dashscope_openai_preset, qwen_preset, qwen_web_chat_preset,
};
pub use registry::{builtin_presets, get_builtin_preset};
pub use search::{
    exa_preset, jina_reader_preset, jina_search_preset, linkup_preset, perplexity_search_preset,
    tavily_preset, websearchapi_preset, you_preset,
};
pub use session_chat::{
    accio_preset, chatgpt_web_reverse_preset, freebuff_preset, grok_preset, kiro_preset,
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

#[cfg(test)]
mod tests;
