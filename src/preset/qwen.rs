use super::ProviderPreset;
use crate::credential_runtime::SessionAuthConfig;
use std::collections::HashMap;

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
