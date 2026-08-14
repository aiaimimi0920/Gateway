use base64::Engine;
use serde_json::{json, Value};

use super::*;
use crate::protocol::gemini::shared::GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER;
use crate::routing::candidate::ProviderAccountPayload;
use crate::routing::candidate::ProviderExecutionMode;

fn make_payload() -> ProviderAccountPayload {
    ProviderAccountPayload {
        adapter: GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER.to_string(),
        base_url: "https://gemini.google.com".to_string(),
        api_key: "unused".to_string(),
        credential_id: None,
        expires_at: None,
        runtime_state_object_key: Some(
            "credential-runtime/gemini-canvas/program/storage-state.json".to_string(),
        ),
        account_name: None,
        execution_mode: None,
        endpoint_execution_modes: None,
        default_model: None,
        headers: std::collections::HashMap::new(),
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
        extra_body: Some(std::collections::HashMap::from([
            ("shareId".to_string(), json!("canvas-share-789")),
            (
                "browserRuntimeStateObjectKey".to_string(),
                json!("credential-runtime/gemini-canvas/program"),
            ),
            ("canvasProgramHint".to_string(), json!("quota-lane-probe")),
            (
                "invokeBaseUrl".to_string(),
                json!("https://canvas-endpoint.example"),
            ),
            (
                "musicWsUrl".to_string(),
                json!("wss://canvas-endpoint.example/ws/music"),
            ),
            (
                "videoInvokePath".to_string(),
                json!("/v1beta/models/gemini-video:predictLongRunning"),
            ),
            ("canvasProgramAction".to_string(), json!("music_generation")),
            (
                "canvasProgramActionInput".to_string(),
                json!("{'prompt': 'A short electronic cue.', 'duration_seconds': 30}"),
            ),
            (
                "canvasProgramInvokeContract".to_string(),
                json!({
                    "operation": "music",
                    "transportKind": "canvas_program_ws_candidate",
                    "target": "ws://127.0.0.1:9998",
                    "targetMimeType": "video/mp4",
                    "targetCandidates": [
                        {
                            "url": "https://contribution.usercontent.google.com/download?filename=sunrise_over_gold.mp4",
                            "mimeType": "video/mp4",
                            "kind": "video",
                            "source": "media_node_current_src"
                        }
                    ],
                    "wsUrl": "ws://127.0.0.1:9998",
                    "apiStyle": "google_generative_language",
                    "requestEnvelopeKind": "canvas_proxy_request",
                    "requestUrl": "https://gemini.google.com/_/BardChatUi/data/batchexecute?rpcids=hNvQHb&source-path=%2Fapp%2F4abc4e7577b6149f",
                    "requestBody": "f.req=%5Bnull%2C%22A+short+electronic+cue.%22%5D&at=OLD-TOKEN",
                    "requestRpcId": "hNvQHb",
                    "responseRpcId": "MUAZcd",
                    "sourcePath": "/app/4abc4e7577b6149f",
                    "modelHint": "models/veo-3.1-lite-generate-001;backend_beyond",
                    "actionName": "music_generation",
                    "actionInput": "{'prompt': 'A short electronic cue.', 'duration_seconds': 30}",
                    "prompt": "A short electronic cue.",
                    "durationSeconds": 30,
                    "uiState": "player_ready"
                }),
            ),
        ])),
        session_auth: None,
        keepalive: None,
    }
}

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

#[test]
fn preferred_app_endpoint_invoke_helpers_prefer_explicit_contract_fields() {
    let mut payload = make_payload();
    payload.extra_body.as_mut().expect("extra").insert(
        "invokeBaseUrl".to_string(),
        json!("https://canvas-endpoint.example"),
    );
    payload.extra_body.as_mut().expect("extra").insert(
        "musicWsUrl".to_string(),
        json!("wss://canvas-endpoint.example/ws/music"),
    );
    payload.extra_body.as_mut().expect("extra").insert(
        "videoInvokePath".to_string(),
        json!("/v1beta/models/gemini-video:predictLongRunning"),
    );
    let config = relay_config_from_payload(&payload).expect("config");

    assert_eq!(
        preferred_app_endpoint_invoke_base_url(
            "https://generativelanguage.googleapis.com/v1beta",
            &config,
        ),
        "https://canvas-endpoint.example"
    );
    assert_eq!(
        preferred_app_endpoint_music_ws_url("wss://fallback.example/ws", &config),
        "wss://canvas-endpoint.example/ws/music"
    );
    assert_eq!(
        preferred_app_endpoint_video_request_url(
            "https://generativelanguage.googleapis.com/v1beta",
            "gemini-video",
            &config,
        ),
        "https://canvas-endpoint.example/v1beta/models/gemini-video:predictLongRunning"
    );
}

