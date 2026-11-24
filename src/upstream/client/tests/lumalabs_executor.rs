//! A dispatched Luma Web generation is never a reason to submit locally again.
use super::*;
fn assert_abort(error: GatewayError) {
    assert!(!error.retryable);
    assert!(matches!(
        error.fallback_hint,
        crate::error::FallbackHint::Abort { .. }
    ));
    assert!(!crate::retry::should_retry(
        &error,
        &crate::retry::RetryPolicy::default()
    ));
    assert!(!error.message.contains("private-remote"));
}
#[tokio::test]
async fn luma_web_red_remote_failures_cannot_decline_into_local_generation() {
    for endpoint in [
        EndpointKind::ImagesGenerations,
        EndpointKind::VideosGenerations,
        EndpointKind::MusicGenerations,
    ] {
        for (status, body) in [
            (503, "private-remote"),
            (200, "private-remote invalid"),
            (200, r#"{"ok":true}"#),
            (200, r#"{"ok":true,"result":null}"#),
            (
                200,
                r#"{"ok":false,"error":{"status":429,"code":"fixture_limit"}}"#,
            ),
        ] {
            let (base, server) = single_response_executor_base_url(status, "Fixture", body);
            let mut client = UpstreamClient::new(1);
            client.http = rquest::Client::builder().no_proxy().build().unwrap();
            client.browser_executor_base_url = Some(base);
            client.request_time_browser_policy = RequestTimeBrowserPolicy::LocalAllowed;
            let result = client
                .execute_remote_browser_executor("lumalabs", "fixture", endpoint, json!({}))
                .await;
            server.join().unwrap();
            assert_abort(
                result.expect_err("an uncertain dispatched generation must not fall back locally"),
            );
        }
    }
}
#[tokio::test]
async fn luma_web_no_remote_keeps_explicit_local_policy() {
    let mut client = UpstreamClient::new(1);
    client.browser_executor_base_url = None;
    client.request_time_browser_policy = RequestTimeBrowserPolicy::LocalAllowed;
    assert_eq!(
        client
            .execute_remote_browser_executor(
                "lumalabs",
                "fixture",
                EndpointKind::ImagesGenerations,
                json!({})
            )
            .await
            .unwrap(),
        None
    );
}
#[tokio::test]
async fn luma_web_remote_connection_failure_aborts() {
    let mut client = UpstreamClient::new(1);
    client.http = rquest::Client::builder().no_proxy().build().unwrap();
    client.browser_executor_base_url = Some(unused_loopback_base_url());
    client.request_time_browser_policy = RequestTimeBrowserPolicy::LocalAllowed;
    assert_abort(
        client
            .execute_remote_browser_executor(
                "lumalabs",
                "fixture",
                EndpointKind::ImagesGenerations,
                json!({}),
            )
            .await
            .unwrap_err(),
    );
}
#[tokio::test]
async fn luma_web_remote_redirect_does_not_submit_twice() {
    use axum::{routing::post, Router};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    for status in [307, 308] {
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let path = "/v1/internal/browser-executor/luma-session-v1/execute";
        let app = Router::new().route(
            path,
            post(move || {
                observed.fetch_add(1, Ordering::SeqCst);
                async move {
                    (
                        axum::http::StatusCode::from_u16(status).unwrap(),
                        [("location", path)],
                        "private-remote",
                    )
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let mut client = UpstreamClient::new(1);
        client.http = rquest::Client::builder().no_proxy().build().unwrap();
        client.browser_executor_base_url = Some(base);
        client.request_time_browser_policy = RequestTimeBrowserPolicy::LocalAllowed;
        let result = client
            .execute_remote_browser_executor(
                "lumalabs",
                "fixture",
                EndpointKind::ImagesGenerations,
                json!({}),
            )
            .await;
        server.abort();
        let _ = server.await;
        assert_abort(result.unwrap_err());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
#[tokio::test]
async fn luma_web_media_owner_protects_errors_after_remote_success() {
    // The worker envelope succeeds, but the required task context is missing.
    let (base, server) = single_response_executor_base_url(
        200,
        "OK",
        r#"{"ok":true,"result":{"ok":true,"signedUrl":"https://assets.example.test/fixture.png"}}"#,
    );
    let mut client = UpstreamClient::new(1);
    client.http = rquest::Client::builder().no_proxy().build().unwrap();
    client.browser_executor_base_url = Some(base);
    client.request_time_browser_policy = RequestTimeBrowserPolicy::LocalAllowed;
    let mut payload = make_payload("lumalabs_compatible", "https://app.lumalabs.ai");
    payload.extra_body = Some(HashMap::from([("realmId".into(), json!("fixture"))]));
    let mut request = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesGenerations);
    request.raw_body = json!({"prompt":"fixture"});
    let result = client
        .execute_json_passthrough(
            "fixture",
            ProviderExecutionMode::BrowserBacked,
            &payload,
            &request,
            "uni-1",
            None,
        )
        .await;
    server.join().unwrap();
    let error = result.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("lumalabs_task_result_invalid"));
    assert_eq!(error.provider_name.as_deref(), Some("lumalabs_compatible"));
    assert_abort(error);
}
#[tokio::test]
async fn luma_web_remote_chunked_metadata_budget_is_128_kib() {
    use axum::{body::Body, response::Response, routing::post, Router};
    for extra in [0, 1] {
        let mut data = json!({"ok":true,"result":{"pad":""}});
        let base_size = serde_json::to_vec(&data).unwrap().len();
        data["result"]["pad"] = json!("x".repeat(128 * 1024 + extra - base_size));
        let bytes = serde_json::to_vec(&data).unwrap();
        let app = Router::new().route(
            "/v1/internal/browser-executor/luma-session-v1/execute",
            post(move || {
                let bytes = bytes.clone();
                async move {
                    let chunks: Vec<_> = bytes
                        .chunks(1024)
                        .map(|part| Ok::<_, std::io::Error>(bytes::Bytes::copy_from_slice(part)))
                        .collect();
                    Response::builder()
                        .header("content-type", "application/json")
                        .body(Body::from_stream(futures::stream::iter(chunks)))
                        .unwrap()
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let mut client = UpstreamClient::new(1);
        client.http = rquest::Client::builder().no_proxy().build().unwrap();
        client.browser_executor_base_url = Some(base);
        client.request_time_browser_policy = RequestTimeBrowserPolicy::LocalAllowed;
        let result = client
            .execute_remote_browser_executor(
                "lumalabs",
                "fixture",
                EndpointKind::ImagesGenerations,
                json!({}),
            )
            .await;
        server.abort();
        let _ = server.await;
        if extra == 0 {
            assert!(result.unwrap().is_some());
        } else {
            assert_abort(result.unwrap_err());
        }
    }
}
