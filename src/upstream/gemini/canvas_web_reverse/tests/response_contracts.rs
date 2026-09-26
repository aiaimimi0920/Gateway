use super::*;

#[test]
fn parse_browser_invocation_response_decodes_success_payload() {
    let body_text = json!({
        "ok": true,
        "status": 200,
        "result": {
            "operation": "image",
            "canvasProgramUrl": "https://gemini.google.com/app/browser-owned",
            "appPath": "/app/browser-owned",
            "conversationId": "c_browser_owned",
            "responseId": "r_browser_owned",
            "media": [{
                "kind": "image",
                "url": "https://example.com/image.png",
                "mimeType": "image/png",
                "bodyBase64": "cG5n",
                "alt": "preview",
                "width": 512,
                "height": 768
            }]
        }
    })
    .to_string();

    let invocation = parse_browser_invocation_response(
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        200,
        &body_text,
        "image",
    )
    .expect("browser invocation");
    assert_eq!(invocation.operation, "image");
    assert_eq!(invocation.app_path.as_deref(), Some("/app/browser-owned"));
    assert_eq!(invocation.media.len(), 1);
    assert_eq!(invocation.media[0].body_base64.as_deref(), Some("cG5n"));
    assert_eq!(invocation.media[0].alt.as_deref(), Some("preview"));
    assert_eq!(invocation.media[0].width, Some(512));
    assert_eq!(invocation.media[0].height, Some(768));
}

#[test]
fn parse_browser_invocation_response_rejects_operation_mismatch() {
    let body_text = json!({
        "ok": true,
        "status": 200,
        "result": {
            "operation": "video"
        }
    })
    .to_string();

    let error = parse_browser_invocation_response(
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        200,
        &body_text,
        "image",
    )
    .expect_err("operation mismatch");
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_browser_pool_operation_mismatch")
    );
    assert!(error
        .message
        .contains("returned 'video' while 'image' was requested"));
}

#[test]
fn parse_browser_invocation_response_surfaces_pool_error_fields() {
    let body_text = json!({
        "ok": false,
        "error": {
            "status": 401,
            "code": "browser_session_invalid",
            "message": "Browser session expired",
            "body": "{\"message\":\"upstream body\"}"
        }
    })
    .to_string();

    let error = parse_browser_invocation_response(
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        502,
        &body_text,
        "image",
    )
    .expect_err("browser pool error");
    assert_eq!(error.http_status, Some(401));
    assert_eq!(error.code.as_deref(), Some("browser_session_invalid"));
    assert_eq!(error.message, "Browser session expired");
}

#[test]
fn parse_connected_fetch_invocation_response_decodes_success_payload() {
    let body_text = json!({
        "ok": true,
        "status": 200,
        "result": {
            "operation": "fetch",
            "status": 200,
            "canvasProgramUrl": "https://gemini.google.com/app/browser-owned",
            "appPath": "/app/browser-owned",
            "conversationId": "c_browser_owned",
            "responseId": "r_browser_owned",
            "bodyText": "{\"ok\":true}"
        }
    })
    .to_string();

    let invocation = parse_connected_fetch_invocation_response(
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        200,
        &body_text,
    )
    .expect("connected fetch invocation");
    assert_eq!(invocation.status, 200);
    assert_eq!(invocation.body_text.as_deref(), Some("{\"ok\":true}"));
}

#[test]
fn parse_connected_fetch_invocation_response_rejects_missing_result() {
    let body_text = json!({
        "ok": true,
        "status": 200
    })
    .to_string();

    let error = parse_connected_fetch_invocation_response(
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        200,
        &body_text,
    )
    .expect_err("missing result");
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_connected_fetch_missing_result")
    );
}

