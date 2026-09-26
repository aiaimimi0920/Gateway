use super::*;

#[test]
fn runtime_patch_from_bootstrap_prefers_bootstrap_operation() {
    let invocation = GeminiCanvasBrowserInvocationResult {
        operation: "bootstrap_program".to_string(),
        bootstrap_operation: Some("video".to_string()),
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
        invoke_base_url: None,
        music_ws_url: None,
        video_invoke_path: None,
        canvas_program_action: None,
        canvas_program_action_input: None,
        canvas_program_invoke_contract: None,
        last_seen_conversation_id: None,
        last_seen_response_id: None,
        candidate_pairs: Vec::new(),
        stable_program_pair: None,
        latest_response_pair: None,
        aggregate_hints: None,
        captured_at: None,
        last_validated_at: None,
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
        patch.get("canvasProgramOperation").and_then(Value::as_str),
        Some("video")
    );
    assert_eq!(
        patch.get("bootstrapOperation").and_then(Value::as_str),
        Some("video")
    );
}

#[test]
fn runtime_patch_from_bootstrap_promotes_latest_video_pair_when_final_page_is_generic_app() {
    let invocation = GeminiCanvasBrowserInvocationResult {
        operation: "bootstrap_program".to_string(),
        bootstrap_operation: Some("video".to_string()),
        share_url: None,
        share_id: None,
        share_follow_kind: None,
        before_url: None,
        final_url: Some("https://gemini.google.com/app".to_string()),
        page_url: Some("https://gemini.google.com/app".to_string()),
        canvas_program_url: Some("https://gemini.google.com/app/4abc4e7577b6149f".to_string()),
        app_path: Some("/app/4abc4e7577b6149f".to_string()),
        conversation_id: Some("c_4abc4e7577b6149f".to_string()),
        response_id: Some("r_1d583cfb0fa7aee4".to_string()),
        invoke_base_url: None,
        music_ws_url: None,
        video_invoke_path: None,
        canvas_program_action: None,
        canvas_program_action_input: None,
        canvas_program_invoke_contract: None,
        last_seen_conversation_id: Some("c_fe0a3932aa6747f0".to_string()),
        last_seen_response_id: Some("r_c2b6bd19ac795efe".to_string()),
        candidate_pairs: vec![json!({
            "appPath": "/app/4abc4e7577b6149f",
            "conversationId": "c_4abc4e7577b6149f",
            "responseId": "r_1d583cfb0fa7aee4"
        })],
        stable_program_pair: Some(json!({
            "appPath": "/app/4abc4e7577b6149f",
            "conversationId": "c_4abc4e7577b6149f",
            "responseId": "r_1d583cfb0fa7aee4",
            "programUrl": "https://gemini.google.com/app/4abc4e7577b6149f"
        })),
        latest_response_pair: Some(json!({
            "appPath": "/app/fe0a3932aa6747f0",
            "conversationId": "c_fe0a3932aa6747f0",
            "responseId": "r_c2b6bd19ac795efe",
            "programUrl": "https://gemini.google.com/app/fe0a3932aa6747f0"
        })),
        aggregate_hints: None,
        captured_at: Some("2026-05-07T12:21:54.364Z".to_string()),
        last_validated_at: Some("2026-05-07T12:22:21.952Z".to_string()),
        new_chat_clicked: Some(true),
        mode_selected: Some(true),
        body_text: None,
        body_base64: None,
        mime_type: None,
        text: None,
        media: Vec::new(),
    };

    let patch = runtime_patch_from_browser_invocation(&invocation).expect("patch");
    assert_eq!(
        patch.get("canvasProgramUrl").and_then(Value::as_str),
        Some("https://gemini.google.com/app/fe0a3932aa6747f0")
    );
    assert_eq!(
        patch.get("appPath").and_then(Value::as_str),
        Some("/app/fe0a3932aa6747f0")
    );
    assert_eq!(
        patch.get("conversationId").and_then(Value::as_str),
        Some("c_fe0a3932aa6747f0")
    );
    assert_eq!(
        patch.get("responseId").and_then(Value::as_str),
        Some("r_c2b6bd19ac795efe")
    );
}