#[test]
fn missing_gemini_canvas_program_app_endpoint_handle_error_matches_contract() {
    let error = missing_gemini_canvas_program_app_endpoint_handle_error(
        GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER,
    );
    assert_eq!(error.http_status, Some(400));
    assert_eq!(
        error.provider_name.as_deref(),
        Some(GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER)
    );
    assert_eq!(
        error.code.as_deref(),
        Some("missing_gemini_canvas_program_app_endpoint_handle")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas program-owned app-endpoint lane requires a concrete app handle before invocation."
    );
}

#[test]
fn gemini_canvas_program_direct_http_exhausted_error_matches_contract() {
    let error = gemini_canvas_program_direct_http_exhausted_error(
        GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER,
    );
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some(GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER)
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_program_direct_http_exhausted")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas program direct HTTP fetch exhausted all API key transport attempts."
    );
}

#[test]
fn gemini_canvas_program_direct_http_invalid_json_error_matches_contract() {
    let error = gemini_canvas_program_direct_http_invalid_json_error(
        GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER,
        "expected ident",
    );
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.provider_name.as_deref(),
        Some(GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER)
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_program_direct_http_invalid_json")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas program direct HTTP did not return valid JSON: expected ident"
    );
}

#[test]
fn gemini_canvas_program_direct_http_get_invalid_json_error_matches_contract() {
    let error = gemini_canvas_program_direct_http_get_invalid_json_error(
        GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER,
        "expected ident",
    );
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.provider_name.as_deref(),
        Some(GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER)
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_program_direct_http_invalid_json")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas program direct HTTP GET did not return valid JSON: expected ident"
    );
}

#[test]
fn preferred_app_endpoint_action_prompt_uses_matching_action_contract() {
    let config = relay_config_from_payload(&make_payload()).expect("config");
    let hints = preferred_app_endpoint_action_hints(
        crate::protocol::gemini_canvas::GeminiCanvasMediaOperation::Music,
        &config,
    )
    .expect("hints");
    assert_eq!(hints.prompt.as_deref(), Some("A short electronic cue."));
    assert_eq!(hints.duration_seconds, Some(30.0));
    assert_eq!(hints.aspect_ratio, None);
    assert_eq!(
        preferred_app_endpoint_action_prompt(
            crate::protocol::gemini_canvas::GeminiCanvasMediaOperation::Music,
            &config,
        )
        .as_deref(),
        Some("A short electronic cue.")
    );
    assert_eq!(
        preferred_app_endpoint_action_prompt(
            crate::protocol::gemini_canvas::GeminiCanvasMediaOperation::Video,
            &config,
        ),
        None
    );
    assert_eq!(
        preferred_app_endpoint_action_aspect_ratio(
            crate::protocol::gemini_canvas::GeminiCanvasMediaOperation::Video,
            &config,
        ),
        None
    );

    let invoke_contract = preferred_app_endpoint_invoke_contract(
        crate::protocol::gemini_canvas::GeminiCanvasMediaOperation::Music,
        "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1alpha.GenerativeService.BidiGenerateMusic",
        None,
        &config,
    );
    assert_eq!(
        invoke_contract.prompt.as_deref(),
        Some("A short electronic cue.")
    );
    assert_eq!(invoke_contract.duration_seconds, Some(30.0));
    assert_eq!(invoke_contract.aspect_ratio, None);
    assert_eq!(
        invoke_contract.music_ws_url.as_deref(),
        Some("wss://canvas-endpoint.example/ws/music")
    );
    assert_eq!(invoke_contract.video_request_url, None);
    assert_eq!(
        invoke_contract.transport_kind.as_deref(),
        Some("canvas_program_ws_candidate")
    );
    assert_eq!(
        invoke_contract.ws_url.as_deref(),
        Some("ws://127.0.0.1:9998")
    );
    assert_eq!(
        invoke_contract.api_style.as_deref(),
        Some("google_generative_language")
    );
    assert_eq!(
        invoke_contract.request_envelope_kind.as_deref(),
        Some("canvas_proxy_request")
    );
    assert_eq!(
        invoke_contract.request_url.as_deref(),
        Some("https://gemini.google.com/_/BardChatUi/data/batchexecute?rpcids=hNvQHb&source-path=%2Fapp%2F4abc4e7577b6149f")
    );
    assert_eq!(
        invoke_contract.request_body.as_deref(),
        Some("f.req=%5Bnull%2C%22A+short+electronic+cue.%22%5D&at=OLD-TOKEN")
    );
    assert_eq!(invoke_contract.request_rpc_id.as_deref(), Some("hNvQHb"));
    assert_eq!(invoke_contract.response_rpc_id.as_deref(), Some("MUAZcd"));
    assert_eq!(
        invoke_contract.source_path.as_deref(),
        Some("/app/4abc4e7577b6149f")
    );
    assert_eq!(
        invoke_contract.model_hint.as_deref(),
        Some("models/veo-3.1-lite-generate-001;backend_beyond")
    );
}

