use base64::Engine;
use serde_json::json;

use super::*;
use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind, ProtocolFamily};
use crate::protocol::gemini::shared::GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER;
use crate::protocol::gemini_canvas;
use crate::routing::candidate::ProviderAccountPayload;

fn make_payload() -> ProviderAccountPayload {
    ProviderAccountPayload {
        adapter: GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER.to_string(),
        base_url: "https://gemini.google.com".to_string(),
        api_key: "unused".to_string(),
        credential_id: None,
        expires_at: None,
        runtime_state_object_key: Some(
            "credential-runtime/gemini-canvas/browser/storage-state.json".to_string(),
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
        extra_body: Some(std::collections::HashMap::from([(
            "shareId".to_string(),
            json!("canvas-share-789"),
        )])),
        session_auth: None,
        keepalive: None,
    }
}

#[test]
fn force_browser_owned_payload_sets_browser_owner_marker() {
    let payload = force_browser_owned_payload(&make_payload());
    let extra = payload.extra_body.expect("extra_body");
    assert_eq!(
        extra
            .get("canvasExecutionOwner")
            .and_then(serde_json::Value::as_str),
        Some("browser_owned_relay")
    );
}

#[test]
fn build_browser_operation_invocation_input_from_values_sets_browser_owned_fields() {
    let input = build_browser_operation_invocation_input_from_values(
        "https://gemini.google.com",
        "canvas-share-789",
        "credential-runtime/gemini-canvas/browser/storage-state.json",
        None,
        None,
        "image",
        "draw a cube",
        "en-US",
        std::time::Duration::from_secs(45),
    );

    assert_eq!(
        input.get("baseUrl").and_then(serde_json::Value::as_str),
        Some("https://gemini.google.com")
    );
    assert_eq!(
        input.get("shareId").and_then(serde_json::Value::as_str),
        Some("canvas-share-789")
    );
    assert_eq!(
        input
            .get("runtimeStateObjectKey")
            .and_then(serde_json::Value::as_str),
        Some("credential-runtime/gemini-canvas/browser/storage-state.json")
    );
    assert_eq!(
        input
            .get("requireAppPage")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
}

#[test]
fn build_browser_operation_invocation_input_prefers_browser_runtime_override() {
    let mut payload = make_payload();
    payload.extra_body = Some(std::collections::HashMap::from([
        ("shareId".to_string(), json!("canvas-share-789")),
        (
            "browserRuntimeStateObjectKey".to_string(),
            json!("credential-runtime/gemini-canvas/browser/profile"),
        ),
    ]));

    let input = build_browser_operation_invocation_input(
        &payload,
        "image",
        "draw a cube",
        "en-US",
        std::time::Duration::from_secs(30),
    )
    .expect("invocation input");

    assert_eq!(
        input
            .get("runtimeStateObjectKey")
            .and_then(serde_json::Value::as_str),
        Some("credential-runtime/gemini-canvas/browser/profile")
    );
}

#[test]
fn build_connected_fetch_invocation_input_sets_fetch_request_shape() {
    let input = build_connected_fetch_invocation_input(
        "https://gemini.google.com/",
        "canvas-share-789",
        "credential-runtime/gemini-canvas/browser/storage-state.json",
        None,
        None,
        "https://gemini.google.com/_/fetch",
        &json!({"hello": "world"}),
        "google_signed",
        std::time::Duration::from_secs(30),
    );

    assert_eq!(
        input
            .get("googleFetchMode")
            .and_then(serde_json::Value::as_str),
        Some("google_signed")
    );
    assert_eq!(
        input
            .get("fetchRequest")
            .and_then(serde_json::Value::as_object)
            .and_then(|value| value.get("referrer"))
            .and_then(serde_json::Value::as_str),
        Some("https://gemini.google.com/share/canvas-share-789")
    );
}

#[test]
fn build_connected_fetch_form_invocation_input_sets_fetch_request_shape() {
    let headers = std::collections::HashMap::from([
        (
            "Content-Type".to_string(),
            "application/x-www-form-urlencoded;charset=UTF-8".to_string(),
        ),
        ("X-Test".to_string(), "yes".to_string()),
    ]);
    let input = build_connected_fetch_form_invocation_input(
        "https://gemini.google.com/",
        "canvas-share-789",
        "credential-runtime/gemini-canvas/browser/storage-state.json",
        Some("http://127.0.0.1:9222"),
        Some("SID=abc"),
        "https://gemini.google.com/_/BardChatUi/data/batchexecute",
        &headers,
        "f.req=%5B%5D&at=token",
        "https://gemini.google.com/app",
        std::time::Duration::from_secs(45),
    );

    let fetch_request = input
        .get("fetchRequest")
        .and_then(serde_json::Value::as_object)
        .expect("fetchRequest");
    assert_eq!(
        fetch_request.get("url").and_then(serde_json::Value::as_str),
        Some("https://gemini.google.com/_/BardChatUi/data/batchexecute")
    );
    assert_eq!(
        fetch_request
            .get("method")
            .and_then(serde_json::Value::as_str),
        Some("POST")
    );
    assert_eq!(
        fetch_request
            .get("headers")
            .and_then(serde_json::Value::as_object)
            .and_then(|value| value.get("Content-Type"))
            .and_then(serde_json::Value::as_str),
        Some("application/x-www-form-urlencoded;charset=UTF-8")
    );
    assert_eq!(
        fetch_request
            .get("bodyText")
            .and_then(serde_json::Value::as_str),
        Some("f.req=%5B%5D&at=token")
    );
    assert_eq!(
        fetch_request
            .get("referrer")
            .and_then(serde_json::Value::as_str),
        Some("https://gemini.google.com/app")
    );
    assert_eq!(input["browserCdpUrl"], "http://127.0.0.1:9222");
    assert_eq!(input["cookieHeader"], "SID=abc");
    assert_eq!(input["timeoutMs"], 45_000u64);
}

#[test]
fn build_connected_fetch_form_invocation_input_disables_app_page_for_non_gemini_hosts() {
    let input = build_connected_fetch_form_invocation_input(
        "https://vertex.example.com",
        "canvas-share-789",
        "credential-runtime/gemini-canvas/browser/storage-state.json",
        None,
        None,
        "https://vertex.example.com/fetch",
        &std::collections::HashMap::new(),
        "",
        "https://vertex.example.com/",
        std::time::Duration::from_secs(10),
    );

    assert_eq!(
        input
            .get("requireAppPage")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
}

#[test]
fn build_http_replay_worker_input_preserves_replay_contract() {
    let query = vec![
        ("hl".to_string(), "zh-CN".to_string()),
        ("rt".to_string(), "c".to_string()),
    ];
    let headers = std::collections::HashMap::from([
        (
            "Content-Type".to_string(),
            "application/x-www-form-urlencoded;charset=UTF-8".to_string(),
        ),
        ("X-Test".to_string(), "yes".to_string()),
    ]);

    let input = build_http_replay_worker_input(
        "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate",
        &query,
        &headers,
        "f.req=%5B%5D&at=token",
        "SID=abc",
        Some("image"),
        std::time::Duration::from_secs(45),
    );
    let payload = serde_json::to_value(&input).expect("http replay worker input");

    assert_eq!(
        payload["url"],
        "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate"
    );
    assert_eq!(payload["query"][0][0], "hl");
    assert_eq!(payload["query"][0][1], "zh-CN");
    assert_eq!(
        payload["headers"]["Content-Type"],
        "application/x-www-form-urlencoded;charset=UTF-8"
    );
    assert_eq!(payload["rawPostData"], "f.req=%5B%5D&at=token");
    assert_eq!(payload["cookieHeader"], "SID=abc");
    assert_eq!(payload["operation"], "image");
    assert_eq!(payload["timeoutMs"], 45_000u64);
}

#[test]
fn prepare_gemini_canvas_browser_executor_service_input_preserves_runtime_contract() {
    let input = json!({
        "baseUrl": "https://gemini.google.com",
        "shareId": "canvas-share-789",
        "runtimeStateObjectKey": "credential-runtime/gemini-canvas/browser/storage-state.json",
        "browserCdpUrl": "http://127.0.0.1:9222",
        "cookieHeader": "SID=abc",
        "operation": "image",
        "prompt": "draw a cube",
        "locale": "en-US",
        "timeoutMs": 45_000u64
    });

    let prepared = prepare_gemini_canvas_browser_executor_service_input(&input)
        .expect("gemini canvas service input");

    assert_eq!(prepared.base_url, "https://gemini.google.com");
    assert_eq!(prepared.share_id, "canvas-share-789");
    assert_eq!(
        prepared.runtime_state_object_key,
        "credential-runtime/gemini-canvas/browser/storage-state.json"
    );
    assert_eq!(
        prepared.browser_cdp_url.as_deref(),
        Some("http://127.0.0.1:9222")
    );
    assert_eq!(prepared.cookie_header.as_deref(), Some("SID=abc"));
    assert_eq!(prepared.operation, "image");
    assert_eq!(prepared.prompt, "draw a cube");
    assert_eq!(prepared.locale, "en-US");
    assert_eq!(prepared.timeout, std::time::Duration::from_secs(45));
}

#[test]
fn prepare_gemini_canvas_browser_executor_service_input_requires_prompt_contract() {
    let input = json!({
        "baseUrl": "https://gemini.google.com",
        "shareId": "canvas-share-789",
        "runtimeStateObjectKey": "credential-runtime/gemini-canvas/browser/storage-state.json",
        "operation": "image"
    });

    let error = prepare_gemini_canvas_browser_executor_service_input(&input)
        .expect_err("missing prompt should fail");
    assert_eq!(
        error.code.as_deref(),
        Some("browser_executor_missing_prompt")
    );
    assert_eq!(error.http_status, Some(400));
}

#[test]
fn build_browser_operation_invocation_input_from_values_disables_app_page_for_non_gemini_hosts() {
    let input = build_browser_operation_invocation_input_from_values(
        "https://example.com",
        "canvas-share-789",
        "credential-runtime/gemini-canvas/browser/storage-state.json",
        None,
        None,
        "image",
        "draw a cube",
        "en-US",
        std::time::Duration::from_millis(1500),
    );

    assert_eq!(
        input
            .get("requireAppPage")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert_eq!(
        input.get("timeoutMs").and_then(serde_json::Value::as_u64),
        Some(1500)
    );
}

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

#[test]
fn decode_inline_audio_payload_uses_default_mime_and_decodes_bytes() {
    let result = GeminiCanvasBrowserOwnedInvocationResult {
        operation: "tts".to_string(),
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
        body_base64: Some(base64::engine::general_purpose::STANDARD.encode(b"abc")),
        mime_type: None,
        text: None,
        media: Vec::new(),
    };
    let result: crate::upstream::gemini::canvas_program_web_reverse::GeminiCanvasBrowserInvocationResult =
        result.into();
    let audio = decode_inline_audio_payload(
        &result,
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        "missing audio",
        "missing_audio",
        "invalid audio",
        "invalid_audio",
        "audio/wav",
    )
    .expect("audio payload");
    assert_eq!(audio.mime_type, "audio/wav");
    assert_eq!(audio.bytes, b"abc");
}

#[test]
fn decode_modular_tts_audio_payload_reports_standard_contracts() {
    let result = GeminiCanvasBrowserOwnedInvocationResult {
        operation: "tts".to_string(),
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
        body_base64: Some("not-base64".to_string()),
        mime_type: None,
        text: None,
        media: Vec::new(),
    };
    let result: crate::upstream::gemini::canvas_program_web_reverse::GeminiCanvasBrowserInvocationResult =
        result.into();

    let error =
        decode_modular_tts_audio_payload(&result, GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER)
            .expect_err("invalid base64 should fail");

    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_modular_invalid_tts_audio_payload")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas modular browser relay returned invalid inline audio bytes: Invalid symbol 45, offset 3."
    );
}

#[test]
fn decode_browser_tts_audio_payload_reports_standard_contracts() {
    let result = GeminiCanvasBrowserOwnedInvocationResult {
        operation: "tts".to_string(),
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
        body_base64: Some("not-base64".to_string()),
        mime_type: None,
        text: None,
        media: Vec::new(),
    };
    let result: crate::upstream::gemini::canvas_program_web_reverse::GeminiCanvasBrowserInvocationResult =
        result.into();

    let error =
        decode_browser_tts_audio_payload(&result, GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER)
            .expect_err("invalid base64 should fail");

    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_invalid_tts_audio_payload")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas browser-backed TTS returned invalid base64 audio bytes: Invalid symbol 45, offset 3."
    );
}

fn make_media_result(
    media: Vec<GeminiCanvasBrowserOwnedMediaAsset>,
) -> crate::upstream::gemini::canvas_program_web_reverse::GeminiCanvasBrowserInvocationResult {
    GeminiCanvasBrowserOwnedInvocationResult {
        operation: "image".to_string(),
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
        media,
    }
    .into()
}

fn make_media_asset(kind: &str, url: &str, mime_type: &str) -> GeminiCanvasBrowserOwnedMediaAsset {
    GeminiCanvasBrowserOwnedMediaAsset {
        kind: kind.to_string(),
        url: url.to_string(),
        mime_type: mime_type.to_string(),
        body_base64: None,
        alt: None,
        width: None,
        height: None,
        duration_seconds: None,
    }
}

fn make_converted_image_asset(url: &str, mime_type: &str) -> gemini_canvas::GeminiCanvasMediaAsset {
    gemini_canvas::GeminiCanvasMediaAsset {
        kind: "image".to_string(),
        url: url.to_string(),
        mime_type: mime_type.to_string(),
        download_token: None,
        body_base64: None,
        alt: None,
        width: None,
        height: None,
        duration_seconds: None,
    }
}

fn make_media_request(endpoint_kind: EndpointKind) -> CanonicalRelayRequest {
    CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind,
        requested_model: None,
        stream: false,
        messages: Vec::new(),
        tools: Vec::new(),
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({}),
        previous_response_id: None,
        explicit_session_key: None,
        extra: std::collections::HashMap::new(),
    }
}

#[test]
fn collect_image_media_assets_filters_only_image_entries() {
    let result = make_media_result(vec![
        make_media_asset("image", "https://example.com/image-1.png", "image/png"),
        make_media_asset("video", "https://example.com/video.mp4", "video/mp4"),
        make_media_asset("image", "https://example.com/image-2.png", "image/png"),
    ]);

    let assets = collect_image_media_assets(&result);
    assert_eq!(assets.len(), 2);
    assert!(assets.iter().all(|asset| asset.kind == "image"));
}

#[test]
fn collect_converted_image_media_assets_keeps_inline_payload() {
    let mut inline = make_media_asset("image", "https://example.com/image.png", "image/png");
    inline.body_base64 = Some(base64::engine::general_purpose::STANDARD.encode(b"png"));
    let result = make_media_result(vec![inline]);
    let assets = collect_converted_image_media_assets(&result);
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].mime_type, "image/png");
    assert_eq!(assets[0].body_base64.as_deref(), Some("cG5n"));
}

