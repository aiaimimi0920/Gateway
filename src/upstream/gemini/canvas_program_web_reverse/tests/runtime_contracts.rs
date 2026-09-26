use super::*;

#[test]
fn build_connected_fetch_invocation_input_with_method_keeps_get_without_body() {
    let config = relay_config_from_payload(&make_payload()).expect("config");
    let value = build_connected_fetch_invocation_input_with_method(
        "https://gemini.google.com",
        &config,
        "https://generativelanguage.googleapis.com/v1beta/operations/abc123",
        "GET",
        None,
        "canvas_proxy",
        std::time::Duration::from_secs(30),
    );
    assert_eq!(value["googleFetchMode"].as_str(), Some("canvas_proxy"));
    assert_eq!(value["fetchRequest"]["method"].as_str(), Some("GET"));
    assert_eq!(
        value["fetchRequest"]["url"].as_str(),
        Some("https://generativelanguage.googleapis.com/v1beta/operations/abc123")
    );
    assert!(value["fetchRequest"]["jsonBody"].is_null());
}

#[test]
fn program_prefers_canvas_proxy_contract_when_ws_contract_present() {
    let config = relay_config_from_payload(&make_payload()).expect("config");
    assert!(program_prefers_canvas_proxy_contract(&config));
}

#[test]
fn apply_program_connected_fetch_identity_contract_injects_headers_and_query_key() {
    let mut input = build_connected_fetch_invocation_input_with_method(
        "https://gemini.google.com",
        &relay_config_from_payload(&make_payload()).expect("config"),
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent",
        "POST",
        Some(&json!({"contents":[{"parts":[{"text":"hello"}]}]})),
        "canvas_proxy",
        std::time::Duration::from_secs(30),
    );
    let extra_headers = std::collections::HashMap::from([
        (
            "Origin".to_string(),
            "https://gemini.google.com".to_string(),
        ),
        (
            "Referer".to_string(),
            "https://gemini.google.com/app/4abc4e7577b6149f".to_string(),
        ),
    ]);
    apply_program_connected_fetch_identity_contract(
        &mut input,
        Some("0"),
        Some("AIzaProgramPrimary123"),
        Some(&extra_headers),
    )
    .expect("apply contract");
    assert_eq!(
        input["fetchRequest"]["headers"]["X-Goog-AuthUser"].as_str(),
        Some("0")
    );
    assert_eq!(
        input["fetchRequest"]["headers"]["x-goog-api-key"].as_str(),
        Some("AIzaProgramPrimary123")
    );
    assert_eq!(
        input["fetchRequest"]["headers"]["Accept"].as_str(),
        Some("application/json")
    );
    assert_eq!(
        input["fetchRequest"]["headers"]["Origin"].as_str(),
        Some("https://gemini.google.com")
    );
    assert_eq!(
        input["fetchRequest"]["headers"]["Referer"].as_str(),
        Some("https://gemini.google.com/app/4abc4e7577b6149f")
    );
    assert!(input["fetchRequest"]["url"]
        .as_str()
        .is_some_and(|value| value.contains("key=AIzaProgramPrimary123")));
}

#[test]
fn gemini_canvas_program_payload_source_path_prefers_concrete_app_path() {
    let mut payload = make_payload();
    payload
        .extra_body
        .as_mut()
        .expect("extra")
        .insert("appPath".to_string(), json!("/app/4abc4e7577b6149f"));
    payload.extra_body.as_mut().expect("extra").insert(
        "conversationId".to_string(),
        json!("c_ignored_because_app_path_wins"),
    );

    assert_eq!(
        gemini_canvas_program_payload_source_path(&payload).as_deref(),
        Some("/app/4abc4e7577b6149f")
    );
}

#[test]
fn gemini_canvas_program_payload_source_path_falls_back_to_conversation_id() {
    let mut payload = make_payload();
    payload
        .extra_body
        .as_mut()
        .expect("extra")
        .insert("conversationId".to_string(), json!("c_4abc4e7577b6149f"));

    assert_eq!(
        gemini_canvas_program_payload_source_path(&payload).as_deref(),
        Some("/app/4abc4e7577b6149f")
    );
}

