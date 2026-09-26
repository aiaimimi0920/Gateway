use super::*;

#[test]
fn runtime_reads_state_key_and_default_share_id() {
    let payload = ProviderAccountPayload {
        adapter: "gemini_canvas_compatible".to_string(),
        base_url: "https://gemini.google.com".to_string(),
        api_key: "AIzaPayloadKeyZero".to_string(),
        credential_id: None,
        expires_at: None,
        runtime_state_object_key: Some(
            "credential-runtime/gemini-canvas-profile/test/user-data".to_string(),
        ),
        account_name: None,
        execution_mode: None,
        endpoint_execution_modes: None,
        default_model: Some(GEMINI_CANVAS_DEFAULT_MODEL.to_string()),
        headers: HashMap::new(),
        auth_mode: None,
        anthropic_version: None,
        beta_headers: None,
        auth_header_name: None,
        auth_token: None,
        responses_path: None,
        chat_completions_path: None,
        completions_path: None,
        embeddings_path: None,
        audio_transcriptions_path: None,
        audio_speech_path: None,
        messages_path: None,
        search_path: None,
        fetch_path: None,
        research_path: None,
        balance_path: None,
        search_query_field: None,
        fetch_urls_field: None,
        extra_body: Some(HashMap::new()),
        session_auth: None,
        keepalive: None,
    };

    let runtime = runtime_from_payload(&payload).unwrap();
    assert_eq!(
        runtime.runtime_state_object_key,
        "credential-runtime/gemini-canvas-profile/test/user-data"
    );
    assert_eq!(runtime.share_id, GEMINI_CANVAS_DEFAULT_SHARE_ID);
    assert_eq!(runtime.api_base_url, GEMINI_CANVAS_DEFAULT_API_BASE_URL);
}

#[test]
fn pure_http_mode_reads_preferred_required_and_disabled() {
    assert_eq!(
        pure_http_mode(&pure_http_mode_payload(Some("preferred"))),
        GeminiCanvasPureHttpMode::Preferred
    );
    assert_eq!(
        pure_http_mode(&pure_http_mode_payload(Some("required"))),
        GeminiCanvasPureHttpMode::Required
    );
    assert_eq!(
        pure_http_mode(&pure_http_mode_payload(Some("disabled"))),
        GeminiCanvasPureHttpMode::Disabled
    );
}

#[test]
fn pure_http_required_only_accepts_required_mode() {
    assert!(!pure_http_required(&pure_http_mode_payload(Some(
        "preferred"
    ))));
    assert!(pure_http_required(&pure_http_mode_payload(Some(
        "required"
    ))));
    assert!(!pure_http_required(&pure_http_mode_payload(Some(
        "disabled"
    ))));
}

#[test]
fn browser_runtime_state_object_key_prefers_storage_state_primary_over_browser_override() {
    let mut payload = pure_http_mode_payload(None);
    payload.runtime_state_object_key =
        Some("credential-runtime/gemini-canvas/storage-state.json".to_string());
    payload.extra_body = Some(HashMap::from([(
        "browserRuntimeStateObjectKey".to_string(),
        Value::String("credential-runtime/gemini-canvas/browser/profile".to_string()),
    )]));

    assert_eq!(
        browser_runtime_state_object_key(&payload).as_deref(),
        Some("credential-runtime/gemini-canvas/storage-state.json")
    );
}

#[test]
fn browser_runtime_state_object_key_prefers_explicit_browser_override_for_profile_dir() {
    let mut payload = pure_http_mode_payload(None);
    payload.runtime_state_object_key =
        Some("credential-runtime/gemini-canvas/browser/profile".to_string());
    payload.extra_body = Some(HashMap::from([(
        "browserRuntimeStateObjectKey".to_string(),
        Value::String("credential-runtime/gemini-canvas/browser/profile-override".to_string()),
    )]));

    assert_eq!(
        browser_runtime_state_object_key(&payload).as_deref(),
        Some("credential-runtime/gemini-canvas/browser/profile-override")
    );
}

#[test]
fn browser_runtime_state_object_key_prefers_explicit_browser_override_when_primary_missing() {
    let mut payload = pure_http_mode_payload(None);
    payload.runtime_state_object_key = None;
    payload.extra_body = Some(HashMap::from([(
        "browserRuntimeStateObjectKey".to_string(),
        Value::String("credential-runtime/gemini-canvas/browser/profile".to_string()),
    )]));

    assert_eq!(
        browser_runtime_state_object_key(&payload).as_deref(),
        Some("credential-runtime/gemini-canvas/browser/profile")
    );
}

#[test]
fn browser_runtime_state_object_key_for_browser_operation_prefers_profile_override_for_image() {
    let mut payload = pure_http_mode_payload(None);
    payload.runtime_state_object_key =
        Some("credential-runtime/gemini-canvas/storage-state.json".to_string());
    payload.extra_body = Some(HashMap::from([(
        "browserRuntimeStateObjectKey".to_string(),
        Value::String("credential-runtime/gemini-canvas/browser/profile".to_string()),
    )]));

    assert_eq!(
        browser_runtime_state_object_key_for_browser_operation(&payload, "image").as_deref(),
        Some("credential-runtime/gemini-canvas/browser/profile")
    );
}

#[test]
fn browser_runtime_state_object_key_for_browser_operation_keeps_storage_state_for_text() {
    let mut payload = pure_http_mode_payload(None);
    payload.runtime_state_object_key =
        Some("credential-runtime/gemini-canvas/storage-state.json".to_string());
    payload.extra_body = Some(HashMap::from([(
        "browserRuntimeStateObjectKey".to_string(),
        Value::String("credential-runtime/gemini-canvas/browser/profile".to_string()),
    )]));

    assert_eq!(
        browser_runtime_state_object_key_for_browser_operation(&payload, "text").as_deref(),
        Some("credential-runtime/gemini-canvas/storage-state.json")
    );
}

#[test]
fn browser_runtime_state_object_key_falls_back_to_primary_runtime_state() {
    let mut payload = pure_http_mode_payload(None);
    payload.runtime_state_object_key =
        Some("credential-runtime/gemini-canvas/storage-state.json".to_string());
    payload.extra_body = Some(HashMap::new());

    assert_eq!(
        browser_runtime_state_object_key(&payload).as_deref(),
        Some("credential-runtime/gemini-canvas/storage-state.json")
    );
}

#[test]
fn browser_cdp_url_reads_explicit_extra_body_value() {
    let mut payload = pure_http_mode_payload(None);
    payload.extra_body = Some(HashMap::from([(
        "browserCdpUrl".to_string(),
        Value::String("http://127.0.0.1:9334".to_string()),
    )]));

    assert_eq!(
        browser_cdp_url(&payload).as_deref(),
        Some("http://127.0.0.1:9334")
    );
}

#[test]
fn browser_cookie_header_prefers_extra_body_cookie_value() {
    let mut payload = pure_http_mode_payload(None);
    payload
        .headers
        .insert("Cookie".to_string(), "stale=1".to_string());
    payload.extra_body = Some(HashMap::from([(
        "cookieHeader".to_string(),
        Value::String("fresh=1; __Secure-1PSID=abc".to_string()),
    )]));

    assert_eq!(
        browser_cookie_header(&payload).as_deref(),
        Some("fresh=1; __Secure-1PSID=abc")
    );
}