#[test]
fn collect_converted_image_media_assets_preserves_metadata_fields() {
    let mut asset = make_media_asset("image", "https://example.com/image.png", "image/png");
    asset.alt = Some("preview".to_string());
    asset.width = Some(640);
    asset.height = Some(480);

    let result = make_media_result(vec![asset]);
    let assets = collect_converted_image_media_assets(&result);
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].url, "https://example.com/image.png");
    assert_eq!(assets[0].alt.as_deref(), Some("preview"));
    assert_eq!(assets[0].width, Some(640));
    assert_eq!(assets[0].height, Some(480));
}

#[test]
fn decode_inline_image_asset_decodes_bytes_and_preserves_mime() {
    let mut asset = make_converted_image_asset("https://example.com/image.png", "image/png");
    asset.body_base64 = Some(base64::engine::general_purpose::STANDARD.encode(b"png"));

    let image = decode_inline_image_asset(
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        &asset,
        "invalid_inline",
        "inline image",
    )
    .expect("decode should succeed")
    .expect("inline image should exist");

    assert_eq!(image.mime_type, "image/png");
    assert_eq!(image.bytes, b"png");
}

#[test]
fn decode_inline_image_asset_returns_none_without_inline_payload() {
    let asset = make_converted_image_asset("https://example.com/image.png", "image/png");

    let image = decode_inline_image_asset(
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        &asset,
        "invalid_inline",
        "inline image",
    )
    .expect("decode should succeed");

    assert!(image.is_none());
}

