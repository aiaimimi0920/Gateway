use super::*;
use crate::protocol::stream_error::{ProtocolStreamError, StreamError};
use crate::protocol::stream_error_test_support::probe;
use std::error::Error as StdError;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

type Error = StreamError<std::io::Error>;

#[tokio::test]
async fn http_body_preserves_both_domain_variants_and_terminal_cleanup() {
    let errors = [
        Error::Protocol(ProtocolStreamError::invalid_data("local failure")),
        Error::Transport(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "body reset",
        )),
    ];
    for (index, error) in errors.into_iter().enumerate() {
        let (source, state) = probe(vec![Err(error), Ok(Bytes::from_static(b"late"))], false);
        let calls = Arc::new(AtomicUsize::new(0));
        let completion = Arc::clone(&calls);
        let probe_state = Arc::clone(&state);
        let tracked = TrackedStream::new_with_error(source, move |_, success| {
            assert!(!success);
            assert_eq!(probe_state.drops.load(Ordering::SeqCst), 1);
            completion.fetch_add(1, Ordering::SeqCst);
        });
        let response = into_sse_response(tracked, EndpointKind::ChatCompletions).into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let mut body = response.into_body().into_data_stream();
        let error = body.next().await.unwrap().unwrap_err();
        let mut source: &(dyn StdError + 'static) = &error;
        for _ in 0..8 {
            if source.is::<Error>() {
                break;
            }
            source = source
                .source()
                .expect("domain error must remain in the HTTP source chain");
        }
        match source.downcast_ref::<Error>().unwrap() {
            Error::Protocol(error) => {
                assert_eq!(index, 0);
                assert!(error.to_string().contains("local failure"));
            }
            Error::Transport(error) => {
                assert_eq!(index, 1);
                assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe);
            }
        }
        assert!(body.next().await.is_none());
        drop(body);
        assert_eq!(state.polls.load(Ordering::SeqCst), 1);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn keepalive_does_not_pollute_metrics_and_cancel_drops_typed_source() {
    let (source, state) = probe::<Error>(vec![], true);
    let calls = Arc::new(AtomicUsize::new(0));
    let completion = Arc::clone(&calls);
    let tracked = TrackedStream::new_with_error(source, move |metrics, success| {
        assert!(!success);
        assert_eq!(metrics.total_bytes, 0);
        assert_eq!(metrics.chunk_count, 0);
        completion.fetch_add(1, Ordering::SeqCst);
    });
    let mut stream = KeepAliveStream::new(tracked, Duration::from_millis(1));
    let ping = tokio::time::timeout(Duration::from_secs(2), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(ping.as_ref(), SSE_KEEPALIVE_FRAME);
    drop(stream);
    assert_eq!(state.drops.load(Ordering::SeqCst), 1);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