#[test]
fn gemini_canvas_program_payload_page_url_prefers_program_url() {
    let mut payload = make_payload();
    payload.extra_body.as_mut().expect("extra").insert(
        "canvasProgramUrl".to_string(),
        json!("https://gemini.google.com/app/4abc4e7577b6149f"),
    );
    payload.extra_body.as_mut().expect("extra").insert(
        "pageUrl".to_string(),
        json!("https://gemini.google.com/app/would-be-ignored"),
    );

    assert_eq!(
        gemini_canvas_program_payload_page_url(&payload, "https://gemini.google.com").as_deref(),
        Some("https://gemini.google.com/app/4abc4e7577b6149f")
    );
}

#[test]
fn gemini_canvas_program_payload_locator_uses_stream_response_id_with_payload_handle() {
    let mut payload = make_payload();
    payload
        .extra_body
        .as_mut()
        .expect("extra")
        .insert("appPath".to_string(), json!("/app/4abc4e7577b6149f"));
    payload
        .extra_body
        .as_mut()
        .expect("extra")
        .insert("conversationId".to_string(), json!("c_4abc4e7577b6149f"));
    payload
        .extra_body
        .as_mut()
        .expect("extra")
        .insert("responseId".to_string(), json!("r_payload_fallback"));
    let stream_body = "[[null,null,\"r_1d583cfb0fa7aee4\"]]";

    let locator =
        gemini_canvas_program_payload_locator(&payload, Some(stream_body)).expect("locator");
    assert_eq!(locator.app_path, "/app/4abc4e7577b6149f");
    assert_eq!(locator.conversation_id, "c_4abc4e7577b6149f");
    assert_eq!(locator.response_id, "r_1d583cfb0fa7aee4");
}

#[test]
fn gemini_canvas_program_payload_locator_falls_back_to_payload_response_id() {
    let mut payload = make_payload();
    payload
        .extra_body
        .as_mut()
        .expect("extra")
        .insert("appPath".to_string(), json!("/app/4abc4e7577b6149f"));
    payload
        .extra_body
        .as_mut()
        .expect("extra")
        .insert("conversationId".to_string(), json!("c_4abc4e7577b6149f"));
    payload
        .extra_body
        .as_mut()
        .expect("extra")
        .insert("responseId".to_string(), json!("r_1d583cfb0fa7aee4"));

    let locator = gemini_canvas_program_payload_locator(&payload, None).expect("locator");
    assert_eq!(locator.response_id, "r_1d583cfb0fa7aee4");
}

#[test]
fn gemini_canvas_program_payload_handle_matches_operation_requires_same_modality() {
    let mut payload = make_payload();
    payload.extra_body.as_mut().expect("extra").insert(
        "canvasProgramUrl".to_string(),
        json!("https://gemini.google.com/app/4abc4e7577b6149f"),
    );
    payload
        .extra_body
        .as_mut()
        .expect("extra")
        .insert("canvasProgramOperation".to_string(), json!("music"));

    assert!(gemini_canvas_program_payload_handle_matches_operation(
        &payload, "music"
    ));
    assert!(!gemini_canvas_program_payload_handle_matches_operation(
        &payload, "image"
    ));
    assert!(!gemini_canvas_program_payload_handle_matches_operation(
        &payload, "video"
    ));
}

#[test]
fn gemini_canvas_program_payload_handle_without_operation_forces_rebootstrap() {
    let mut payload = make_payload();
    payload.extra_body.as_mut().expect("extra").insert(
        "canvasProgramUrl".to_string(),
        json!("https://gemini.google.com/app/4abc4e7577b6149f"),
    );
    payload
        .extra_body
        .as_mut()
        .expect("extra")
        .remove("canvasProgramOperation");
    payload
        .extra_body
        .as_mut()
        .expect("extra")
        .remove("canvas_program_operation");
    payload
        .extra_body
        .as_mut()
        .expect("extra")
        .remove("bootstrapOperation");
    payload
        .extra_body
        .as_mut()
        .expect("extra")
        .remove("bootstrap_operation");

    assert!(!gemini_canvas_program_payload_handle_matches_operation(
        &payload, "image"
    ));
}