#[test]
fn decode_inline_image_asset_reports_configured_error_code() {
    let mut asset = make_converted_image_asset("https://example.com/image.png", "image/png");
    asset.body_base64 = Some("not-base64".to_string());

    let error = decode_inline_image_asset(
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        &asset,
        "invalid_inline",
        "inline image",
    )
    .expect_err("invalid base64 should fail");

    assert_eq!(error.code.as_deref(), Some("invalid_inline"));
    assert!(error.message.contains("invalid inline image bytes"));
}

#[test]
fn decode_browser_pool_inline_image_asset_reports_standard_contract() {
    let mut asset = make_converted_image_asset("https://example.com/image.png", "image/png");
    asset.body_base64 = Some("not-base64".to_string());

    let error =
        decode_browser_pool_inline_image_asset(GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER, &asset)
            .expect_err("invalid base64 should fail");

    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_invalid_inline_image_bytes")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas browser pool returned invalid inline image bytes: Invalid symbol 45, offset 3."
    );
}

#[test]
fn decode_direct_http_inline_image_asset_reports_standard_contract() {
    let mut asset = make_converted_image_asset("https://example.com/image.png", "image/png");
    asset.body_base64 = Some("not-base64".to_string());

    let error = decode_direct_http_inline_image_asset("gemini_canvas_compatible", &asset)
        .expect_err("invalid base64 should fail");

    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_invalid_inline_image_bytes")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas direct HTTP image asset contained invalid base64 bytes returned invalid inline image bytes: Invalid symbol 45, offset 3."
    );
}

