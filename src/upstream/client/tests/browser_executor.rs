use super::*;

#[tokio::test]
async fn remote_only_browser_executor_without_base_url_returns_required_unavailable_error() {
    let mut client = UpstreamClient::new(5);
    client.browser_executor_base_url = None;
    client.request_time_browser_policy = RequestTimeBrowserPolicy::RemoteOnly;

    let err = client
        .execute_remote_browser_executor(
            "gemini_canvas",
            "provider-account-1",
            EndpointKind::ImagesGenerations,
            json!({"prompt": "image"}),
        )
        .await
        .expect_err("remote_only policy must fail closed instead of returning Ok(None)");

    assert_eq!(
        err.code.as_deref(),
        Some("browser_executor_required_unavailable")
    );
    assert_eq!(err.http_status, Some(503));
}

#[test]
fn remote_browser_executor_timeout_covers_the_worker_budget() {
    assert_eq!(
        browser_executor_remote_request_timeout(
            Duration::from_secs(120),
            &json!({"timeoutMs": 300_000}),
        ),
        Duration::from_secs(320),
    );
    assert_eq!(
        browser_executor_remote_request_timeout(Duration::from_secs(120), &json!({})),
        Duration::from_secs(140),
    );
}

#[tokio::test]
async fn disabled_browser_executor_without_base_url_returns_forbidden_error() {
    let mut client = UpstreamClient::new(5);
    client.browser_executor_base_url = None;
    client.request_time_browser_policy = RequestTimeBrowserPolicy::Disabled;

    let err = client
        .execute_remote_browser_executor(
            "gemini_canvas",
            "provider-account-1",
            EndpointKind::ImagesGenerations,
            json!({"prompt": "image"}),
        )
        .await
        .expect_err("disabled policy must fail closed instead of allowing local fallback");

    assert_eq!(err.code.as_deref(), Some("request_time_browser_forbidden"));
    assert_eq!(err.http_status, Some(503));
}

#[tokio::test]
async fn remote_only_browser_executor_unreachable_base_url_returns_required_unavailable_error() {
    let mut client = UpstreamClient::new(5);
    client.browser_executor_base_url = Some(unused_loopback_base_url());
    client.request_time_browser_policy = RequestTimeBrowserPolicy::RemoteOnly;

    let err = client
        .execute_remote_browser_executor(
            "gemini_canvas",
            "provider-account-1",
            EndpointKind::ImagesGenerations,
            json!({"prompt": "image"}),
        )
        .await
        .expect_err("unreachable remote_only executor must fail closed");

    assert_eq!(
        err.code.as_deref(),
        Some("browser_executor_required_unavailable")
    );
    assert_eq!(err.http_status, Some(503));
}

#[tokio::test]
async fn disabled_browser_executor_unreachable_base_url_returns_required_unavailable_error() {
    let mut client = UpstreamClient::new(5);
    client.browser_executor_base_url = Some(unused_loopback_base_url());
    client.request_time_browser_policy = RequestTimeBrowserPolicy::Disabled;

    let err = client
        .execute_remote_browser_executor(
            "gemini_canvas",
            "provider-account-1",
            EndpointKind::ImagesGenerations,
            json!({"prompt": "image"}),
        )
        .await
        .expect_err("disabled policy must not allow local fallback when remote is unreachable");

    assert_eq!(
        err.code.as_deref(),
        Some("browser_executor_required_unavailable")
    );
    assert_eq!(err.http_status, Some(503));
}

#[tokio::test]
async fn remote_only_browser_executor_non_success_status_returns_required_failed_error() {
    let (base_url, server) = single_response_executor_base_url(
        503,
        "Service Unavailable",
        r#"{"ok":false,"error":{"code":"executor_down","message":"down"}}"#,
    );
    let mut client = UpstreamClient::new(5);
    client.browser_executor_base_url = Some(base_url);
    client.request_time_browser_policy = RequestTimeBrowserPolicy::RemoteOnly;

    let err = client
        .execute_remote_browser_executor(
            "gemini_canvas",
            "provider-account-1",
            EndpointKind::ImagesGenerations,
            json!({"prompt": "image"}),
        )
        .await
        .expect_err("remote_only executor non-success status must fail closed");
    server.join().expect("executor response server exits");

    assert_eq!(
        err.code.as_deref(),
        Some("browser_executor_required_failed")
    );
    assert_eq!(err.http_status, Some(503));
}

#[tokio::test]
async fn disabled_browser_executor_non_success_status_returns_required_failed_error() {
    let (base_url, server) = single_response_executor_base_url(
        503,
        "Service Unavailable",
        r#"{"ok":false,"error":{"code":"executor_down","message":"down"}}"#,
    );
    let mut client = UpstreamClient::new(5);
    client.browser_executor_base_url = Some(base_url);
    client.request_time_browser_policy = RequestTimeBrowserPolicy::Disabled;

    let err = client
        .execute_remote_browser_executor(
            "gemini_canvas",
            "provider-account-1",
            EndpointKind::ImagesGenerations,
            json!({"prompt": "image"}),
        )
        .await
        .expect_err("disabled policy must not allow local fallback on remote non-success");
    server.join().expect("executor response server exits");

    assert_eq!(
        err.code.as_deref(),
        Some("browser_executor_required_failed")
    );
    assert_eq!(err.http_status, Some(503));
}

#[tokio::test]
async fn remote_only_browser_executor_invalid_json_returns_required_failed_error() {
    let (base_url, server) = single_response_executor_base_url(200, "OK", "not-json");
    let mut client = UpstreamClient::new(5);
    client.browser_executor_base_url = Some(base_url);
    client.request_time_browser_policy = RequestTimeBrowserPolicy::RemoteOnly;

    let err = client
        .execute_remote_browser_executor(
            "gemini_canvas",
            "provider-account-1",
            EndpointKind::ImagesGenerations,
            json!({"prompt": "image"}),
        )
        .await
        .expect_err("remote_only executor invalid JSON must fail closed");
    server.join().expect("executor response server exits");

    assert_eq!(
        err.code.as_deref(),
        Some("browser_executor_required_failed")
    );
    assert_eq!(err.http_status, Some(503));
}

#[tokio::test]
async fn disabled_browser_executor_invalid_json_returns_required_failed_error() {
    let (base_url, server) = single_response_executor_base_url(200, "OK", "not-json");
    let mut client = UpstreamClient::new(5);
    client.browser_executor_base_url = Some(base_url);
    client.request_time_browser_policy = RequestTimeBrowserPolicy::Disabled;

    let err = client
        .execute_remote_browser_executor(
            "gemini_canvas",
            "provider-account-1",
            EndpointKind::ImagesGenerations,
            json!({"prompt": "image"}),
        )
        .await
        .expect_err("disabled policy must not allow local fallback on remote invalid JSON");
    server.join().expect("executor response server exits");

    assert_eq!(
        err.code.as_deref(),
        Some("browser_executor_required_failed")
    );
    assert_eq!(err.http_status, Some(503));
}
