use super::*;

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