#[tokio::test]
async fn build_image_generation_response_from_invocation_uses_url_mode_and_requested_count() {
    let result = make_media_result(vec![
        make_media_asset("image", "https://example.com/image-1.png", "image/png"),
        make_media_asset("image", "https://example.com/image-2.png", "image/png"),
    ]);
    let mut req = make_media_request(EndpointKind::ImagesGenerations);
    req.raw_body = json!({
        "response_format": "url",
        "n": 1
    });

    let response = build_image_generation_response_from_invocation(
        &rquest::Client::new(),
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        &req,
        "bright cube",
        &result,
        std::time::Duration::from_secs(5),
    )
    .await
    .expect("url image response");

    let data = response["data"].as_array().expect("data array");
    assert_eq!(data.len(), 1);
    assert_eq!(data[0]["url"], "https://example.com/image-1.png");
    assert_eq!(data[0]["mime_type"], "image/png");
    assert_eq!(data[0]["revised_prompt"], "bright cube");
}

#[tokio::test]
async fn build_image_generation_response_from_invocation_uses_inline_b64_mode() {
    let mut inline = make_media_asset("image", "https://example.com/image-1.png", "image/png");
    inline.body_base64 = Some(base64::engine::general_purpose::STANDARD.encode(b"png-1"));
    let mut second = make_media_asset("image", "https://example.com/image-2.png", "image/png");
    second.body_base64 = Some(base64::engine::general_purpose::STANDARD.encode(b"png-2"));
    let result = make_media_result(vec![inline, second]);
    let mut req = make_media_request(EndpointKind::ImagesGenerations);
    req.raw_body = json!({
        "response_format": "b64_json",
        "n": 1
    });

    let response = build_image_generation_response_from_invocation(
        &rquest::Client::new(),
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        &req,
        "bright cube",
        &result,
        std::time::Duration::from_secs(5),
    )
    .await
    .expect("inline image response");

    let data = response["data"].as_array().expect("data array");
    assert_eq!(data.len(), 1);
    assert_eq!(
        data[0]["b64_json"],
        base64::engine::general_purpose::STANDARD.encode(b"png-1")
    );
    assert_eq!(data[0]["mime_type"], "image/png");
}

