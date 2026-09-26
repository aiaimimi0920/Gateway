use super::*;
use serde_json::{json, Value};
use std::collections::HashMap;

fn sample_browser_executor_error() -> BrowserExecutorInvocationError {
    BrowserExecutorInvocationError {
        code: Some("browser_executor_timeout".to_string()),
        message: Some("worker timed out".to_string()),
        status: Some(504),
        body: Some("gateway timeout".to_string()),
    }
}

fn sample_replay_query() -> Vec<(String, String)> {
    vec![
        ("rt".to_string(), "c".to_string()),
        ("hl".to_string(), "en".to_string()),
    ]
}

fn sample_suno_worker_result_json() -> Value {
    json!({
        "ok": true,
        "status": 200,
        "result": {
            "clips": [],
            "completed": true,
            "message": "ready",
        },
        "error": null,
    })
}

#[test]
fn browser_executor_invocation_request_serializes_camel_case_shape() {
    let request = BrowserExecutorInvocationRequest {
        provider: "udio",
        provider_account_id: "provider-account-1",
        endpoint_kind: "music_generations",
        execution_mode: "browser_owned",
        input: json!({
            "prompt": "compose a chorus",
            "waitAudio": true,
        }),
    };

    assert_eq!(
        serde_json::to_value(&request).expect("serialize browser executor invocation request"),
        json!({
            "provider": "udio",
            "providerAccountId": "provider-account-1",
            "endpointKind": "music_generations",
            "executionMode": "browser_owned",
            "input": {
                "prompt": "compose a chorus",
                "waitAudio": true,
            }
        })
    );
}

#[test]
fn browser_executor_invocation_response_deserializes_error_shape() {
    let decoded: BrowserExecutorInvocationResponse = serde_json::from_value(json!({
        "ok": false,
        "status": 502,
        "result": null,
        "error": {
            "code": "browser_executor_failed",
            "message": "executor unavailable",
            "status": 502,
            "body": "bad gateway",
        }
    }))
    .expect("deserialize browser executor invocation response");

    assert!(!decoded.ok);
    assert_eq!(decoded.status, Some(502));
    assert!(decoded.result.is_none());
    assert_eq!(
        decoded.error.as_ref().and_then(|item| item.code.as_deref()),
        Some("browser_executor_failed")
    );
    assert_eq!(
        decoded.error.as_ref().and_then(|item| item.body.as_deref()),
        Some("bad gateway")
    );
}

#[test]
fn browser_executor_service_invocation_request_deserializes_camel_case_shape() {
    let decoded: BrowserExecutorServiceInvocationRequest = serde_json::from_value(json!({
        "provider": "suno",
        "providerAccountId": "provider-account-2",
        "endpointKind": "music_generations",
        "executionMode": "browser_owned",
        "input": {
            "prompt": "make a hook",
        }
    }))
    .expect("deserialize browser executor service invocation request");

    assert_eq!(decoded.provider, "suno");
    assert_eq!(decoded.provider_account_id, "provider-account-2");
    assert_eq!(decoded.endpoint_kind, "music_generations");
    assert_eq!(decoded.execution_mode.as_deref(), Some("browser_owned"));
    assert_eq!(
        decoded.input.get("prompt").and_then(Value::as_str),
        Some("make a hook")
    );
}

#[test]
fn browser_executor_service_health_serializes_camel_case_shape() {
    let health = BrowserExecutorServiceHealth {
        ok: true,
        enabled: true,
        mode: "local".to_string(),
        remote_base_url: Some("http://127.0.0.1:4227".to_string()),
        lumalabs_script_path: "lumalabs-worker.mjs".to_string(),
        producer_script_path: "producer-worker.mjs".to_string(),
        suno_script_path: "suno-worker.mjs".to_string(),
        udio_script_path: "udio-worker.mjs".to_string(),
        gemini_canvas_pool_script_path: "gemini-canvas-pool.mjs".to_string(),
    };

    assert_eq!(
        serde_json::to_value(&health).expect("serialize browser executor service health"),
        json!({
            "ok": true,
            "enabled": true,
            "mode": "local",
            "remoteBaseUrl": "http://127.0.0.1:4227",
            "lumalabsScriptPath": "lumalabs-worker.mjs",
            "producerScriptPath": "producer-worker.mjs",
            "sunoScriptPath": "suno-worker.mjs",
            "udioScriptPath": "udio-worker.mjs",
            "geminiCanvasPoolScriptPath": "gemini-canvas-pool.mjs",
        })
    );
}

