use super::ProviderPreset;
use serde_json::Value;
use std::collections::HashMap;

/// Returns the built-in site-backend compatibility preset for Codex traffic
/// served from `chatgpt.com/backend-api/codex`.
pub fn codex_preset() -> ProviderPreset {
    let mut headers = HashMap::new();
    headers.insert(
        "User-Agent".to_string(),
        crate::protocol::chatgpt::codex_client::USER_AGENT.to_string(),
    );
    headers.insert("Originator".to_string(), "codex_cli_rs".to_string());

    let mut extra_body = HashMap::new();
    extra_body.insert("store".to_string(), Value::Bool(false));

    ProviderPreset {
        id: crate::protocol::chatgpt::official_api::CHATGPT_CODEX_BACKEND_PRESET_ID.to_string(),
        adapter: "openai_compatible".to_string(),
        headers,
        extra_body,
        default_model: Some("gpt-5.6-luna".to_string()),
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

/// Agnes 国际站 Key；不把订阅 Key 建模为 OAuth 或自动刷新材料。
pub fn agnes_api_preset() -> ProviderPreset {
    ProviderPreset {
        auth_mode: Some(crate::protocol::agnes::AUTH_MODE.into()),
        chat_completions_path: Some("/chat/completions".into()),
        responses_path: Some("/responses".into()),
        ..generic_openai_compatible_profile_preset(crate::protocol::agnes::PRESET)
    }
}

/// River uses stateless OpenAI Chat and Responses with an account-selected model.
pub fn river_api_preset() -> ProviderPreset {
    ProviderPreset {
        chat_completions_path: Some("/chat/completions".to_string()),
        responses_path: Some("/responses".to_string()),
        ..generic_openai_compatible_profile_preset("river-api")
    }
}

/// Luma token inference has a separate Key and billing product from Agents media.
pub fn lumalabs_inference_api_preset() -> ProviderPreset {
    ProviderPreset {
        auth_mode: Some(crate::protocol::luma_inference::AUTH_MODE.into()),
        chat_completions_path: Some("/chat/completions".into()),
        ..generic_openai_compatible_profile_preset(crate::protocol::luma_inference::PRESET)
    }
}

/// Workers AI's account target and restricted Responses support are validated separately.
pub fn cloudflare_workers_ai_preset() -> ProviderPreset {
    ProviderPreset {
        auth_mode: Some(crate::protocol::cloudflare_workers_ai::AUTH_MODE.into()),
        chat_completions_path: Some("/chat/completions".into()),
        responses_path: Some("/responses".into()),
        ..generic_openai_compatible_profile_preset("cloudflare-workers-ai")
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

/// Returns the built-in "poe-openai" preset for Poe's OpenAI-compatible API.
pub fn poe_openai_preset() -> ProviderPreset {
    generic_openai_compatible_profile_preset("poe-openai")
}

/// Returns the built-in "longcat-openai" preset for LongCat's official model API.
pub fn longcat_openai_preset() -> ProviderPreset {
    generic_openai_compatible_profile_preset("longcat-openai")
}

/// Returns the built-in "deepseek-api" preset for DeepSeek's OpenAI-compatible API.
pub fn deepseek_openai_preset() -> ProviderPreset {
    ProviderPreset {
        chat_completions_path: Some("/chat/completions".to_string()),
        ..generic_openai_compatible_profile_preset("deepseek-api")
    }
}

/// Returns the built-in "mistral-api" preset for Mistral's OpenAI-compatible API.
pub fn mistral_openai_preset() -> ProviderPreset {
    generic_openai_compatible_profile_preset("mistral-api")
}

pub fn mistral_studio_preset() -> ProviderPreset {
    generic_openai_compatible_profile_preset("mistral-studio-api")
}

pub fn mistral_vibe_preset() -> ProviderPreset {
    generic_openai_compatible_profile_preset("mistral-vibe-api")
}

pub(super) fn mistral_vibe_user_agent(preset_id: &str, base_url: &str) -> Option<&'static str> {
    // Vibe 3.1.4's Mistral SDK adds this prefix on the actual HTTP request.
    (preset_id == "mistral-vibe-api" && is_mistral_official_base(base_url))
        .then_some(crate::preset::MISTRAL_VIBE_USER_AGENT)
}

pub(crate) fn is_mistral_official_base(base_url: &str) -> bool {
    matches!(
        base_url,
        "https://api.mistral.ai/v1" | "https://api.mistral.ai/v1/"
    )
}

/// Returns the built-in "xai-api" preset for xAI's official OpenAI-compatible API.
pub fn xai_openai_preset() -> ProviderPreset {
    generic_openai_compatible_profile_preset("xai-api")
}

/// Returns the built-in "nvidia-openai" preset for NVIDIA's official OpenAI-compatible API.
pub fn nvidia_openai_preset() -> ProviderPreset {
    ProviderPreset {
        chat_completions_path: Some("/v1/chat/completions".to_string()),
        ..generic_openai_compatible_profile_preset("nvidia-openai")
    }
}

/// Returns the built-in "azure-api" preset for Azure OpenAI's v1-style
/// OpenAI-compatible API surface.
///
/// Recommended base URL form:
/// `https://<resource>.openai.azure.com/openai/v1`
///
/// This preset uses Azure's `api-key` header mode by default and rewrites the
/// standard OpenAI endpoint paths so they work under the `/openai/v1` base URL.
pub fn azure_openai_preset() -> ProviderPreset {
    ProviderPreset {
        id: "azure-api".to_string(),
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

/// Sonar compatibility only; Agent uses a different request and response contract.
pub fn perplexity_preset() -> ProviderPreset {
    ProviderPreset {
        id: "perplexity-api".to_string(),
        adapter: "openai_compatible".to_string(),
        headers: HashMap::new(),
        extra_body: HashMap::new(),
        default_model: Some("sonar".to_string()),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        responses_path: None,
        chat_completions_path: Some("/v1/sonar".to_string()),
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

/// Token Plan is a Key product; all public text endpoints use the Chat wire.
pub fn sensenova_token_plan_preset() -> ProviderPreset {
    ProviderPreset {
        auth_mode: Some(crate::protocol::sensenova::AUTH_MODE.into()),
        chat_completions_path: Some("/chat/completions".into()),
        ..generic_openai_compatible_profile_preset(crate::protocol::sensenova::PRESET)
    }
}

/// SAP's client-credentials owner authenticates one explicit deployment.
pub fn sap_aicore_chat_preset() -> ProviderPreset {
    ProviderPreset {
        auth_mode: Some(crate::protocol::sap_aicore::AUTH_MODE.into()),
        ..generic_openai_compatible_profile_preset(crate::protocol::sap_aicore::PRESET)
    }
}

/// Provider API Chat is explicit; the directory selects models without guessing their protocol.
pub fn commandcode_api_preset() -> ProviderPreset {
    ProviderPreset {
        auth_mode: Some(crate::protocol::commandcode::AUTH_MODE.into()),
        chat_completions_path: Some("/chat/completions".into()),
        ..generic_openai_compatible_profile_preset(crate::protocol::commandcode::PRESET)
    }
}
