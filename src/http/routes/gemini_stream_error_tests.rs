//! 实际 Gemini response builder 保留领域 source chain 和 exactly-once 清理。
use super::*;
use crate::protocol::stream_error::{ProtocolStreamError, StreamError};
use crate::protocol::stream_error_test_support::{http_body_error, probe, source_messages};
use bytes::Bytes;
use futures::StreamExt;
use std::error::Error;
use std::sync::atomic::{AtomicUsize, Ordering};

#[tokio::test]
async fn gemini_http_body_keeps_both_error_variants_and_terminal_ownership() {
    let errors = [
        StreamError::Protocol(ProtocolStreamError::invalid_data("local Gemini failure")),
        StreamError::Transport(http_body_error("Gemini body failure").await),
    ];
    for (index, error) in errors.into_iter().enumerate() {
        let expected = source_messages(&error);
        let (source, state) = probe(vec![Err(error), Ok(Bytes::from_static(b"late"))], false);
        let calls = Arc::new(AtomicUsize::new(0));
        let completion = Arc::clone(&calls);
        let probe_state = Arc::clone(&state);
        let stream = TrackedStream::new_with_error(source, move |metrics, success| {
            assert!(!success);
            assert_eq!(metrics.total_bytes, 0);
            assert_eq!(probe_state.drops.load(Ordering::SeqCst), 1);
            completion.fetch_add(1, Ordering::SeqCst);
        });
        let response = build_stream_generate_content_response(stream, "gemini-test").unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let mut body = response.into_body().into_data_stream();
        let received = body.next().await.unwrap().unwrap_err();
        let mut source: &(dyn Error + 'static) = &received;
        for _ in 0..8 {
            if source.is::<StreamError<rquest::Error>>() {
                break;
            }
            source = source
                .source()
                .expect("domain error must remain in HTTP body");
        }
        let error = source.downcast_ref::<StreamError<rquest::Error>>().unwrap();
        assert_eq!(matches!(error, StreamError::Protocol(_)), index == 0);
        assert_eq!(source_messages(error), expected);
        assert!(body.next().await.is_none());
        drop(body);
        assert_eq!(state.polls.load(Ordering::SeqCst), 1);
        assert_eq!(state.drops.load(Ordering::SeqCst), 1);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn cancelling_gemini_response_releases_pending_source_once() {
    let (source, state) = probe::<StreamError<rquest::Error>>(vec![], true);
    let calls = Arc::new(AtomicUsize::new(0));
    let completion = Arc::clone(&calls);
    let tracked = TrackedStream::new_with_error(source, move |_, success| {
        assert!(!success);
        completion.fetch_add(1, Ordering::SeqCst);
    });
    let mut body = build_stream_generate_content_response(tracked, "test")
        .unwrap()
        .into_body()
        .into_data_stream();
    assert!(futures::poll!(body.next()).is_pending());
    drop(body);
    assert_eq!(state.drops.load(Ordering::SeqCst), 1);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