#[test]
fn browser_executor_service_invocation_response_serializes_error_shape() {
    let response = BrowserExecutorServiceInvocationResponse {
        ok: false,
        provider: "producer".to_string(),
        status: Some(504),
        result: None,
        error: Some(sample_browser_executor_error()),
        lease: None,
        browser_execution_status: "timeout".to_string(),
    };

    assert_eq!(
        serde_json::to_value(&response)
            .expect("serialize browser executor service invocation response"),
        json!({
            "ok": false,
            "provider": "producer",
            "status": 504,
            "result": null,
            "error": {
                "code": "browser_executor_timeout",
                "message": "worker timed out",
                "status": 504,
                "body": "gateway timeout",
            },
            "lease": null,
            "browserExecutionStatus": "timeout",
        })
    );
}

#[test]
fn gemini_canvas_http_replay_worker_input_serializes_expected_shape() {
    let query = sample_replay_query();
    let headers = HashMap::from([
        (
            "content-type".to_string(),
            "application/x-www-form-urlencoded".to_string(),
        ),
        ("x-test-header".to_string(), "value".to_string()),
    ]);
    let input = GeminiCanvasHttpReplayWorkerInput {
        url: "https://gemini.google.com/_/BardChatUi/data/streamgenerate",
        query: &query,
        headers: &headers,
        raw_post_data: "f.req=%5B%5D&at=token",
        cookie_header: "SID=test",
        operation: Some("image"),
        timeout_ms: 55_000,
    };

    assert_eq!(
        serde_json::to_value(&input).expect("serialize gemini canvas replay input"),
        json!({
            "url": "https://gemini.google.com/_/BardChatUi/data/streamgenerate",
            "query": [
                ["rt", "c"],
                ["hl", "en"],
            ],
            "headers": {
                "content-type": "application/x-www-form-urlencoded",
                "x-test-header": "value",
            },
            "rawPostData": "f.req=%5B%5D&at=token",
            "cookieHeader": "SID=test",
            "operation": "image",
            "timeoutMs": 55_000,
        })
    );
}

#[test]
fn gemini_canvas_http_replay_worker_result_deserializes_response_shape() {
    let decoded: GeminiCanvasHttpReplayWorkerResult = serde_json::from_value(json!({
        "ok": true,
        "status": 200,
        "finalUrl": "https://gemini.google.com/final",
        "contentType": "application/json",
        "bodyText": "{\"ok\":true}",
        "error": null,
    }))
    .expect("deserialize gemini canvas replay result");

    assert!(decoded.ok);
    assert_eq!(decoded.status, Some(200));
    assert_eq!(decoded.content_type.as_deref(), Some("application/json"));
    assert_eq!(decoded.body_text.as_deref(), Some("{\"ok\":true}"));
    assert!(decoded.error.is_none());
}

#[test]
fn suno_browser_worker_result_deserializes_completed_payload() {
    let decoded: SunoBrowserWorkerResult = serde_json::from_value(sample_suno_worker_result_json())
        .expect("deserialize suno worker result");

    assert!(decoded.ok);
    assert_eq!(decoded.status, Some(200));
    assert_eq!(
        decoded.result.as_ref().map(|item| item.completed),
        Some(true)
    );
    assert_eq!(
        decoded
            .result
            .as_ref()
            .and_then(|item| item.message.as_deref()),
        Some("ready")
    );
    assert!(decoded.error.is_none());
}

