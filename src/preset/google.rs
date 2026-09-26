use super::ProviderPreset;
use crate::credential_runtime::SessionAuthConfig;
use crate::protocol::gemini_business::NANO_BANANA_PRO_MODEL;
use crate::protocol::gemini_canvas::{
    GEMINI_CANVAS_DEFAULT_MODEL, GEMINI_CANVAS_DEFAULT_TEXT_MODEL,
};
use crate::routing::candidate::ProviderExecutionMode;
use serde_json::Value;
use std::collections::HashMap;

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