#[test]
fn parse_connected_fetch_invocation_response_rejects_non_success_fetch_status() {
    let body_text = json!({
        "ok": true,
        "status": 200,
        "result": {
            "operation": "fetch",
            "status": 503,
            "bodyText": "{\"message\":\"temporary upstream failure\"}"
        }
    })
    .to_string();

    let error = parse_connected_fetch_invocation_response(
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        200,
        &body_text,
    )
    .expect_err("upstream fetch failure");
    assert_eq!(error.http_status, Some(503));
    assert_eq!(error.message, "temporary upstream failure");
}

#[test]
fn parse_connected_fetch_json_body_decodes_and_validates_json() {
    let body = parse_connected_fetch_json_body(
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        Some("{\"ok\":true,\"count\":2}"),
    )
    .expect("json body");
    assert_eq!(
        body.get("ok").and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        body.get("count").and_then(serde_json::Value::as_u64),
        Some(2)
    );
}

#[test]
fn parse_connected_fetch_json_body_rejects_missing_or_invalid_json() {
    let missing_error =
        parse_connected_fetch_json_body(GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER, None)
            .expect_err("missing body");
    assert_eq!(
        missing_error.code.as_deref(),
        Some("gemini_canvas_connected_fetch_missing_body")
    );

    let invalid_error = parse_connected_fetch_json_body(
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        Some("not-json"),
    )
    .expect_err("invalid json");
    assert_eq!(
        invalid_error.code.as_deref(),
        Some("gemini_canvas_connected_fetch_invalid_json")
    );
}

#[test]
fn parse_remote_browser_invocation_value_decodes_executor_payload() {
    let invocation = parse_remote_browser_invocation_value(
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        json!({
            "operation": "video",
            "canvasProgramUrl": "https://gemini.google.com/app/browser-owned",
            "appPath": "/app/browser-owned",
            "conversationId": "c_browser_owned",
            "responseId": "r_browser_owned",
            "media": [{
                "kind": "video",
                "url": "https://example.com/video.mp4",
                "mimeType": "video/mp4"
            }]
        }),
        "remote Gemini Canvas modular video result",
        "gemini_canvas_modular_remote_result_parse_failed",
    )
    .expect("remote browser invocation");
    assert_eq!(invocation.operation, "video");
    assert_eq!(invocation.media.len(), 1);
}

#[test]
fn parse_remote_browser_invocation_value_rejects_invalid_shape() {
    let error = parse_remote_browser_invocation_value(
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        json!({
            "canvasProgramUrl": "https://gemini.google.com/app/browser-owned"
        }),
        "remote Gemini Canvas modular video result",
        "gemini_canvas_modular_remote_result_parse_failed",
    )
    .expect_err("invalid remote payload");
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_modular_remote_result_parse_failed")
    );
}

#[test]
fn parse_remote_media_browser_invocation_value_rejects_invalid_shape() {
    let error = parse_remote_media_browser_invocation_value(
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        json!({
            "canvasProgramUrl": "https://gemini.google.com/app/browser-owned"
        }),
        "image",
    )
    .expect_err("invalid remote payload");
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_remote_result_parse_failed")
    );
    assert_eq!(
        error.message.as_str(),
        "Failed to parse remote Gemini Canvas image result: missing field `operation`"
    );
}

#[test]
fn parse_remote_modular_media_browser_invocation_value_rejects_invalid_shape() {
    let error = parse_remote_modular_media_browser_invocation_value(
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        json!({
            "canvasProgramUrl": "https://gemini.google.com/app/browser-owned"
        }),
        "video",
    )
    .expect_err("invalid remote payload");
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_modular_remote_result_parse_failed")
    );
    assert_eq!(
        error.message.as_str(),
        "Failed to parse remote Gemini Canvas modular video result: missing field `operation`"
    );
}

#[test]
fn parse_http_replay_worker_output_reads_result_contract() {
    let parsed = parse_http_replay_worker_output(
        "{\"ok\":true,\"status\":200,\"contentType\":\"application/json\",\"bodyText\":\"{\\\"ok\\\":true}\"}",
        "",
    )
    .expect("http replay worker output");

    assert!(parsed.ok);
    assert_eq!(parsed.status, Some(200));
    assert_eq!(parsed.content_type.as_deref(), Some("application/json"));
    assert_eq!(parsed.body_text.as_deref(), Some("{\"ok\":true}"));
}

