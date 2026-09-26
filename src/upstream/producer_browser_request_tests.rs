use super::*;

#[test]
fn build_producer_browser_executor_payload_preserves_header_and_timeout_contract() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Bearer token-123".parse().unwrap());
    headers.insert("cookie", "a=b".parse().unwrap());
    headers.insert("user-agent", "producer-agent".parse().unwrap());
    headers.insert("accept-language", "en-US".parse().unwrap());
    headers.insert("origin", "https://www.flowmusic.app".parse().unwrap());
    headers.insert(
        "referer",
        "https://www.flowmusic.app/create".parse().unwrap(),
    );

    let payload = build_producer_browser_executor_payload(
        "https://www.flowmusic.app",
        &headers,
        &json!({ "prompt": "cover art" }),
        "producer:image",
        std::time::Duration::from_secs(900),
        Some("C:/browser/chrome.exe".to_string()),
    );

    assert_eq!(payload["baseUrl"], "https://www.flowmusic.app");
    assert_eq!(payload["authToken"], "token-123");
    assert_eq!(payload["cookieHeader"], "a=b");
    assert_eq!(payload["requestBody"]["prompt"], "cover art");
    assert_eq!(payload["model"], "producer:image");
    assert_eq!(payload["timeoutMs"], 900_000u64);
    assert_eq!(payload["browserExecutablePath"], "C:/browser/chrome.exe");
    assert_eq!(payload["userAgent"], "producer-agent");
    assert_eq!(payload["acceptLanguage"], "en-US");
    assert_eq!(payload["origin"], "https://www.flowmusic.app");
    assert_eq!(payload["referer"], "https://www.flowmusic.app/create");
}

#[test]
fn build_producer_browser_executor_payload_from_prepared_preserves_header_and_timeout_contract() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Bearer token-123".parse().unwrap());
    headers.insert("cookie", "a=b".parse().unwrap());
    headers.insert("user-agent", "producer-agent".parse().unwrap());
    headers.insert("accept-language", "en-US".parse().unwrap());
    headers.insert("origin", "https://www.flowmusic.app".parse().unwrap());
    headers.insert(
        "referer",
        "https://www.flowmusic.app/create".parse().unwrap(),
    );
    let request_body = json!({ "prompt": "cover art" });
    let prepared = prepare_producer_browser_execution_input(
        "https://www.flowmusic.app/",
        &headers,
        &request_body,
        "producer:image",
        std::time::Duration::from_secs(900),
    );

    let payload = build_producer_browser_executor_payload_from_prepared(
        &prepared,
        Some("C:/browser/chrome.exe".to_string()),
    );

    assert_eq!(payload["baseUrl"], "https://www.flowmusic.app");
    assert_eq!(payload["authToken"], "token-123");
    assert_eq!(payload["cookieHeader"], "a=b");
    assert_eq!(payload["requestBody"]["prompt"], "cover art");
    assert_eq!(payload["model"], "producer:image");
    assert_eq!(payload["timeoutMs"], 900_000u64);
    assert_eq!(payload["browserExecutablePath"], "C:/browser/chrome.exe");
    assert_eq!(payload["userAgent"], "producer-agent");
    assert_eq!(payload["acceptLanguage"], "en-US");
    assert_eq!(payload["origin"], "https://www.flowmusic.app");
    assert_eq!(payload["referer"], "https://www.flowmusic.app/create");
}

#[test]
fn build_producer_browser_worker_input_preserves_header_and_async_contract() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Bearer token-123".parse().unwrap());
    headers.insert("cookie", "a=b".parse().unwrap());
    headers.insert("user-agent", "producer-agent".parse().unwrap());
    headers.insert("accept-language", "en-US".parse().unwrap());
    headers.insert("origin", "https://www.flowmusic.app".parse().unwrap());
    headers.insert(
        "referer",
        "https://www.flowmusic.app/create".parse().unwrap(),
    );
    let request_body = json!({ "prompt": "cover art" });

    let input = build_producer_browser_worker_input(
        "https://www.flowmusic.app",
        &headers,
        &request_body,
        "producer:image",
        true,
        std::time::Duration::from_secs(900),
        Some("C:/browser/chrome.exe".to_string()),
    );
    let payload = serde_json::to_value(&input).expect("producer worker input");

    assert_eq!(payload["baseUrl"], "https://www.flowmusic.app");
    assert_eq!(payload["authToken"], "token-123");
    assert_eq!(payload["cookieHeader"], "a=b");
    assert_eq!(payload["requestBody"]["prompt"], "cover art");
    assert_eq!(payload["model"], "producer:image");
    assert_eq!(payload["acceptAsyncJob"], true);
    assert_eq!(payload["timeoutMs"], 900_000u64);
    assert_eq!(payload["browserExecutablePath"], "C:/browser/chrome.exe");
    assert_eq!(payload["userAgent"], "producer-agent");
    assert_eq!(payload["acceptLanguage"], "en-US");
    assert_eq!(payload["origin"], "https://www.flowmusic.app");
    assert_eq!(payload["referer"], "https://www.flowmusic.app/create");
}

