use super::ProviderPreset;
use crate::credential_runtime::SessionAuthConfig;
use crate::protocol::chataibot::CHATAIBOT_DEFAULT_MODEL;
use crate::protocol::lumalabs::LUMALABS_DEFAULT_MODEL;
use crate::protocol::producer::PRODUCER_DEFAULT_MODEL;
use crate::protocol::suno::SUNO_DEFAULT_MODEL;
use crate::protocol::udio::UDIO_DEFAULT_MODEL;
use crate::routing::candidate::ProviderExecutionMode;
use std::collections::HashMap;

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