#[test]
fn preferred_app_endpoint_video_request_url_requires_explicit_app_contract() {
    let mut payload = make_payload();
    payload
        .extra_body
        .as_mut()
        .expect("extra")
        .remove("invokeBaseUrl");
    payload
        .extra_body
        .as_mut()
        .expect("extra")
        .remove("videoInvokePath");
    let config = relay_config_from_payload(&payload).expect("config");
    let invoke_contract = preferred_app_endpoint_invoke_contract(
        crate::protocol::gemini_canvas::GeminiCanvasMediaOperation::Video,
        "https://generativelanguage.googleapis.com/v1beta",
        Some("gemini-video"),
        &config,
    );
    assert_eq!(invoke_contract.video_request_url, None);
}

#[test]
fn preferred_app_endpoint_invoke_contract_reads_music_download_target_candidate() {
    let config = relay_config_from_payload(&make_payload()).expect("config");
    let invoke_contract = preferred_app_endpoint_invoke_contract(
        crate::protocol::gemini_canvas::GeminiCanvasMediaOperation::Music,
        "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1alpha.GenerativeService.BidiGenerateMusic",
        None,
        &config,
    );
    assert_eq!(
        invoke_contract.asset_url.as_deref(),
        Some("https://contribution.usercontent.google.com/download?filename=sunrise_over_gold.mp4")
    );
    assert_eq!(
        invoke_contract.asset_mime_type.as_deref(),
        Some("video/mp4")
    );
    assert_eq!(invoke_contract.asset_kind.as_deref(), Some("video"));
}

#[test]
fn preferred_app_endpoint_invoke_contract_ignores_video_asset_download_target() {
    let mut payload = make_payload();
    payload
        .extra_body
        .as_mut()
        .expect("extra")
        .remove("invokeBaseUrl");
    payload
        .extra_body
        .as_mut()
        .expect("extra")
        .remove("videoInvokePath");
    payload.extra_body.as_mut().expect("extra").insert(
        "canvasProgramInvokeContract".to_string(),
        json!({
            "operation": "video",
            "transportKind": "official_video_http_candidate",
            "target": "https://contribution.usercontent.google.com/download?filename=video.mp4",
            "prompt": "A short clip of a glowing cube."
        }),
    );
    let config = relay_config_from_payload(&payload).expect("config");
    let invoke_contract = preferred_app_endpoint_invoke_contract(
        crate::protocol::gemini_canvas::GeminiCanvasMediaOperation::Video,
        "https://generativelanguage.googleapis.com/v1beta",
        Some("gemini-video"),
        &config,
    );
    assert_eq!(invoke_contract.video_request_url, None);
}

#[test]
fn build_program_batchexecute_request_from_invoke_contract_refreshes_bootstrap_and_prompt() {
    let config = relay_config_from_payload(&make_payload()).expect("config");
    let invoke_contract = preferred_app_endpoint_invoke_contract(
        crate::protocol::gemini_canvas::GeminiCanvasMediaOperation::Music,
        "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1alpha.GenerativeService.BidiGenerateMusic",
        None,
        &config,
    );
    let bootstrap = crate::protocol::gemini::web_reverse::GeminiWebBootstrap {
        access_token: Some("NEW-TOKEN".to_string()),
        build_label: Some("boq-updated".to_string()),
        session_id: Some("1234567890".to_string()),
        language: "zh-CN".to_string(),
        push_id: None,
        client_pctx: None,
        app_page_path: Some("/app/4abc4e7577b6149f".to_string()),
    };

    let request = build_program_batchexecute_request_from_invoke_contract(
        &invoke_contract,
        &bootstrap,
        "zh-CN",
        Some("A replacement prompt."),
    )
    .expect("request build")
    .expect("request");

    assert!(request
        .query
        .iter()
        .any(|(key, value)| key == "rpcids" && value == "hNvQHb"));
    assert!(request
        .query
        .iter()
        .any(|(key, value)| key == "source-path" && value == "/app/4abc4e7577b6149f"));
    assert!(request
        .query
        .iter()
        .any(|(key, value)| key == "bl" && value == "boq-updated"));
    assert!(request
        .query
        .iter()
        .any(|(key, value)| key == "f.sid" && value == "1234567890"));
    assert!(request.query.iter().any(|(key, _)| key == "_reqid"));
    assert!(request
        .form
        .iter()
        .any(|(key, value)| key == "at" && value == "NEW-TOKEN"));
    assert!(request
        .form
        .iter()
        .any(|(_, value)| value.contains("A replacement prompt.")));
}

