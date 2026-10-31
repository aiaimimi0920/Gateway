//! Remote submission may have generated even when its response is lost.
use super::*;

fn media_request(endpoint: EndpointKind) -> CanonicalRelayRequest {
    serde_json::from_value(json!({
        "protocol_family":crate::protocol::canonical::ProtocolFamily::OpenAi, "endpoint_kind":endpoint, "stream":false,
        "messages":[], "tools":[], "raw_body":{"prompt":"fixture", "n":1},
    }))
    .unwrap()
}

#[tokio::test]
async fn modular_web_explicit_execution_policy_rejects_unsupported_media_before_any_dispatch() {
    let client = UpstreamClient::new(1);
    for (endpoint, mode) in [
        (EndpointKind::MusicGenerations, "required"),
        (EndpointKind::VideosGenerations, "required"),
        (EndpointKind::ImagesEdits, "disabled"),
    ] {
        let mut payload = make_payload(
            "gemini_web_reverse_modular_compatible",
            "https://gemini.google.com",
        );
        payload.extra_body = Some(HashMap::from([("pureHttpMode".into(), json!(mode))]));
        let error = client
            .execute_gemini_canvas_media(
                "fixture",
                &payload,
                &media_request(endpoint),
                "fixture",
                None,
            )
            .await
            .unwrap_err();
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_web_media_execution_policy_unsupported")
        );
        assert!(!error.retryable);
        assert!(matches!(
            error.fallback_hint,
            crate::error::FallbackHint::Abort { .. }
        ));
    }
}

#[tokio::test]
async fn ordinary_web_image_remote_decode_failure_is_not_retried_on_another_account() {
    use axum::{routing::post, Json, Router};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let app = Router::new().route(
        "/v1/internal/browser-executor/execute",
        post(move || {
            observed.fetch_add(1, Ordering::SeqCst);
            async { Json(json!({"ok":true,"result":{}})) }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut client = UpstreamClient::new(1);
    client.http = rquest::Client::builder().no_proxy().build().unwrap();
    client.browser_executor_base_url = Some(base);
    client.request_time_browser_policy = RequestTimeBrowserPolicy::RemoteOnly;
    for adapter in [
        "gemini_web_compatible",
        "gemini_web_reverse_modular_compatible",
    ] {
        let before = calls.load(Ordering::SeqCst);
        let mut payload = make_payload(adapter, "https://gemini.google.com");
        payload.runtime_state_object_key = Some("synthetic-not-read".into());
        payload.extra_body = Some(HashMap::from([("pureHttpMode".into(), json!("disabled"))]));
        let error = client
            .execute_json_passthrough(
                "fixture",
                ProviderExecutionMode::DirectHttp,
                &payload,
                &media_request(EndpointKind::ImagesGenerations),
                gemini_canvas::GEMINI_25_FLASH_IMAGE_MODEL,
                None,
            )
            .await
            .unwrap_err();
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_remote_result_parse_failed")
        );
        assert!(!error.retryable);
        assert!(matches!(
            error.fallback_hint,
            crate::error::FallbackHint::Abort { .. }
        ));
        assert_eq!(calls.load(Ordering::SeqCst), before + 1);
    }
    server.abort();
    let _ = server.await;
}

#[tokio::test]
async fn gemini_media_remote_failures_never_decline_into_local_dispatch() {
    for endpoint in [
        EndpointKind::ImagesGenerations,
        EndpointKind::ImagesEdits,
        EndpointKind::MusicGenerations,
        EndpointKind::VideosGenerations,
    ] {
        for (status, body) in [
            (503, "fixture remote response lost"),
            (200, "not-json"),
            (200, r#"{"ok":true}"#),
            (200, r#"{"ok":true,"result":null}"#),
        ] {
            let (base, server) = single_response_executor_base_url(status, "Fixture", body);
            let mut client = UpstreamClient::new(1);
            client.browser_executor_base_url = Some(base);
            client.request_time_browser_policy = RequestTimeBrowserPolicy::LocalAllowed;
            let result = client
                .execute_remote_browser_executor(
                    "gemini_canvas",
                    "fixture",
                    endpoint,
                    json!({"operation": "image"}),
                )
                .await;
            server.join().unwrap();
            let error = result.expect_err("a dispatched media request cannot fall back locally");
            assert!(!error.retryable);
            assert!(matches!(
                error.fallback_hint,
                crate::error::FallbackHint::Abort { .. }
            ));
            assert!(!error.message.contains("fixture remote response lost"));
        }
    }
}

#[tokio::test]
async fn gemini_media_connection_failure_is_not_a_local_fallback() {
    let mut client = UpstreamClient::new(1);
    client.browser_executor_base_url = Some(unused_loopback_base_url());
    client.request_time_browser_policy = RequestTimeBrowserPolicy::LocalAllowed;
    let error = client
        .execute_remote_browser_executor(
            "gemini_canvas",
            "fixture",
            EndpointKind::ImagesGenerations,
            json!({}),
        )
        .await
        .expect_err("dispatch uncertainty must fail closed");
    assert!(!error.retryable);
    assert!(matches!(
        error.fallback_hint,
        crate::error::FallbackHint::Abort { .. }
    ));
}

#[tokio::test]
async fn gemini_media_remote_redirect_does_not_resubmit_the_generation() {
    use axum::{routing::post, Router};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    for status in [307, 308] {
        let hits = Arc::new(AtomicUsize::new(0));
        let observed = hits.clone();
        let path = "/v1/internal/browser-executor/execute";
        let app = Router::new().route(
            path,
            post(move || {
                observed.fetch_add(1, Ordering::SeqCst);
                async move {
                    (
                        axum::http::StatusCode::from_u16(status).unwrap(),
                        [("location", path)],
                        "redirect",
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
                "gemini_canvas",
                "fixture",
                EndpointKind::ImagesGenerations,
                json!({"operation":"image"}),
            )
            .await;
        server.abort();
        let _ = server.await;
        assert!(result.is_err());
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn gemini_text_and_other_providers_keep_local_fallback() {
    for (provider, endpoint) in [
        ("gemini_canvas", EndpointKind::ChatCompletions),
        ("producer", EndpointKind::ImagesGenerations),
    ] {
        let (base, server) = single_response_executor_base_url(503, "Fixture", "unavailable");
        let mut client = UpstreamClient::new(1);
        client.browser_executor_base_url = Some(base);
        client.request_time_browser_policy = RequestTimeBrowserPolicy::LocalAllowed;
        let result = client
            .execute_remote_browser_executor(provider, "fixture", endpoint, json!({}))
            .await;
        server.join().unwrap();
        assert_eq!(result.unwrap(), None);
    }
}
