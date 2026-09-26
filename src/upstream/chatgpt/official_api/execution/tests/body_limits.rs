use std::time::Duration;

use super::body_targets::{client, Target};
use super::body_wire::{BodyMode, WireServer};
use super::execute_forced_streaming_accumulate;
use crate::error::{ErrorKind, GatewayError};
use crate::protocol::chatgpt::official_api as surface;
use crate::protocol::upstream_body::MAX_ACCUMULATED_UPSTREAM_BODY_BYTES as LIMIT;

fn assert_limit(error: GatewayError, label: &str, provider: &str) {
    assert_eq!(error.kind, ErrorKind::ServerError);
    assert_eq!(error.http_status, Some(500));
    assert_eq!(error.provider_name.as_deref(), Some(provider));
    assert_eq!(error.code.as_deref(), Some("upstream_body_too_large"));
    assert_eq!(
        error.message,
        format!("{label} exceeded the {LIMIT}-byte accumulation limit.")
    );
}

async fn rejects_before_eof(target: Target, mode: BodyMode) {
    let server = WireServer::start(target.path(), target.status(), "application/json", mode).await;
    let result = tokio::time::timeout(Duration::from_secs(8), target.invoke(&server.url)).await;
    server.stop().await;
    let error = result
        .expect("oversized body must be rejected before EOF")
        .expect_err("oversized body");
    assert_limit(error, target.label(), surface::CHATGPT_OFFICIAL_API_PROFILE);
}

#[tokio::test]
async fn json_rejects_declared_oversize_before_eof() {
    rejects_before_eof(Target::Json, BodyMode::DeclaredOversize).await;
}
#[tokio::test]
async fn json_rejects_chunked_oversize_before_eof() {
    rejects_before_eof(Target::Json, BodyMode::ChunkedOversize).await;
}
#[tokio::test]
async fn nonstream_error_rejects_declared_oversize_before_eof() {
    rejects_before_eof(Target::NonstreamError, BodyMode::DeclaredOversize).await;
}
#[tokio::test]
async fn nonstream_error_rejects_chunked_oversize_before_eof() {
    rejects_before_eof(Target::NonstreamError, BodyMode::ChunkedOversize).await;
}
#[tokio::test]
async fn forced_error_rejects_declared_oversize_before_eof() {
    rejects_before_eof(Target::ForcedError, BodyMode::DeclaredOversize).await;
}
#[tokio::test]
async fn forced_error_rejects_chunked_oversize_before_eof() {
    rejects_before_eof(Target::ForcedError, BodyMode::ChunkedOversize).await;
}
#[tokio::test]
async fn stream_error_rejects_declared_oversize_before_eof() {
    rejects_before_eof(Target::StreamError, BodyMode::DeclaredOversize).await;
}
#[tokio::test]
async fn stream_error_rejects_chunked_oversize_before_eof() {
    rejects_before_eof(Target::StreamError, BodyMode::ChunkedOversize).await;
}

#[tokio::test]
async fn forced_error_oversize_retains_codex_provider() {
    let target = Target::ForcedError;
    let server = WireServer::start(
        target.path(),
        429,
        "application/json",
        BodyMode::DeclaredOversize,
    )
    .await;
    let client = client();
    let result = tokio::time::timeout(
        Duration::from_secs(8),
        execute_forced_streaming_accumulate(
            client
                .http
                .post(format!("{}{}", server.url, target.path()))
                .timeout(Duration::from_secs(30)),
            surface::CHATGPT_CODEX_BACKEND_PROFILE,
            "fixture-model",
        ),
    )
    .await;
    server.stop().await;
    assert_limit(
        result
            .expect("oversized body must be rejected before EOF")
            .expect_err("oversize"),
        target.label(),
        surface::CHATGPT_CODEX_BACKEND_PROFILE,
    );
}