#[test]
fn lumalabs_browser_worker_result_deserializes_signed_url_and_error_shape() {
    let decoded: LumalabsBrowserWorkerResult = serde_json::from_value(json!({
        "ok": false,
        "signedUrl": null,
        "error": {
            "code": "lumalabs_browser_failed",
            "message": "challenge required",
            "status": 403,
            "body": "<html></html>",
        }
    }))
    .expect("deserialize lumalabs worker result");

    assert!(!decoded.ok);
    assert!(decoded.signed_url.is_none());
    assert_eq!(
        decoded.error.as_ref().and_then(|item| item.code.as_deref()),
        Some("lumalabs_browser_failed")
    );
    assert_eq!(
        decoded.error.as_ref().and_then(|item| item.status),
        Some(403)
    );
}

#[test]
fn producer_browser_worker_input_serializes_expected_camel_case_shape() {
    let body = json!({
        "prompt": "make a clip",
        "aspect_ratio": "16:9",
    });
    let input = ProducerBrowserWorkerInput {
        base_url: "https://producer.ai",
        auth_token: Some("producer-token".to_string()),
        cookie_header: Some("session=abc".to_string()),
        request_body: &body,
        model: "producer-video",
        accept_async_job: true,
        timeout_ms: 30_000,
        browser_executable_path: Some("C:/tools/chrome.exe".to_string()),
        user_agent: Some("producer-agent".to_string()),
        accept_language: Some("en-US,en;q=0.9".to_string()),
        origin: Some("https://producer.ai".to_string()),
        referer: Some("https://producer.ai/create".to_string()),
    };

    assert_eq!(
        serde_json::to_value(&input).expect("serialize producer worker input"),
        json!({
            "baseUrl": "https://producer.ai",
            "authToken": "producer-token",
            "cookieHeader": "session=abc",
            "requestBody": {
                "prompt": "make a clip",
                "aspect_ratio": "16:9",
            },
            "model": "producer-video",
            "acceptAsyncJob": true,
            "timeoutMs": 30_000,
            "browserExecutablePath": "C:/tools/chrome.exe",
            "userAgent": "producer-agent",
            "acceptLanguage": "en-US,en;q=0.9",
            "origin": "https://producer.ai",
            "referer": "https://producer.ai/create",
        })
    );
}

#[test]
fn udio_browser_worker_input_skips_empty_optional_fields() {
    let body = json!({
        "prompt": "build a chorus",
        "lyrics": "hello world",
    });
    let input = UdioBrowserWorkerInput {
        base_url: "https://www.udio.com",
        cookie_header: None,
        request_body: &body,
        target_asset_kind: "song",
        wait_audio: true,
        wait_timeout_ms: 120_000,
        poll_interval_ms: 2_000,
        timeout_ms: 45_000,
        runtime_state_object_key: Some(
            "credential-runtime/udio/manual-browser-helper/storage-state.json".to_string(),
        ),
        browser_executable_path: None,
        user_agent: Some("udio-agent".to_string()),
        accept_language: Some("en-US".to_string()),
        origin: Some("https://www.udio.com".to_string()),
        referer: Some("https://www.udio.com/create".to_string()),
    };

    assert_eq!(
        serde_json::to_value(&input).expect("serialize udio worker input"),
        json!({
            "baseUrl": "https://www.udio.com",
            "requestBody": {
                "prompt": "build a chorus",
                "lyrics": "hello world",
            },
            "targetAssetKind": "song",
            "waitAudio": true,
            "waitTimeoutMs": 120_000,
            "pollIntervalMs": 2_000,
            "timeoutMs": 45_000,
            "runtimeStateObjectKey": "credential-runtime/udio/manual-browser-helper/storage-state.json",
            "userAgent": "udio-agent",
            "acceptLanguage": "en-US",
            "origin": "https://www.udio.com",
            "referer": "https://www.udio.com/create",
        })
    );
}