#[test]
fn parse_program_bootstrap_invocation_response_decodes_success_payload() {
    let body_text = json!({
        "ok": true,
        "status": 200,
        "result": {
            "operation": "bootstrap_program",
            "bootstrapOperation": "image",
            "canvasProgramUrl": "https://gemini.google.com/app/4abc4e7577b6149f",
            "appPath": "/app/4abc4e7577b6149f",
            "conversationId": "c_4abc4e7577b6149f",
            "responseId": "r_1d583cfb0fa7aee4",
            "invokeBaseUrl": "https://canvas-endpoint.example",
            "musicWsUrl": "wss://canvas-endpoint.example/ws/music",
            "videoInvokePath": "/v1beta/models/gemini-video:predictLongRunning",
            "canvasProgramAction": "music_generation",
            "canvasProgramActionInput": "{'prompt': 'A short electronic cue.', 'duration_seconds': 30}",
            "canvasProgramInvokeContract": {
                "operation": "music",
                "transportKind": "canvas_program_ws_candidate",
                "target": "ws://127.0.0.1:9998",
                "wsUrl": "ws://127.0.0.1:9998",
                "apiStyle": "google_generative_language",
                "requestEnvelopeKind": "canvas_proxy_request",
                "actionName": "music_generation",
                "actionInput": "{'prompt': 'A short electronic cue.', 'duration_seconds': 30}",
                "prompt": "A short electronic cue.",
                "durationSeconds": 30,
                "uiState": "player_ready"
            }
        }
    })
    .to_string();

    let invocation = parse_program_bootstrap_invocation_response(
        GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER,
        200,
        &body_text,
    )
    .expect("bootstrap invocation");
    assert_eq!(invocation.operation, "bootstrap_program");
    assert_eq!(invocation.bootstrap_operation.as_deref(), Some("image"));
    assert_eq!(
        invocation.canvas_program_url.as_deref(),
        Some("https://gemini.google.com/app/4abc4e7577b6149f")
    );
    assert_eq!(
        invocation.invoke_base_url.as_deref(),
        Some("https://canvas-endpoint.example")
    );
    assert_eq!(
        invocation.canvas_program_action.as_deref(),
        Some("music_generation")
    );
    assert_eq!(
        invocation.canvas_program_action_input.as_deref(),
        Some("{'prompt': 'A short electronic cue.', 'duration_seconds': 30}")
    );
    assert_eq!(
        invocation
            .canvas_program_invoke_contract
            .as_ref()
            .and_then(Value::as_object)
            .and_then(|value| value.get("transportKind"))
            .and_then(Value::as_str),
        Some("canvas_program_ws_candidate")
    );
}

#[test]
fn parse_connected_fetch_invocation_response_decodes_success_payload() {
    let body_text = json!({
        "ok": true,
        "status": 200,
        "result": {
            "operation": "fetch",
            "status": 200,
            "canvasProgramUrl": "https://gemini.google.com/app/4abc4e7577b6149f",
            "appPath": "/app/4abc4e7577b6149f",
            "conversationId": "c_4abc4e7577b6149f",
            "responseId": "r_1d583cfb0fa7aee4",
            "invokeBaseUrl": "https://canvas-endpoint.example",
            "bodyText": "{\"ok\":true}"
        }
    })
    .to_string();

    let invocation = parse_connected_fetch_invocation_response(
        GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER,
        200,
        &body_text,
    )
    .expect("connected fetch invocation");
    assert_eq!(invocation.status, 200);
    assert_eq!(invocation.body_text.as_deref(), Some("{\"ok\":true}"));
    assert_eq!(
        invocation.invoke_base_url.as_deref(),
        Some("https://canvas-endpoint.example")
    );
}

#[test]
fn parse_program_connected_fetch_json_body_rejects_missing_or_invalid_json() {
    let missing_error =
        parse_connected_fetch_json_body(GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER, None)
            .expect_err("missing JSON body should fail");
    assert_eq!(
        missing_error.code.as_deref(),
        Some("gemini_canvas_connected_fetch_missing_body")
    );
    assert_eq!(
        missing_error.message.as_str(),
        "Gemini Canvas connected fetch completed without a JSON body."
    );

    let invalid_error = parse_connected_fetch_json_body(
        GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER,
        Some("{not-json"),
    )
    .expect_err("invalid JSON body should fail");
    assert_eq!(
        invalid_error.code.as_deref(),
        Some("gemini_canvas_connected_fetch_invalid_json")
    );
    assert!(invalid_error
        .message
        .starts_with("Gemini Canvas connected fetch did not return valid JSON:"));
}

