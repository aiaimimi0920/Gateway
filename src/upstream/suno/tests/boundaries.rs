//! Resource ownership, decoded metadata limits, no redirects and real retry-owner integration.
use super::*;
use std::time::Duration;

#[tokio::test]
async fn suno_direct_decoded_body_is_bounded_even_with_chunked_json() {
    let server = Server::start(vec![
        Reply::json(json!({"required":false})),
        Reply::json(json!({"clips":[clip("a","complete")],"padding":"x".repeat(128*1024)})),
    ])
    .await;
    let error = UpstreamClient::new(3)
        .execute_suno_media(
            &payload(&server.origin),
            &request(json!({})),
            "chirp-v3-5",
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("suno_body_limit"));
    assert_eq!(server.paths().len(), 2);
    server.stop().await;
}

#[tokio::test]
async fn suno_direct_cumulative_budget_is_enforced_by_stream_reader() {
    use crate::upstream::suno::transport;
    use crate::upstream::suno_runtime_helpers::{
        build_suno_challenge_check_plan, prepare_suno_execution_context,
    };
    let server = Server::start(vec![Reply::json(json!({"required":false}))]).await;
    let req = request(json!({}));
    let payload = payload(&server.origin);
    let context =
        prepare_suno_execution_context(&payload, &req, None, false, Duration::from_secs(3))
            .unwrap();
    let mut total = transport::TOTAL_LIMIT - 1;
    let error = transport::send(
        &UpstreamClient::new(3),
        &build_suno_challenge_check_plan(&server.origin, req.endpoint_kind),
        &context,
        tokio::time::Instant::now() + Duration::from_secs(3),
        &mut total,
    )
    .await
    .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("suno_body_limit"));
    assert_eq!(total, transport::TOTAL_LIMIT - 1);
    server.stop().await;
}

#[tokio::test]
async fn suno_direct_deadline_covers_an_infinite_metadata_body_and_closes_it() {
    let mut hold = Reply::json(json!({}));
    hold.hold = true;
    let server = Server::start(vec![Reply::json(json!({"required":false})), hold]).await;
    let started = std::time::Instant::now();
    let error = UpstreamClient::new(1)
        .execute_suno_media(
            &payload(&server.origin),
            &request(json!({})),
            "chirp-v3-5",
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("suno_timeout"));
    assert!(started.elapsed() < Duration::from_secs(3));
    server.wait_closed().await;
    assert_eq!(server.paths().len(), 2);
    server.stop().await;
}

#[tokio::test]
async fn suno_direct_cancellation_drops_inflight_body_without_replay() {
    let mut hold = Reply::json(json!({}));
    hold.hold = true;
    let server = Server::start(vec![
        Reply::json(json!({"required":false})),
        Reply::json(json!({"clips":[clip("a","submitted")]})),
        hold,
    ])
    .await;
    let client = UpstreamClient::new(5);
    let payload = payload(&server.origin);
    let req = request(json!({}));
    let mut future = Box::pin(client.execute_suno_media(&payload, &req, "chirp-v3-5", None));
    tokio::select! { _=server.wait_calls(3)=>{}, result=&mut future=>panic!("unexpected result {result:?}") }
    drop(future);
    server.wait_closed().await;
    assert_eq!(server.paths().len(), 3);
    server.stop().await;
}

#[tokio::test]
async fn suno_direct_errors_do_not_follow_redirects_or_expose_json_html_secrets() {
    for (status, mime, body) in [
        (307, "text/plain", "redirect private-token"),
        (200, "text/html", "<html>private-token</html>"),
        (
            200,
            "application/json",
            "{\"token\":\"private-token\" invalid",
        ),
        (401, "application/json", "{\"token\":\"private-token\"}"),
        (429, "text/plain", "private-token"),
    ] {
        let mut reply = Reply::status(status, body);
        reply.mime = mime;
        let server = Server::start(vec![Reply::json(json!({"required":false})), reply]).await;
        let error = UpstreamClient::new(3)
            .execute_suno_media(
                &payload(&server.origin),
                &request(json!({})),
                "chirp-v3-5",
                None,
            )
            .await
            .unwrap_err();
        assert!(!error.message.contains("private-token"));
        assert!(!error.retryable);
        assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
        assert_eq!(server.paths().len(), 2);
        server.stop().await;
    }
}

#[tokio::test]
async fn suno_direct_real_retry_owner_does_not_reissue_generation() {
    let server = Server::start(vec![
        Reply::json(json!({"required":false})),
        Reply::status(503, "private failure"),
    ])
    .await;
    let client = UpstreamClient::new(3);
    let payload = payload(&server.origin);
    let req = request(json!({}));
    let error = crate::retry::execute_with_retry(
        || client.execute_suno_media(&payload, &req, "chirp-v3-5", None),
        &crate::retry::RetryPolicy::default(),
    )
    .await
    .unwrap_err();
    assert!(!error.retryable);
    assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
    assert_eq!(server.paths(), ["/api/c/check", "/api/generate/v2-web/"]);
    server.stop().await;
}