#[test]
fn gemini_canvas_program_payload_handle_accepts_invoke_contract_operation() {
    let mut payload = make_payload();
    payload.extra_body = Some(std::collections::HashMap::from([
        (
            "canvasProgramUrl".to_string(),
            json!("https://gemini.google.com/app/4abc4e7577b6149f"),
        ),
        (
            "canvasProgramInvokeContract".to_string(),
            json!({
                "operation": "music",
                "transportKind": "program_music_streamgenerate_candidate",
                "requestEnvelopeKind": "page_stream_generate_form",
                "requestUrl": "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate",
                "requestBody": "f.req=..."
            }),
        ),
    ]));

    assert!(gemini_canvas_program_payload_handle_matches_operation(
        &payload, "music"
    ));
    assert!(!gemini_canvas_program_payload_handle_matches_operation(
        &payload, "video"
    ));
}

#[test]
fn strip_gemini_canvas_program_handle_hints_preserves_share_id_but_drops_concrete_handle() {
    let mut payload = make_payload();
    payload.extra_body = Some(std::collections::HashMap::from([
        ("shareId".to_string(), json!("fe24c455a570")),
        (
            "canvasProgramUrl".to_string(),
            json!("https://gemini.google.com/app/4abc4e7577b6149f"),
        ),
        (
            "invokeBaseUrl".to_string(),
            json!("https://canvas-endpoint.example"),
        ),
        ("appPath".to_string(), json!("/app/4abc4e7577b6149f")),
        ("conversationId".to_string(), json!("c_4abc4e7577b6149f")),
        ("canvasProgramOperation".to_string(), json!("image")),
        (
            "googleApiKey".to_string(),
            json!("AIzaSyCanvasForbiddenKey"),
        ),
        ("apiKeys".to_string(), json!(["AIzaSyCanvasForbiddenKey"])),
        ("authToken".to_string(), json!("page-token")),
    ]));

    let stripped = strip_gemini_canvas_program_handle_hints_from_payload(&payload);
    let stripped_extra = stripped.extra_body.expect("extra_body");
    assert_eq!(
        stripped_extra.get("shareId").and_then(Value::as_str),
        Some("fe24c455a570")
    );
    assert!(!stripped_extra.contains_key("canvasProgramUrl"));
    assert!(!stripped_extra.contains_key("invokeBaseUrl"));
    assert!(!stripped_extra.contains_key("appPath"));
    assert!(!stripped_extra.contains_key("conversationId"));
    assert!(!stripped_extra.contains_key("canvasProgramOperation"));
    assert!(!stripped_extra.contains_key("googleApiKey"));
    assert!(!stripped_extra.contains_key("apiKeys"));
    assert!(!stripped_extra.contains_key("authToken"));
}

