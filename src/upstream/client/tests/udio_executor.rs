//! Udio dispatch uncertainty must not decline into a second local generation.
use super::*;

fn assert_abort(error: GatewayError) {
    assert!(!error.retryable);
    assert!(!crate::retry::should_retry(
        &error,
        &crate::retry::RetryPolicy::default()
    ));
    assert!(matches!(
        error.fallback_hint,
        crate::error::FallbackHint::Abort { .. }
    ));
    assert!(!error.message.contains("private-remote"));
}

#[tokio::test]
async fn udio_remote_http_decode_and_business_failures_abort_for_all_media() {
    for endpoint in [
        EndpointKind::MusicGenerations,
        EndpointKind::ImagesGenerations,
        EndpointKind::VideosGenerations,
    ] {
        for (status, body) in [
            (503, "private-remote"),
            (200, "not-json private-remote"),
            (200, r#"{"ok":true}"#),
            (200, r#"{"ok":true,"result":null}"#),
            (
                200,
                r#"{"ok":false,"error":{"status":429,"code":"fixture_rate_limit"}}"#,
            ),
        ] {
            let (base, server) = single_response_executor_base_url(status, "Fixture", body);
            let mut client = UpstreamClient::new(1);
            client.http = rquest::Client::builder().no_proxy().build().unwrap();
            client.browser_executor_base_url = Some(base);
            client.request_time_browser_policy = RequestTimeBrowserPolicy::LocalAllowed;
            let result = client
                .execute_remote_browser_executor("udio", "fixture", endpoint, json!({}))
                .await;
            server.join().unwrap();
            assert_abort(result.expect_err("dispatch must not decline into local generation"));
        }
    }
}

#[tokio::test]
async fn udio_remote_connection_failure_does_not_allow_local_fallback() {
    let mut client = UpstreamClient::new(1);
    client.http = rquest::Client::builder().no_proxy().build().unwrap();
    client.browser_executor_base_url = Some(unused_loopback_base_url());
    client.request_time_browser_policy = RequestTimeBrowserPolicy::LocalAllowed;
    assert_abort(
        client
            .execute_remote_browser_executor(
                "udio",
                "fixture",
                EndpointKind::MusicGenerations,
                json!({}),
            )
            .await
            .unwrap_err(),
    );
}

#[tokio::test]
async fn udio_remote_redirect_never_resubmits_generation() {
    use axum::{routing::post, Router};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    for status in [307, 308] {
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let path = "/v1/internal/browser-executor/execute";
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
                "udio",
                "fixture",
                EndpointKind::MusicGenerations,
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
async fn udio_unconfigured_remote_still_permits_explicit_local_policy() {
    let mut client = UpstreamClient::new(1);
    client.browser_executor_base_url = None;
    client.request_time_browser_policy = RequestTimeBrowserPolicy::LocalAllowed;
    assert_eq!(
        client
            .execute_remote_browser_executor(
                "udio",
                "fixture",
                EndpointKind::MusicGenerations,
                json!({})
            )
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn udio_media_owner_protects_errors_after_remote_success() {
    let (base, server) = single_response_executor_base_url(200, "OK", r#"{"ok":true,"result":{}}"#);
    let mut client = UpstreamClient::new(1);
    client.http = rquest::Client::builder().no_proxy().build().unwrap();
    client.browser_executor_base_url = Some(base);
    client.request_time_browser_policy = RequestTimeBrowserPolicy::LocalAllowed;
    let mut payload = make_payload("udio_compatible", "https://www.udio.com");
    payload
        .headers
        .insert("Cookie".into(), "fixture=not-real".into());
    let mut request = make_request(ProtocolFamily::OpenAi, EndpointKind::MusicGenerations);
    request.raw_body = json!({"prompt":"fixture"});
    let error = client
        .execute_udio_media("fixture", &payload, &request, "fixture", None)
        .await
        .unwrap_err();
    server.join().unwrap();
    assert_eq!(
        error.code.as_deref(),
        Some("udio_remote_result_parse_failed")
    );
    assert_eq!(error.provider_name.as_deref(), Some("udio_compatible"));
    assert_abort(error);
}
#[tokio::test]
async fn udio_no_replay_red_remote_failure_cannot_decline_locally() {
    let (base, server) = single_response_executor_base_url(503, "Fixture", "private-remote");
    let mut client = UpstreamClient::new(1);
    client.http = rquest::Client::builder().no_proxy().build().unwrap();
    client.browser_executor_base_url = Some(base);
    client.request_time_browser_policy = RequestTimeBrowserPolicy::LocalAllowed;
    let result = client
        .execute_remote_browser_executor(
            "udio",
            "fixture",
            EndpointKind::MusicGenerations,
            json!({}),
        )
        .await;
    server.join().unwrap();
    assert_abort(result.expect_err("remote dispatch uncertainty cannot request a local replay"));
}