#[test]
fn prepare_producer_browser_execution_input_preserves_header_and_timeout_contract() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Bearer token-123".parse().unwrap());
    headers.insert("cookie", "a=b".parse().unwrap());
    headers.insert("user-agent", "producer-agent".parse().unwrap());
    headers.insert("accept-language", "en-US".parse().unwrap());
    headers.insert("origin", "https://www.flowmusic.app".parse().unwrap());
    headers.insert(
        "referer",
        "https://www.flowmusic.app/create".parse().unwrap(),
    );
    let request_body = json!({ "prompt": "cover art" });

    let prepared = prepare_producer_browser_execution_input(
        "https://www.flowmusic.app/",
        &headers,
        &request_body,
        "producer:image",
        std::time::Duration::from_secs(900),
    );

    assert_eq!(prepared.base_url, "https://www.flowmusic.app");
    assert_eq!(prepared.headers["authorization"], "Bearer token-123");
    assert_eq!(prepared.headers["cookie"], "a=b");
    assert_eq!(prepared.request_body["prompt"], "cover art");
    assert_eq!(prepared.model, "producer:image");
    assert_eq!(prepared.timeout, std::time::Duration::from_secs(900));
}

#[test]
fn serialize_producer_browser_worker_input_preserves_header_and_async_contract() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Bearer token-123".parse().unwrap());
    headers.insert("cookie", "a=b".parse().unwrap());
    headers.insert("user-agent", "producer-agent".parse().unwrap());
    headers.insert("accept-language", "en-US".parse().unwrap());
    headers.insert("origin", "https://www.flowmusic.app".parse().unwrap());
    headers.insert(
        "referer",
        "https://www.flowmusic.app/create".parse().unwrap(),
    );
    let request_body = json!({ "prompt": "cover art" });

    let stdin_json = serialize_producer_browser_worker_input(
        "https://www.flowmusic.app",
        &headers,
        &request_body,
        "producer:image",
        true,
        std::time::Duration::from_secs(900),
        Some("C:/browser/chrome.exe".to_string()),
    )
    .expect("producer worker stdin");
    let payload: serde_json::Value =
        serde_json::from_slice(&stdin_json).expect("stdin json should parse");

    assert_eq!(payload["baseUrl"], "https://www.flowmusic.app");
    assert_eq!(payload["authToken"], "token-123");
    assert_eq!(payload["cookieHeader"], "a=b");
    assert_eq!(payload["requestBody"]["prompt"], "cover art");
    assert_eq!(payload["model"], "producer:image");
    assert_eq!(payload["acceptAsyncJob"], true);
    assert_eq!(payload["timeoutMs"], 900_000u64);
    assert_eq!(payload["browserExecutablePath"], "C:/browser/chrome.exe");
    assert_eq!(payload["userAgent"], "producer-agent");
    assert_eq!(payload["acceptLanguage"], "en-US");
    assert_eq!(payload["origin"], "https://www.flowmusic.app");
    assert_eq!(payload["referer"], "https://www.flowmusic.app/create");
}

#[test]
fn serialize_producer_browser_worker_input_from_prepared_preserves_header_and_async_contract() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Bearer token-123".parse().unwrap());
    headers.insert("cookie", "a=b".parse().unwrap());
    headers.insert("user-agent", "producer-agent".parse().unwrap());
    headers.insert("accept-language", "en-US".parse().unwrap());
    headers.insert("origin", "https://www.flowmusic.app".parse().unwrap());
    headers.insert(
        "referer",
        "https://www.flowmusic.app/create".parse().unwrap(),
    );
    let request_body = json!({ "prompt": "cover art" });
    let prepared = prepare_producer_browser_execution_input(
        "https://www.flowmusic.app/",
        &headers,
        &request_body,
        "producer:image",
        std::time::Duration::from_secs(900),
    );

    let stdin_json = serialize_producer_browser_worker_input_from_prepared(
        &prepared,
        true,
        Some("C:/browser/chrome.exe".to_string()),
    )
    .expect("producer prepared worker stdin");
    let payload: serde_json::Value =
        serde_json::from_slice(&stdin_json).expect("stdin json should parse");

    assert_eq!(payload["baseUrl"], "https://www.flowmusic.app");
    assert_eq!(payload["authToken"], "token-123");
    assert_eq!(payload["cookieHeader"], "a=b");
    assert_eq!(payload["requestBody"]["prompt"], "cover art");
    assert_eq!(payload["model"], "producer:image");
    assert_eq!(payload["acceptAsyncJob"], true);
    assert_eq!(payload["timeoutMs"], 900_000u64);
    assert_eq!(payload["browserExecutablePath"], "C:/browser/chrome.exe");
    assert_eq!(payload["userAgent"], "producer-agent");
    assert_eq!(payload["acceptLanguage"], "en-US");
    assert_eq!(payload["origin"], "https://www.flowmusic.app");
    assert_eq!(payload["referer"], "https://www.flowmusic.app/create");
}