#[test]
fn runtime_patch_from_browser_invocation_keeps_handle_fields() {
    let invocation = GeminiCanvasBrowserInvocationResult {
        operation: "image".to_string(),
        bootstrap_operation: None,
        share_url: None,
        share_id: None,
        share_follow_kind: None,
        before_url: None,
        final_url: None,
        page_url: Some("https://gemini.google.com/app".to_string()),
        canvas_program_url: Some("https://gemini.google.com/app/4abc4e7577b6149f".to_string()),
        app_path: Some("/app/4abc4e7577b6149f".to_string()),
        conversation_id: Some("c_4abc4e7577b6149f".to_string()),
        response_id: Some("r_1d583cfb0fa7aee4".to_string()),
        invoke_base_url: Some("https://canvas-endpoint.example".to_string()),
        music_ws_url: Some("wss://canvas-endpoint.example/ws/music".to_string()),
        video_invoke_path: Some("/v1beta/models/gemini-video:predictLongRunning".to_string()),
        canvas_program_action: Some("music_generation".to_string()),
        canvas_program_action_input: Some(
            "{'prompt': 'A short electronic cue.', 'duration_seconds': 30}".to_string(),
        ),
        canvas_program_invoke_contract: Some(json!({
            "operation": "music",
            "transportKind": "canvas_program_ws_candidate",
            "target": "ws://127.0.0.1:9998",
            "wsUrl": "ws://127.0.0.1:9998",
            "apiStyle": "google_generative_language",
            "requestEnvelopeKind": "canvas_proxy_request",
            "actionName": "music_generation",
            "actionInput": "{'prompt': 'A short electronic cue.', 'duration_seconds': 30}",
            "requestUrl": "https://gemini.google.com/_/BardChatUi/data/batchexecute?rpcids=hNvQHb&source-path=%2Fapp%2F4abc4e7577b6149f",
            "requestBody": "f.req=%5Bnull%2C%22A+short+electronic+cue.%22%5D&at=OLD-TOKEN",
            "requestRpcId": "hNvQHb",
            "responseRpcId": "MUAZcd",
            "sourcePath": "/app/4abc4e7577b6149f",
            "modelHint": "models/veo-3.1-lite-generate-001;backend_beyond",
            "prompt": "A short electronic cue.",
            "durationSeconds": 30,
            "uiState": "player_ready"
        })),
        last_seen_conversation_id: Some("c_4b6fc89f965c0d1e".to_string()),
        last_seen_response_id: Some("r_7fa26ee4c86a2f0a".to_string()),
        candidate_pairs: vec![json!({
            "appPath": "/app/4abc4e7577b6149f",
            "conversationId": "c_4abc4e7577b6149f",
            "responseId": "r_1d583cfb0fa7aee4"
        })],
        stable_program_pair: None,
        latest_response_pair: None,
        aggregate_hints: None,
        captured_at: Some("2026-05-05T02:40:36.361Z".to_string()),
        last_validated_at: Some("2026-05-05T02:40:36.361Z".to_string()),
        new_chat_clicked: None,
        mode_selected: None,
        body_text: None,
        body_base64: None,
        mime_type: None,
        text: None,
        media: Vec::new(),
    };

    let patch = runtime_patch_from_browser_invocation(&invocation).expect("patch");
    assert_eq!(
        patch.get("canvasProgramUrl").and_then(Value::as_str),
        Some("https://gemini.google.com/app/4abc4e7577b6149f")
    );
    assert_eq!(
        patch.get("programId").and_then(Value::as_str),
        Some("4abc4e7577b6149f")
    );
    assert_eq!(
        patch.get("lastSeenResponseId").and_then(Value::as_str),
        Some("r_7fa26ee4c86a2f0a")
    );
    assert_eq!(
        patch.get("invokeBaseUrl").and_then(Value::as_str),
        Some("https://canvas-endpoint.example")
    );
    assert_eq!(
        patch.get("musicWsUrl").and_then(Value::as_str),
        Some("wss://canvas-endpoint.example/ws/music")
    );
    assert_eq!(
        patch.get("videoInvokePath").and_then(Value::as_str),
        Some("/v1beta/models/gemini-video:predictLongRunning")
    );
    assert_eq!(
        patch
            .get("candidatePairs")
            .and_then(Value::as_array)
            .map(|value| value.len()),
        Some(1)
    );
    assert_eq!(
        patch.get("canvasProgramOperation").and_then(Value::as_str),
        Some("image")
    );
    assert_eq!(
        patch
            .get("canvasProgramInvokeContract")
            .and_then(Value::as_object)
            .and_then(|value| value.get("transportKind"))
            .and_then(Value::as_str),
        Some("canvas_program_ws_candidate")
    );
    assert_eq!(
        patch
            .get("canvasProgramInvokeContract")
            .and_then(Value::as_object)
            .and_then(|value| value.get("wsUrl"))
            .and_then(Value::as_str),
        Some("ws://127.0.0.1:9998")
    );
    assert_eq!(
        patch
            .get("canvasProgramInvokeContract")
            .and_then(Value::as_object)
            .and_then(|value| value.get("requestUrl"))
            .and_then(Value::as_str),
        Some("https://gemini.google.com/_/BardChatUi/data/batchexecute?rpcids=hNvQHb&source-path=%2Fapp%2F4abc4e7577b6149f")
    );
}
