use super::*;

#[test]
fn force_program_owned_payload_sets_program_owner_markers() {
    let payload = force_program_owned_payload(&make_payload());
    let extra = payload.extra_body.expect("extra_body");
    assert_eq!(
        extra.get("pureHttpMode").and_then(Value::as_str),
        Some("preferred")
    );
    assert_eq!(
        extra.get("canvasExecutionOwner").and_then(Value::as_str),
        Some("program_owned_relay")
    );
    assert_eq!(
        extra.get("canvasQuotaMode").and_then(Value::as_str),
        Some("canvas_program")
    );
    assert_eq!(
        payload.execution_mode,
        Some(ProviderExecutionMode::BrowserBacked)
    );
}

#[test]
fn force_program_owned_payload_preserves_explicit_pure_http_mode() {
    let mut payload = make_payload();
    payload
        .extra_body
        .as_mut()
        .expect("extra_body")
        .insert("pureHttpMode".to_string(), json!("disabled"));
    let payload = force_program_owned_payload(&payload);
    let extra = payload.extra_body.expect("extra_body");
    assert_eq!(
        extra.get("pureHttpMode").and_then(Value::as_str),
        Some("disabled")
    );
}

#[test]
fn force_program_owned_payload_preserves_required_pure_http_mode() {
    let mut payload = make_payload();
    payload
        .extra_body
        .as_mut()
        .expect("extra_body")
        .insert("pureHttpMode".to_string(), json!("required"));
    let payload = force_program_owned_payload(&payload);
    let extra = payload.extra_body.expect("extra_body");
    assert_eq!(
        extra.get("pureHttpMode").and_then(Value::as_str),
        Some("required")
    );
}

#[test]
fn build_program_bootstrap_probe_input_keeps_probe_shape() {
    let value = build_program_bootstrap_probe_input(&make_payload()).expect("probe input");
    assert_eq!(
        value["provider"].as_str(),
        Some("gemini_canvas_program_bootstrap_probe")
    );
    assert_eq!(
        value["input"]["probe"]["shareUrl"].as_str(),
        Some("https://gemini.google.com/share/canvas-share-789")
    );
    assert_eq!(
        value["input"]["probe"]["expectCanvasProgram"].as_bool(),
        Some(true)
    );
}

#[test]
fn build_program_bootstrap_invocation_input_defaults_to_discovery_only() {
    let payload = make_payload();
    let value = build_program_bootstrap_invocation_input(
        &payload,
        "video",
        "bootstrap prompt",
        "zh-CN",
        std::time::Duration::from_secs(90),
    )
    .expect("bootstrap invocation input");
    assert_eq!(value["discoveryOnly"].as_bool(), Some(true));
    assert_eq!(value["launchCanvasProxyPreview"].as_bool(), Some(true));
    assert_eq!(value["bootstrapOperation"].as_str(), Some("video"));
    assert_eq!(value["bootstrapPrompt"].as_str(), Some("bootstrap prompt"));
    assert_eq!(
        value["runtimeStateObjectKey"].as_str(),
        Some("credential-runtime/gemini-canvas/program")
    );
}

#[test]
fn build_browser_operation_invocation_input_selects_runtime_state_by_operation() {
    let payload = make_payload();
    for operation in ["image", "music", "video"] {
        let value = build_browser_operation_invocation_input(
            &payload,
            operation,
            "prompt",
            "en-US",
            std::time::Duration::from_secs(30),
        )
        .expect("browser operation input");
        assert_eq!(
            value["runtimeStateObjectKey"].as_str(),
            Some("credential-runtime/gemini-canvas/program"),
            "operation={operation}"
        );
    }

    let text = build_browser_operation_invocation_input(
        &payload,
        "text",
        "prompt",
        "en-US",
        std::time::Duration::from_secs(30),
    )
    .expect("text browser operation input");
    assert_eq!(
        text["runtimeStateObjectKey"].as_str(),
        Some("credential-runtime/gemini-canvas/program/storage-state.json")
    );
}