#[test]
fn parse_program_connected_fetch_get_json_body_rejects_missing_or_invalid_json() {
    let missing_error = parse_connected_fetch_get_json_body(
        GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER,
        None,
    )
    .expect_err("missing GET JSON body should fail");
    assert_eq!(
        missing_error.code.as_deref(),
        Some("gemini_canvas_connected_fetch_missing_body")
    );
    assert_eq!(
        missing_error.message.as_str(),
        "Gemini Canvas connected fetch GET completed without a JSON body."
    );

    let invalid_error = parse_connected_fetch_get_json_body(
        GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER,
        Some("{not-json"),
    )
    .expect_err("invalid GET JSON body should fail");
    assert_eq!(
        invalid_error.code.as_deref(),
        Some("gemini_canvas_connected_fetch_invalid_json")
    );
    assert!(invalid_error
        .message
        .starts_with("Gemini Canvas connected fetch GET did not return valid JSON:"));
}

#[test]
fn decode_program_connected_fetch_bytes_reports_standard_contracts() {
    let invalid_base64 = GeminiCanvasBrowserFetchInvocationResult {
        operation: "fetch".to_string(),
        status: 200,
        final_url: None,
        page_url: None,
        canvas_program_url: None,
        app_path: None,
        conversation_id: None,
        response_id: None,
        invoke_base_url: None,
        music_ws_url: None,
        video_invoke_path: None,
        last_seen_conversation_id: None,
        last_seen_response_id: None,
        candidate_pairs: Vec::new(),
        captured_at: None,
        last_validated_at: None,
        content_type: Some("image/png".to_string()),
        headers: std::collections::HashMap::new(),
        body_text: None,
        body_base64: Some("not-base64".to_string()),
    };
    let invalid_error = decode_connected_fetch_body_bytes(
        GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER,
        &invalid_base64,
    )
    .expect_err("invalid base64 should fail");
    assert_eq!(
        invalid_error.code.as_deref(),
        Some("gemini_canvas_connected_fetch_invalid_base64")
    );
    assert!(invalid_error
        .message
        .starts_with("Gemini Canvas connected fetch returned invalid base64 bytes:"));

    let missing_body = GeminiCanvasBrowserFetchInvocationResult {
        operation: "fetch".to_string(),
        status: 200,
        final_url: None,
        page_url: None,
        canvas_program_url: None,
        app_path: None,
        conversation_id: None,
        response_id: None,
        invoke_base_url: None,
        music_ws_url: None,
        video_invoke_path: None,
        last_seen_conversation_id: None,
        last_seen_response_id: None,
        candidate_pairs: Vec::new(),
        captured_at: None,
        last_validated_at: None,
        content_type: None,
        headers: std::collections::HashMap::new(),
        body_text: None,
        body_base64: None,
    };
    let missing_error = decode_connected_fetch_body_bytes(
        GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER,
        &missing_body,
    )
    .expect_err("missing body should fail");
    assert_eq!(
        missing_error.code.as_deref(),
        Some("gemini_canvas_connected_fetch_missing_body")
    );
    assert_eq!(
        missing_error.message.as_str(),
        "Gemini Canvas connected fetch GET completed without a body payload."
    );

    let inline_body = GeminiCanvasBrowserFetchInvocationResult {
        operation: "fetch".to_string(),
        status: 200,
        final_url: None,
        page_url: None,
        canvas_program_url: None,
        app_path: None,
        conversation_id: None,
        response_id: None,
        invoke_base_url: None,
        music_ws_url: None,
        video_invoke_path: None,
        last_seen_conversation_id: None,
        last_seen_response_id: None,
        candidate_pairs: Vec::new(),
        captured_at: None,
        last_validated_at: None,
        content_type: Some("image/png".to_string()),
        headers: std::collections::HashMap::new(),
        body_text: None,
        body_base64: Some(base64::engine::general_purpose::STANDARD.encode(b"png")),
    };
    let (bytes, content_type) = decode_connected_fetch_body_bytes(
        GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER,
        &inline_body,
    )
    .expect("valid base64 should decode");
    assert_eq!(bytes.as_ref(), b"png");
    assert_eq!(content_type.as_deref(), Some("image/png"));
}
