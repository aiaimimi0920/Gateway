use super::*;

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