#[test]
fn build_program_stream_generate_request_from_invoke_contract_preserves_query_and_replaces_prompt()
{
    let mut payload = make_payload();
    payload.extra_body.as_mut().expect("extra").insert(
        "canvasProgramInvokeContract".to_string(),
        json!({
            "operation": "video",
            "transportKind": "program_video_streamgenerate_candidate",
            "requestEnvelopeKind": "page_stream_generate_form",
            "requestUrl": "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate?bl=boq_assistant-bard-web-server_20260507.06_p3&f.sid=-300856583778447260&hl=zh-CN&_reqid=2405763&rt=c",
            "requestBody": "f.req=%5Bnull%2C%22%5B%5B%5C%22ORIGINAL+PROMPT%5C%22%2C0%5D%5D%22%5D&",
            "sourcePath": "/app/656b216d6cd92774",
            "prompt": "ORIGINAL PROMPT"
        }),
    );
    let config = relay_config_from_payload(&payload).expect("config");
    let invoke_contract = preferred_app_endpoint_invoke_contract(
        crate::protocol::gemini_canvas::GeminiCanvasMediaOperation::Video,
        "https://generativelanguage.googleapis.com/v1beta",
        Some("veo-3.1-generate-preview"),
        &config,
    );
    let request = build_program_stream_generate_request_from_invoke_contract(
        &invoke_contract,
        Some("REPLACED PROMPT"),
    )
    .expect("request build")
    .expect("request");
    assert_eq!(
        request.url,
        "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate"
    );
    assert!(
        request
            .query
            .iter()
            .any(|(key, value)| key == "bl"
                && value == "boq_assistant-bard-web-server_20260507.06_p3")
    );
    assert!(request
        .query
        .iter()
        .any(|(key, value)| key == "f.sid" && value == "-300856583778447260"));
    assert!(request.raw_post_data.contains("REPLACED+PROMPT"));
    assert_eq!(
        request.headers.get("content-type").map(String::as_str),
        Some("application/x-www-form-urlencoded;charset=UTF-8")
    );
}

#[test]
fn connected_fetch_mode_helpers_recognize_canvas_page_no_key_variants() {
    assert!(connected_fetch_mode_is_canvas_page_no_key(
        "canvas_page_no_key"
    ));
    assert!(connected_fetch_mode_is_canvas_page_music_no_key(
        "canvas_page_music_no_key"
    ));
    assert!(!connected_fetch_mode_is_canvas_page_no_key("canvas_proxy"));
    assert!(!connected_fetch_mode_is_canvas_page_music_no_key(
        "canvas_preview_music_no_key"
    ));
}

#[test]
fn build_program_stream_generate_request_replaces_only_first_prompt_occurrence() {
    let mut payload = make_payload();
    payload.extra_body = Some(std::collections::HashMap::from([(
        "canvasProgramInvokeContract".to_string(),
        json!({
            "operation": "music",
            "transportKind": "program_music_streamgenerate_candidate",
            "requestEnvelopeKind": "page_stream_generate_form",
            "requestUrl": "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate?bl=boq_assistant-bard-web-server_20260507.06_p3&f.sid=-300856583778447260&hl=zh-CN&_reqid=2405763&rt=c",
            "requestBody": "f.req=%5Bnull%2C%22%5B%5B%5C%22ORIGINAL+PROMPT%5C%22%2C0%5D%2C%5B%5C%22history%5C%22%2C%5C%22ORIGINAL+PROMPT%5C%22%5D%5D%22%5D&",
            "sourcePath": "/app/656b216d6cd92774",
            "prompt": "ORIGINAL PROMPT"
        }),
    )]));
    let config = relay_config_from_payload(&payload).expect("config");
    let invoke_contract = preferred_app_endpoint_invoke_contract(
        crate::protocol::gemini_canvas::GeminiCanvasMediaOperation::Music,
        "https://generativelanguage.googleapis.com/v1beta",
        Some("lyria-3-preview"),
        &config,
    );
    let request = build_program_stream_generate_request_from_invoke_contract(
        &invoke_contract,
        Some("REPLACED PROMPT"),
    )
    .expect("request build")
    .expect("request");
    let decoded_form = url::form_urlencoded::parse(request.raw_post_data.as_bytes())
        .find(|(key, _)| key == "f.req")
        .map(|(_, value)| value.into_owned())
        .expect("f.req");
    assert!(decoded_form.contains("REPLACED PROMPT"));
    assert!(decoded_form.contains("history"));
    assert!(decoded_form.contains("ORIGINAL PROMPT"));
}