#[test]
fn parse_http_replay_worker_output_rejects_empty_stdout_contract() {
    let error = parse_http_replay_worker_output("", "permission denied").expect_err("empty stdout");
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_http_replay_worker_empty_output")
    );
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
}

#[test]
fn extract_http_replay_worker_success_reads_status_and_body_contract() {
    let result = parse_http_replay_worker_output(
        "{\"ok\":true,\"status\":202,\"contentType\":\"application/json\",\"bodyText\":\"accepted\"}",
        "",
    )
    .expect("http replay worker output");

    let success = extract_http_replay_worker_success(result).expect("http replay worker success");
    assert_eq!(success.status, 202);
    assert_eq!(success.content_type.as_deref(), Some("application/json"));
    assert_eq!(success.body_text, "accepted");
}

#[test]
fn extract_http_replay_worker_success_defaults_missing_status_and_body_contract() {
    let result = parse_http_replay_worker_output("{\"ok\":true}", "").expect("worker output");

    let success = extract_http_replay_worker_success(result).expect("http replay worker success");
    assert_eq!(success.status, 200);
    assert_eq!(success.content_type, None);
    assert_eq!(success.body_text, "");
}

#[test]
fn classify_http_replay_worker_failure_prefers_worker_error_contract() {
    let result = parse_http_replay_worker_output(
        "{\"ok\":false,\"status\":500,\"error\":{\"status\":422,\"code\":\"gemini_canvas_worker_failed\",\"message\":\"challenge required\",\"bodyText\":\"{\\\"message\\\":\\\"Unauthorized\\\"}\"}}",
        "",
    )
    .expect("worker output");

    let error = classify_http_replay_worker_failure(result, "", "gemini_canvas_compatible");
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(error.http_status, Some(422));
    assert_eq!(error.code.as_deref(), Some("gemini_canvas_worker_failed"));
    assert_eq!(error.message, "challenge required");
}

#[test]
fn classify_http_replay_worker_failure_uses_stderr_when_body_missing() {
    let result = parse_http_replay_worker_output("{\"ok\":false,\"status\":429}", "")
        .expect("worker output");

    let error = classify_http_replay_worker_failure(
        result,
        "permission denied",
        "gemini_canvas_compatible",
    );
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(error.http_status, Some(429));
    assert!(error.message.contains("permission denied"));
}

#[test]
fn build_gemini_canvas_browser_executor_service_result_preserves_body_and_media_contract() {
    let result = parse_remote_browser_invocation_value(
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        json!({
            "operation": "image",
            "bodyText": "accepted",
            "media": [{
                "kind": "image",
                "url": "https://cdn.example.com/image.png",
                "mimeType": "image/png",
                "alt": "preview",
                "width": 512,
                "height": 512
            }]
        }),
        "service result",
        "gemini_canvas_service_result_parse_failed",
    )
    .expect("gemini canvas invocation result");

    let body = build_gemini_canvas_browser_executor_service_result(&result);
    assert_eq!(body["operation"], "image");
    assert_eq!(body["bodyText"], "accepted");
    assert_eq!(body["media"][0]["kind"], "image");
    assert_eq!(body["media"][0]["url"], "https://cdn.example.com/image.png");
    assert_eq!(body["media"][0]["mimeType"], "image/png");
    assert_eq!(body["media"][0]["alt"], "preview");
    assert_eq!(body["media"][0]["width"], 512);
    assert_eq!(body["media"][0]["height"], 512);
}

