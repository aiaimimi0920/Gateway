use std::time::Duration;

use futures::StreamExt;

use super::body_targets::{cached_payload, invoke, Target};
use super::body_wire::{BodyMode, WireServer};
use super::{execute_stream, make_request, UpstreamStreamingResponse, PROVIDER};
use crate::error::ErrorKind;
use crate::protocol::upstream_body::MAX_ACCUMULATED_UPSTREAM_BODY_BYTES as LIMIT;

async fn rejects_before_eof(target: Target, mode: BodyMode) {
    let server =
        WireServer::start(target.path(), target.status(), target.content_type(), mode).await;
    let result = tokio::time::timeout(Duration::from_secs(8), invoke(target, &server.url)).await;
    server.stop().await;
    let error = result
        .expect("oversized body must be rejected before EOF")
        .expect_err("oversized body");
    assert_eq!(error.kind, ErrorKind::ServerError);
    assert_eq!(error.http_status, Some(500));
    assert_eq!(error.provider_name.as_deref(), Some(PROVIDER));
    assert_eq!(error.code.as_deref(), Some("upstream_body_too_large"));
    assert_eq!(
        error.message,
        format!(
            "{} exceeded the {LIMIT}-byte accumulation limit.",
            target.label()
        )
    );
}

#[tokio::test]
async fn bootstrap_rejects_declared_oversize_before_eof() {
    rejects_before_eof(Target::Bootstrap, BodyMode::DeclaredOversize).await;
}
#[tokio::test]
async fn requirements_rejects_declared_oversize_before_eof() {
    rejects_before_eof(Target::Requirements, BodyMode::DeclaredOversize).await;
}
#[tokio::test]
async fn prepare_rejects_declared_oversize_before_eof() {
    rejects_before_eof(Target::Prepare, BodyMode::DeclaredOversize).await;
}
#[tokio::test]
async fn conversation_rejects_declared_oversize_before_eof() {
    rejects_before_eof(Target::Conversation, BodyMode::DeclaredOversize).await;
}
#[tokio::test]
async fn stream_failure_rejects_declared_oversize_before_eof() {
    rejects_before_eof(Target::StreamFailure, BodyMode::DeclaredOversize).await;
}
#[tokio::test]
async fn stream_non_sse_rejects_declared_oversize_before_eof() {
    rejects_before_eof(Target::StreamNonSse, BodyMode::DeclaredOversize).await;
}

#[tokio::test]
async fn bootstrap_rejects_chunked_oversize_before_eof() {
    rejects_before_eof(Target::Bootstrap, BodyMode::ChunkedOversize).await;
}
#[tokio::test]
async fn requirements_rejects_chunked_oversize_before_eof() {
    rejects_before_eof(Target::Requirements, BodyMode::ChunkedOversize).await;
}
#[tokio::test]
async fn prepare_rejects_chunked_oversize_before_eof() {
    rejects_before_eof(Target::Prepare, BodyMode::ChunkedOversize).await;
}
#[tokio::test]
async fn conversation_rejects_chunked_oversize_before_eof() {
    rejects_before_eof(Target::Conversation, BodyMode::ChunkedOversize).await;
}
#[tokio::test]
async fn stream_failure_rejects_chunked_oversize_before_eof() {
    rejects_before_eof(Target::StreamFailure, BodyMode::ChunkedOversize).await;
}
#[tokio::test]
async fn stream_non_sse_rejects_chunked_oversize_before_eof() {
    rejects_before_eof(Target::StreamNonSse, BodyMode::ChunkedOversize).await;
}

#[tokio::test]
async fn prepare_accepts_exact_accumulation_limit() {
    let target = Target::Prepare;
    let server = WireServer::start(
        target.path(),
        200,
        target.content_type(),
        BodyMode::ExactLimit,
    )
    .await;
    let result = tokio::time::timeout(Duration::from_secs(15), invoke(target, &server.url)).await;
    server.stop().await;
    assert_eq!(
        result
            .expect("finite exact-limit body")
            .expect("accepted body"),
        ""
    );
}

#[tokio::test]
async fn conversation_preserves_http_charset_decoding() {
    let target = Target::Conversation;
    let body = b"data: {\"p\":\"/message/content/parts/0\",\"o\":\"append\",\"v\":\"caf\xe9\"}\n\ndata: [DONE]\n\n";
    let server = WireServer::start(
        target.path(),
        200,
        "text/event-stream; charset=windows-1252",
        BodyMode::Finite(body),
    )
    .await;
    let result = tokio::time::timeout(Duration::from_secs(8), invoke(target, &server.url)).await;
    server.stop().await;
    assert_eq!(
        result.expect("finite text").expect("decoded response"),
        "caf\u{e9}"
    );
}

#[tokio::test]
async fn live_sse_produces_text_before_body_completion() {
    let mut server = WireServer::start(
        Target::Conversation.path(),
        200,
        "text/event-stream",
        BodyMode::HeldSse,
    )
    .await;
    let http = rquest::Client::builder()
        .no_proxy()
        .build()
        .expect("client");
    let mut request = make_request();
    request.stream = true;
    let response = tokio::time::timeout(
        Duration::from_secs(8),
        execute_stream(
            &http,
            Duration::from_secs(5),
            &cached_payload(&server.url),
            &request,
            "auto",
            None,
        ),
    )
    .await
    .expect("stream headers")
    .expect("stream");
    let UpstreamStreamingResponse::Bytes(mut stream) = response else {
        panic!("translated stream")
    };
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(chunk) = stream.next().await {
            if String::from_utf8_lossy(&chunk.expect("first chunk"))
                .contains("\"content\":\"Paris\"")
            {
                return;
            }
        }
        panic!("stream ended before assistant text");
    })
    .await
    .expect("assistant text arrives while upstream is open");
    server.complete();
    let done_count = tokio::time::timeout(Duration::from_secs(5), async {
        let mut count = 0;
        while let Some(chunk) = stream.next().await {
            if chunk.expect("remaining chunk").as_ref() == b"data: [DONE]\n\n" {
                count += 1;
            }
        }
        count
    })
    .await
    .expect("stream completes");
    server.stop().await;
    assert_eq!(done_count, 1);
}