#[tokio::test]
async fn build_image_generation_response_from_invocation_errors_without_image_asset() {
    let result = make_media_result(vec![make_media_asset(
        "video",
        "https://example.com/video.mp4",
        "video/mp4",
    )]);
    let req = make_media_request(EndpointKind::ImagesGenerations);

    let error = build_image_generation_response_from_invocation(
        &rquest::Client::new(),
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        &req,
        "bright cube",
        &result,
        std::time::Duration::from_secs(5),
    )
    .await
    .expect_err("missing image asset should fail");

    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_modular_no_image_asset")
    );
}

#[tokio::test]
async fn build_image_generation_response_from_invocation_reports_invalid_inline_bytes() {
    let mut inline = make_media_asset("image", "https://example.com/image-1.png", "image/png");
    inline.body_base64 = Some("not-base64".to_string());
    let result = make_media_result(vec![inline]);
    let mut req = make_media_request(EndpointKind::ImagesGenerations);
    req.raw_body = json!({
        "response_format": "b64_json"
    });

    let error = build_image_generation_response_from_invocation(
        &rquest::Client::new(),
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        &req,
        "bright cube",
        &result,
        std::time::Duration::from_secs(5),
    )
    .await
    .expect_err("invalid inline bytes should fail");

    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_modular_invalid_inline_image_bytes")
    );
}