#[test]
fn build_gemini_canvas_browser_executor_service_result_preserves_null_body_text_contract() {
    let result = parse_remote_browser_invocation_value(
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        json!({
            "operation": "video",
            "bodyText": null,
            "media": []
        }),
        "service result",
        "gemini_canvas_service_result_parse_failed",
    )
    .expect("gemini canvas invocation result");

    let body = build_gemini_canvas_browser_executor_service_result(&result);
    assert_eq!(body["operation"], "video");
    assert!(body["bodyText"].is_null());
    assert_eq!(body["media"], json!([]));
}

#[test]
fn browser_pool_missing_image_asset_error_matches_contract() {
    let err = browser_pool_missing_image_asset_error(GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER);
    assert_eq!(err.http_status, Some(500));
    assert_eq!(
        err.provider_name.as_deref(),
        Some(GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER)
    );
    assert_eq!(err.code.as_deref(), Some("gemini_canvas_no_image_asset"));
    assert_eq!(
        err.message.as_str(),
        "Gemini Canvas browser pool completed without returning an image asset."
    );
}

#[test]
fn plan_direct_http_image_response_reports_standard_missing_asset_contract() {
    let req = make_media_request(EndpointKind::ImagesGenerations);
    let assets: Vec<gemini_canvas::GeminiCanvasMediaAsset> = Vec::new();

    match plan_direct_http_image_response(&req, "bright cube", &assets, "gemini_canvas_compatible")
    {
        Ok(_) => panic!("missing image assets should error"),
        Err(error) => {
            assert_eq!(error.http_status, Some(500));
            assert_eq!(
                error.provider_name.as_deref(),
                Some("gemini_canvas_compatible")
            );
            assert_eq!(error.code.as_deref(), Some("gemini_canvas_no_image_asset"));
            assert_eq!(
                error.message.as_str(),
                "Gemini Canvas direct HTTP image generation completed without returning an image asset."
            );
        }
    }
}

#[test]
fn browser_request_retry_delay_ms_matches_browser_owned_policy() {
    let auth_error = GatewayError::unauthorized("auth")
        .with_provider(GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER)
        .with_code("gemini_canvas_auth_required");
    assert_eq!(browser_request_retry_delay_ms(&auth_error, 0), Some(2_000));

    let busy_error = GatewayError::service_unavailable("busy")
        .with_provider(GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER)
        .with_code("gemini_canvas_context_busy");
    assert_eq!(browser_request_retry_delay_ms(&busy_error, 1), Some(4_500));

    let quota_error = GatewayError::rate_limited("quota", 1000)
        .with_provider(GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER)
        .with_code("gemini_canvas_video_quota_reached");
    assert_eq!(browser_request_retry_delay_ms(&quota_error, 0), None);

    let worker_error = GatewayError::service_unavailable("worker")
        .with_provider(GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER)
        .with_code("gemini_canvas_browser_worker_failed");
    assert_eq!(
        browser_request_retry_delay_ms(&worker_error, 0),
        Some(2_500)
    );

    let rate_limit_error = GatewayError::rate_limited("busy", 1000)
        .with_provider(GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER);
    assert_eq!(
        browser_request_retry_delay_ms(&rate_limit_error, 0),
        Some(2_500)
    );
    assert_eq!(browser_request_retry_delay_ms(&rate_limit_error, 2), None);
}

#[test]
fn require_text_result_errors_when_missing_text() {
    let result = GeminiCanvasBrowserOwnedInvocationResult {
        operation: "text".to_string(),
        share_url: None,
        share_id: None,
        share_follow_kind: None,
        before_url: None,
        final_url: None,
        page_url: None,
        canvas_program_url: None,
        app_path: None,
        conversation_id: None,
        response_id: None,
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
    let result: crate::upstream::gemini::canvas_program_web_reverse::GeminiCanvasBrowserInvocationResult =
        result.into();
    let error = require_text_result(
        &result,
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        "missing text",
        "missing_text",
    )
    .expect_err("missing text should error");
    assert_eq!(error.code.as_deref(), Some("missing_text"));
}
