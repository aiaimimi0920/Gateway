use super::*;
use std::io;
use std::sync::atomic::Ordering;

use crate::error::ErrorKind;
use crate::protocol::stream_error_test_support::probe;
use serde_json::json;

fn wire(value: Value) -> Bytes {
    Bytes::from(format!("data: {value}\n\n"))
}

#[tokio::test]
#[cfg(feature = "line-accio-web-reverse-api")]
async fn buffered_domain_bridge_preserves_text_model_usage_and_finish() {
    let (inner, state) = probe::<io::Error>(
        vec![
            Ok(wire(
                json!({"type":"message_start", "message":{"model":"test-model",
            "usage":{"input_tokens":2, "output_tokens":0}}}),
            )),
            Ok(wire(json!({"type":"content_block_delta", "index":0,
            "delta":{"type":"text_delta", "text":"Hello"}}))),
            Ok(wire(
                json!({"type":"message_delta", "delta":{"stop_reason":"end_turn"},
            "usage":{"output_tokens":3}}),
            )),
        ],
        false,
    );
    let response = accumulate(inner, &uuid::Uuid::nil(), "test-model", vec![], None, None)
        .await
        .unwrap();
    assert_eq!(response.text, "Hello");
    assert_eq!(response.model, "test-model");
    assert_eq!(response.finish_reason.as_deref(), Some("completed"));
    let usage = response.usage.unwrap();
    assert_eq!(
        (
            usage.prompt_tokens,
            usage.completion_tokens,
            usage.total_tokens
        ),
        (2, 3, 5)
    );
    assert_eq!(state.drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
#[cfg(feature = "line-accio-web-reverse-api")]
async fn buffered_domain_bridge_returns_same_gateway_error_and_releases_before_feedback() {
    let (inner, state) = probe(
        vec![
            Err(io::Error::new(io::ErrorKind::ConnectionReset, "body reset")),
            Ok(Bytes::from_static(b"data: must-not-read\n\n")),
        ],
        false,
    );
    let error = accumulate(inner, &uuid::Uuid::nil(), "test", vec![], None, None)
        .await
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::ServerError);
    assert_eq!(
        error.message,
        "failed to read translated responses SSE chunk: body reset"
    );
    assert_eq!(state.polls.load(Ordering::SeqCst), 1);
    assert_eq!(state.drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
#[cfg(feature = "line-accio-web-reverse-api")]
async fn buffered_domain_bridge_cancellation_releases_all_upstream_owners() {
    let (inner, state) = probe::<io::Error>(vec![], true);
    let id = uuid::Uuid::nil();
    let mut future = Box::pin(accumulate(inner, &id, "test", vec![], None, None));
    assert!(futures::poll!(&mut future).is_pending());
    drop(future);
    assert_eq!(state.polls.load(Ordering::SeqCst), 1);
    assert_eq!(state.drops.load(Ordering::SeqCst), 1);
}