#[tokio::test]
async fn build_image_generation_response_from_invocation_validates_response_format() {
    let mut inline = make_media_asset("image", "https://example.com/image-1.png", "image/png");
    inline.body_base64 = Some(base64::engine::general_purpose::STANDARD.encode(b"png-1"));
    let result = make_media_result(vec![inline]);

    let mut invalid_req = make_media_request(EndpointKind::ImagesGenerations);
    invalid_req.raw_body = json!({
        "response_format": 123
    });
    let invalid_error = build_image_generation_response_from_invocation(
        &rquest::Client::new(),
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        &invalid_req,
        "bright cube",
        &result,
        std::time::Duration::from_secs(5),
    )
    .await
    .expect_err("non-string response_format should fail");
    assert_eq!(
        invalid_error.code.as_deref(),
        Some("invalid_image_response_format")
    );

    let mut unsupported_req = make_media_request(EndpointKind::ImagesGenerations);
    unsupported_req.raw_body = json!({
        "response_format": "stream"
    });
    let unsupported_error = build_image_generation_response_from_invocation(
        &rquest::Client::new(),
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        &unsupported_req,
        "bright cube",
        &result,
        std::time::Duration::from_secs(5),
    )
    .await
    .expect_err("unsupported response_format should fail");
    assert_eq!(
        unsupported_error.code.as_deref(),
        Some("unsupported_image_response_format")
    );
}

#[test]
fn select_downloaded_image_mime_type_prefers_image_content_type() {
    assert_eq!(
        select_downloaded_image_mime_type("image/png", Some(" image/webp ")),
        "image/webp"
    );
}

#[test]
fn select_downloaded_image_mime_type_falls_back_for_missing_or_non_image_header() {
    assert_eq!(
        select_downloaded_image_mime_type("image/png", Some("application/octet-stream")),
        "image/png"
    );
    assert_eq!(
        select_downloaded_image_mime_type("image/png", None),
        "image/png"
    );
}

#[test]
fn build_downloaded_image_from_bytes_uses_selected_mime_and_body() {
    let asset = make_converted_image_asset("https://example.com/image.png", "image/png");
    let image = build_downloaded_image_from_bytes(&asset, Some("image/jpeg"), b"jpeg-bytes");
    assert_eq!(image.mime_type, "image/jpeg");
    assert_eq!(image.bytes, b"jpeg-bytes");
}

#[test]
fn require_music_media_asset_accepts_video_or_audio_assets() {
    let result = make_media_result(vec![
        make_media_asset("image", "https://example.com/image.png", "image/png"),
        make_media_asset("audio", "https://example.com/audio.wav", "audio/wav"),
    ]);

    let asset = require_music_media_asset(&result, GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER)
        .expect("music asset");
    assert_eq!(asset.kind, "audio");
}

#[test]
fn require_music_media_asset_reports_modular_missing_asset_code() {
    let result = make_media_result(vec![make_media_asset(
        "image",
        "https://example.com/image.png",
        "image/png",
    )]);

    let error = require_music_media_asset(&result, GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER)
        .expect_err("missing music asset should error");
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_modular_no_music_asset")
    );
}

#[test]
fn require_video_media_asset_only_accepts_video_entries() {
    let result = make_media_result(vec![
        make_media_asset("audio", "https://example.com/audio.wav", "audio/wav"),
        make_media_asset("video", "https://example.com/video.mp4", "video/mp4"),
    ]);

    let asset = require_video_media_asset(&result, GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER)
        .expect("video asset");
    assert_eq!(asset.kind, "video");
}

#[test]
fn require_video_media_asset_reports_modular_missing_asset_code() {
    let result = make_media_result(vec![make_media_asset(
        "audio",
        "https://example.com/audio.wav",
        "audio/wav",
    )]);

    let error = require_video_media_asset(&result, GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER)
        .expect_err("missing video asset should error");
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_modular_no_video_asset")
    );
}