#[test]
fn prepare_producer_browser_worker_launch_from_prepared_preserves_header_and_async_contract() {
    let mut headers = HeaderMap::new();
    headers.insert("authorization", "Bearer token-123".parse().unwrap());
    headers.insert("cookie", "a=b".parse().unwrap());
    headers.insert("user-agent", "producer-agent".parse().unwrap());
    headers.insert("accept-language", "en-US".parse().unwrap());
    headers.insert("origin", "https://www.flowmusic.app".parse().unwrap());
    headers.insert(
        "referer",
        "https://www.flowmusic.app/create".parse().unwrap(),
    );
    let request_body = json!({ "prompt": "cover art" });
    let prepared = prepare_producer_browser_execution_input(
        "https://www.flowmusic.app/",
        &headers,
        &request_body,
        "producer:image",
        std::time::Duration::from_secs(900),
    );

    let launch = prepare_producer_browser_worker_launch_from_prepared(
        &prepared,
        true,
        Some("C:/browser/chrome.exe".to_string()),
        Some("node-custom".to_string()),
    )
    .expect("producer prepared worker launch");
    let payload: serde_json::Value =
        serde_json::from_slice(&launch.stdin_json).expect("stdin json should parse");

    assert_eq!(launch.node_bin, "node-custom");
    assert_eq!(
        launch
            .script_path
            .file_name()
            .and_then(|value| value.to_str()),
        Some("producer-browser-worker.mjs")
    );
    assert_eq!(payload["baseUrl"], "https://www.flowmusic.app");
    assert_eq!(payload["authToken"], "token-123");
    assert_eq!(payload["cookieHeader"], "a=b");
    assert_eq!(payload["requestBody"]["prompt"], "cover art");
    assert_eq!(payload["model"], "producer:image");
    assert_eq!(payload["acceptAsyncJob"], true);
    assert_eq!(payload["timeoutMs"], 900_000u64);
    assert_eq!(payload["browserExecutablePath"], "C:/browser/chrome.exe");
    assert_eq!(payload["userAgent"], "producer-agent");
    assert_eq!(payload["acceptLanguage"], "en-US");
    assert_eq!(payload["origin"], "https://www.flowmusic.app");
    assert_eq!(payload["referer"], "https://www.flowmusic.app/create");
}

#[test]
fn prepare_producer_browser_executor_service_input_preserves_header_and_timeout_contract() {
    let input = json!({
        "baseUrl": "https://www.flowmusic.app",
        "authToken": "token-123",
        "cookieHeader": "a=b",
        "requestBody": { "prompt": "cover art" },
        "model": "producer:image",
        "timeoutMs": 900_000u64,
        "userAgent": "producer-agent",
        "acceptLanguage": "en-US",
        "origin": "https://www.flowmusic.app",
        "referer": "https://www.flowmusic.app/create"
    });

    let prepared =
        prepare_producer_browser_executor_service_input(&input).expect("producer service input");

    assert_eq!(prepared.base_url, "https://www.flowmusic.app");
    assert_eq!(prepared.request_body["prompt"], "cover art");
    assert_eq!(prepared.model, "producer:image");
    assert_eq!(prepared.timeout, std::time::Duration::from_secs(900));
    assert_eq!(
        crate::upstream::header_map_helpers::header_map_string(&prepared.headers, "authorization")
            .as_deref(),
        Some("Bearer token-123")
    );
    assert_eq!(
        crate::upstream::header_map_helpers::header_map_string(&prepared.headers, "cookie")
            .as_deref(),
        Some("a=b")
    );
    assert_eq!(
        crate::upstream::header_map_helpers::header_map_string(&prepared.headers, "user-agent")
            .as_deref(),
        Some("producer-agent")
    );
}

#[test]
fn prepare_producer_browser_executor_service_input_requires_model_contract() {
    let input = json!({
        "baseUrl": "https://www.flowmusic.app",
        "requestBody": { "prompt": "cover art" }
    });

    let error = prepare_producer_browser_executor_service_input(&input)
        .expect_err("missing model should fail");
    assert_eq!(
        error.code.as_deref(),
        Some("browser_executor_missing_model")
    );
    assert_eq!(error.http_status, Some(400));
}