#[test]
fn build_program_connected_fetch_uses_browser_profile_runtime_state() {
    let payload = make_payload();
    let config = relay_config_from_payload(&payload).expect("config");
    let value = build_connected_fetch_invocation_input_with_method_for_payload(
        &payload,
        "https://gemini.google.com",
        &config,
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent",
        "POST",
        Some(&json!({"contents":[]})),
        "canvas_proxy",
        std::time::Duration::from_secs(30),
    );
    assert_eq!(
        value["runtimeStateObjectKey"].as_str(),
        Some("credential-runtime/gemini-canvas/program")
    );
}

#[test]
fn build_program_create_pure_http_request_matches_captured_ujx1bf_shape() {
    let request = build_program_create_pure_http_request(
        "fe24c455a570",
        &crate::protocol::gemini::web_reverse::GeminiWebBootstrap {
            access_token: Some("AT-TOKEN".to_string()),
            build_label: Some("boq-bard-webserver".to_string()),
            session_id: Some("sid-123".to_string()),
            language: "zh-CN".to_string(),
            push_id: None,
            client_pctx: None,
            app_page_path: None,
        },
        412345,
    )
    .expect("create request");
    assert!(request
        .query
        .iter()
        .any(|(key, value)| key == "rpcids" && value == GEMINI_CANVAS_PROGRAM_CREATE_RPCID));
    assert!(request
        .query
        .iter()
        .any(|(key, value)| key == "source-path" && value == "/share/fe24c455a570"));
    assert!(request
        .query
        .iter()
        .any(|(key, value)| key == "bl" && value == "boq-bard-webserver"));
    assert!(request
        .query
        .iter()
        .any(|(key, value)| key == "f.sid" && value == "sid-123"));
    assert!(request
        .form
        .iter()
        .any(|(key, value)| key == "at" && value == "AT-TOKEN"));
    assert!(request.form.iter().any(|(key, value)| {
        key == "f.req" && value.contains("\"ujx1Bf\"") && value.contains("fe24c455a570")
    }));
}

#[test]
fn runtime_patch_from_pure_http_create_response_extracts_canvas_handle() {
    let patch = runtime_patch_from_pure_http_create_response(
        "fe24c455a570",
        Some("text"),
        r#"[[["wrb.fr","ujx1Bf","[\"Browser API Proxy Client\",\"/app/4abc4e7577b6149f\",\"c_4abc4e7577b6149f\",\"r_1d583cfb0fa7aee4\"]"]]]"#,
        "https://gemini.google.com",
    )
    .expect("patch");
    assert_eq!(
        patch.get("canvasProgramUrl").and_then(Value::as_str),
        Some("https://gemini.google.com/app/4abc4e7577b6149f")
    );
    assert_eq!(
        patch.get("appPath").and_then(Value::as_str),
        Some("/app/4abc4e7577b6149f")
    );
    assert_eq!(
        patch.get("conversationId").and_then(Value::as_str),
        Some("c_4abc4e7577b6149f")
    );
    assert_eq!(
        patch.get("responseId").and_then(Value::as_str),
        Some("r_1d583cfb0fa7aee4")
    );
    assert_eq!(
        patch
            .get("stableProgramPair")
            .and_then(|value| value.get("sourceSurface"))
            .and_then(Value::as_str),
        Some("canvas_proxy_client")
    );
    assert_eq!(
        patch.get("canvasProgramOperation").and_then(Value::as_str),
        Some("text")
    );
    assert_eq!(
        patch.get("bootstrapOperation").and_then(Value::as_str),
        Some("text")
    );
}

#[test]
fn runtime_patch_from_pure_http_create_response_derives_app_path_from_conversation_only() {
    let patch = runtime_patch_from_pure_http_create_response(
        "fe24c455a570",
        None,
        r#"[[["wrb.fr","ujx1Bf","[\"Browser API Proxy Client\",\"c_4abc4e7577b6149f\",\"r_1d583cfb0fa7aee4\"]"]]]"#,
        "https://gemini.google.com",
    )
    .expect("patch");
    assert_eq!(
        patch.get("appPath").and_then(Value::as_str),
        Some("/app/4abc4e7577b6149f")
    );
    assert_eq!(
        patch.get("canvasProgramUrl").and_then(Value::as_str),
        Some("https://gemini.google.com/app/4abc4e7577b6149f")
    );
}