#[test]
fn build_music_generation_response_from_invocation_uses_selected_asset() {
    let result = make_media_result(vec![make_media_asset(
        "audio",
        "https://example.com/audio.wav",
        "audio/wav",
    )]);
    let response = build_music_generation_response_from_invocation(
        "gemini-2.5-music",
        "bright synth pulse",
        &result,
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
    )
    .expect("music response");
    assert_eq!(response["object"], "music.generation");
    assert_eq!(response["data"][0]["mime_type"], "audio/wav");
}

#[test]
fn build_music_generation_response_from_invocation_marks_pending_when_only_video_and_busy() {
    let mut result = make_media_result(vec![make_media_asset(
        "video",
        "https://example.com/music-preview.mp4",
        "video/mp4",
    )]);
    result.app_path = Some("/app/canvas-music".to_string());
    result.conversation_id = Some("c_canvas_music".to_string());
    result.response_id = Some("r_canvas_music".to_string());
    result.body_text = Some("I've hit a bit of a snag".to_string());

    let response = build_music_generation_response_from_invocation(
        "gemini-2.5-music",
        "bright synth pulse",
        &result,
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
    )
    .expect("accepted pending music response");

    assert_eq!(response["object"], "music.generation");
    assert_eq!(response["accepted"], true);
    assert_eq!(response["completed"], false);
    assert_eq!(response["conversation_id"], "c_canvas_music");
    assert_eq!(response["response_id"], "r_canvas_music");
}

#[test]
fn build_music_generation_response_from_invocation_marks_pending_for_generation_tokens() {
    let mut result = make_media_result(vec![make_media_asset(
        "video",
        "https://example.com/music-preview.mp4",
        "video/mp4",
    )]);
    result.app_path = Some("/app/canvas-music".to_string());
    result.conversation_id = Some("c_canvas_music".to_string());
    result.response_id = Some("r_canvas_music".to_string());
    result.body_text = Some(
        "[null,[\"c_canvas_music\",\"r_canvas_music\"],{\"11\":[\"Electronic Music Cue Generation\"],\"26\":\"AwAAAAAAAAAQwBHO-LzoF6Ltg6rx4Bk\",\"44\":true}]"
            .to_string(),
    );

    let response = build_music_generation_response_from_invocation(
        "gemini-2.5-music",
        "bright synth pulse",
        &result,
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
    )
    .expect("accepted pending music token response");

    assert_eq!(response["accepted"], true);
    assert_eq!(response["completed"], false);
    assert_eq!(response["conversation_id"], "c_canvas_music");
    assert_eq!(response["response_id"], "r_canvas_music");
}

#[test]
fn build_video_generation_response_from_invocation_uses_selected_asset() {
    let mut result = make_media_result(vec![make_media_asset(
        "video",
        "https://example.com/video.mp4",
        "video/mp4",
    )]);
    result.body_text = Some("video ready".to_string());
    let response = build_video_generation_response_from_invocation(
        "gemini-2.5-video",
        "rotating cube",
        &result,
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
    )
    .expect("video response");
    assert_eq!(response["object"], "video.generation");
    assert_eq!(response["data"][0]["mime_type"], "video/mp4");
}

#[test]
fn build_video_generation_response_from_invocation_marks_pending_when_only_locator_frames_exist() {
    let mut result = make_media_result(vec![]);
    result.body_text = Some(
        "[\"wrb.fr\",null,\"[null,[null,\\\"r_canvas_video\\\"],{\\\"18\\\":\\\"r_canvas_video\\\",\\\"21\\\":[\\\"123e4567-e89b-12d3-a456-426614174000\\\"],\\\"44\\\":true}]\"]".to_string(),
    );
    result.conversation_id = Some("c_canvas_video".to_string());
    result.response_id = Some("r_canvas_video".to_string());
    result.app_path = Some("/app/canvas-video".to_string());

    let response = build_video_generation_response_from_invocation(
        "gemini-2.5-video",
        "rotating cube",
        &result,
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
    )
    .expect("video pending response");
    assert_eq!(response["object"], "video.generation");
    assert_eq!(response["accepted"], true);
    assert_eq!(response["completed"], false);
    assert_eq!(response["conversation_id"], "c_canvas_video");
    assert_eq!(response["response_id"], "r_canvas_video");
}
