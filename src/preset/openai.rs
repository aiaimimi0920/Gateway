use super::ProviderPreset;
use serde_json::Value;
use std::collections::HashMap;

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
