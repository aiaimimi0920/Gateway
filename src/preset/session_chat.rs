use super::ProviderPreset;
use crate::credential_runtime::SessionAuthConfig;
use crate::protocol::kiro::{KIRO_DEFAULT_MODEL, KIRO_GENERATE_ASSISTANT_RESPONSE_PATH};
use crate::routing::candidate::ProviderExecutionMode;
use serde_json::Value;
use std::collections::HashMap;

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
