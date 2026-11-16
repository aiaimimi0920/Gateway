//! Direct Web execution regression tests; synthetic HTTP never reaches Suno.
mod assets;
mod boundaries;
mod fixture;
mod polling;
mod worker;
use crate::{error::FallbackHint, upstream::client::UpstreamClient};
use fixture::*;
use serde_json::json;

#[tokio::test]
async fn suno_direct_rejects_foreign_completed_feed_identity() {
    let server = Server::start(vec![
        Reply::json(json!({"required":false})),
        Reply::json(json!({"clips":[clip("ours", "submitted")]})),
        Reply::json(json!({"clips":[clip("someone-else", "complete")]})),
    ])
    .await;
    let error = UpstreamClient::new(5)
        .execute_suno_media(
            &payload(&server.origin),
            &request(json!({})),
            "chirp-v3-5",
            None,
        )
        .await
        .expect_err("another clip must not complete this generation");
    assert_eq!(error.code.as_deref(), Some("suno_clip_identity"));
    assert!(!error.retryable);
    assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
    assert_eq!(
        server.paths(),
        ["/api/c/check", "/api/generate/v2-web/", "/api/feed/v3"]
    );
}

#[tokio::test]
async fn suno_direct_rejects_partly_malformed_generation_before_success() {
    let server = Server::start(vec![
        Reply::json(json!({"required":false})),
        Reply::json(json!({"clips":[clip("ours", "complete"), {"status":"complete"}]})),
    ])
    .await;
    let error = UpstreamClient::new(5)
        .execute_suno_media(
            &payload(&server.origin),
            &request(json!({"waitCompletion":false})),
            "chirp-v3-5",
            None,
        )
        .await
        .expect_err("invalid items must not be silently dropped");
    assert_eq!(error.code.as_deref(), Some("suno_clip_identity"));
    assert_eq!(server.paths().len(), 2);
}

#[tokio::test]
async fn suno_direct_post_submission_failure_aborts_retry_and_fallback() {
    let server = Server::start(vec![
        Reply::json(json!({"required":false})),
        Reply::json(json!({"clips":[clip("ours", "submitted")]})),
        Reply::status(503, "private upstream failure"),
    ])
    .await;
    let error = UpstreamClient::new(5)
        .execute_suno_media(
            &payload(&server.origin),
            &request(json!({})),
            "chirp-v3-5",
            None,
        )
        .await
        .unwrap_err();
    assert!(
        !error.retryable,
        "generation may already have consumed credits"
    );
    assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
    assert!(!error.message.contains("private upstream"));
    assert_eq!(server.paths().len(), 3);
}

#[tokio::test]
async fn suno_direct_failed_clip_never_returns_success_envelope() {
    let server = Server::start(vec![
        Reply::json(json!({"required":false})),
        Reply::json(json!({"clips":[clip("ours", "error")]})),
    ])
    .await;
    let error = UpstreamClient::new(5)
        .execute_suno_media(
            &payload(&server.origin),
            &request(json!({"waitCompletion":false})),
            "chirp-v3-5",
            None,
        )
        .await
        .expect_err("explicit no-wait is not permission to hide a failed clip");
    assert_eq!(error.code.as_deref(), Some("suno_generation_failed"));
}