#[test]
fn gemini_canvas_program_create_missing_handle_patch_error_matches_contract() {
    let error = gemini_canvas_program_create_missing_handle_patch_error(
        GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER,
    );
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.provider_name.as_deref(),
        Some(GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER)
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_program_create_missing_handle_patch")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas program pure HTTP create completed without emitting a concrete canvas app handle."
    );
}

#[test]
fn gemini_canvas_program_bootstrap_missing_handle_patch_error_matches_contract() {
    let error = gemini_canvas_program_bootstrap_missing_handle_patch_error(
        GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER,
    );
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.provider_name.as_deref(),
        Some(GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER)
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_program_bootstrap_missing_handle_patch")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas program bootstrap completed without emitting runtime handle fields."
    );
}

#[test]
fn gemini_canvas_program_bootstrap_incomplete_error_matches_contract() {
    let error = gemini_canvas_program_bootstrap_incomplete_error(
        GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER,
    );
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.provider_name.as_deref(),
        Some(GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER)
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_program_bootstrap_incomplete")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas program bootstrap did not produce a concrete canvasProgramUrl/appPath/conversationId handle."
    );
}

#[test]
fn runtime_patch_from_browser_invocation_promotes_latest_pair_when_final_url_matches_concrete_app()
{
    let invocation = GeminiCanvasBrowserInvocationResult {
        operation: "music".to_string(),
        bootstrap_operation: Some("music".to_string()),
        share_url: None,
        share_id: None,
        share_follow_kind: None,
        before_url: Some("https://gemini.google.com/canvas".to_string()),
        final_url: Some("https://gemini.google.com/app/68e8ea0057593522".to_string()),
        page_url: Some("https://gemini.google.com/app/68e8ea0057593522".to_string()),
        canvas_program_url: Some("https://gemini.google.com/app/4abc4e7577b6149f".to_string()),
        app_path: Some("/app/4abc4e7577b6149f".to_string()),
        conversation_id: Some("c_4abc4e7577b6149f".to_string()),
        response_id: Some("r_1d583cfb0fa7aee4".to_string()),
        invoke_base_url: None,
        music_ws_url: None,
        video_invoke_path: None,
        canvas_program_action: None,
        canvas_program_action_input: None,
        canvas_program_invoke_contract: Some(json!({
            "operation": "music",
            "transportKind": "canvas_program_ws_candidate",
            "target": "https://contribution.usercontent.google.com/download?filename=sunrise_over_gold.mp4",
            "targetMimeType": "video/mp4",
            "uiState": "music_player_ready"
        })),
        last_seen_conversation_id: Some("c_68e8ea0057593522".to_string()),
        last_seen_response_id: Some("r_495753f072bcd505".to_string()),
        candidate_pairs: vec![],
        stable_program_pair: Some(json!({
            "appPath": "/app/4abc4e7577b6149f",
            "programUrl": "https://gemini.google.com/app/4abc4e7577b6149f",
            "conversationId": "c_4abc4e7577b6149f",
            "responseId": "r_1d583cfb0fa7aee4",
            "sourceSurface": "canvas_proxy_client"
        })),
        latest_response_pair: Some(json!({
            "appPath": "/app/68e8ea0057593522",
            "programUrl": "https://gemini.google.com/app/68e8ea0057593522",
            "conversationId": "c_68e8ea0057593522",
            "responseId": "r_495753f072bcd505"
        })),
        aggregate_hints: None,
        captured_at: None,
        last_validated_at: None,
        new_chat_clicked: Some(true),
        mode_selected: Some(true),
        body_text: None,
        body_base64: None,
        mime_type: None,
        text: None,
        media: vec![],
    };

    let patch = runtime_patch_from_browser_invocation(&invocation).expect("patch");
    assert_eq!(
        patch.get("appPath").and_then(Value::as_str),
        Some("/app/68e8ea0057593522")
    );
    assert_eq!(
        patch.get("conversationId").and_then(Value::as_str),
        Some("c_68e8ea0057593522")
    );
    assert_eq!(
        patch.get("responseId").and_then(Value::as_str),
        Some("r_495753f072bcd505")
    );
}
