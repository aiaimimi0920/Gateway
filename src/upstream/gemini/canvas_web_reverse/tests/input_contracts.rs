use super::*;

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
