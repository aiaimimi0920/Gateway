use super::*;
use std::sync::atomic::Ordering;

use crate::protocol::stream_error::StreamError;
use crate::protocol::stream_error_test_support::probe;
use futures::StreamExt;

type Error = StreamError<std::io::Error>;

#[tokio::test]
async fn typed_replay_preserves_fragmented_bytes() {
    let parts = vec![
        Bytes::from_static(b"data: {\"choices\":[{\"delta\":{\"cont"),
        Bytes::from_static(b"ent\":\"plain\"}}]}\n\ndata: [DONE]\n\n"),
    ];
    let inner = futures::stream::iter(
        parts
            .iter()
            .cloned()
            .map(Ok::<_, Error>)
            .collect::<Vec<_>>(),
    );
    let chunks = wrap_streaming_tool_detection_with_error(
        inner,
        "test".into(),
        "resp-test".into(),
        vec![],
        None,
        None,
    )
    .collect::<Vec<_>>()
    .await;
    assert_eq!(
        chunks.into_iter().map(Result::unwrap).collect::<Vec<_>>(),
        parts
    );
}

#[tokio::test]
async fn typed_replay_text_and_chunk_limits_are_protocol_errors_and_terminal() {
    for (byte_limit, text_limit, chunk_limit) in [(4, 1024, 10), (1024, 2, 10), (1024, 1024, 0)] {
        let (inner, state) = probe(
            vec![
                Ok::<_, Error>(Bytes::from_static(
                    b"data: {\"choices\":[{\"delta\":{\"content\":\"text\"}}]}\n\n",
                )),
                Ok(Bytes::from_static(b"data: must-not-read\n\n")),
            ],
            false,
        );
        let mut stream = wrap_streaming_tool_detection_with_limits_and_error(
            inner,
            "test".into(),
            "resp-test".into(),
            vec![],
            None,
            None,
            byte_limit,
            text_limit,
            chunk_limit,
        );
        let error = stream.next().await.unwrap().unwrap_err();
        assert!(matches!(error, StreamError::Protocol(_)));
        assert!(error
            .to_string()
            .contains("tool_injection_stream_too_large"));
        assert!(stream.next().await.is_none());
        assert_eq!(state.polls.load(Ordering::SeqCst), 1);
        assert_eq!(state.drops.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn typed_transport_failure_does_not_replay_captured_bytes() {
    let (inner, state) = probe(
        vec![
            Ok(Bytes::from_static(b"data: captured-but-not-replayed\n\n")),
            Err(Error::Transport(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "body deadline",
            ))),
            Ok(Bytes::from_static(b"data: must-not-read\n\n")),
        ],
        false,
    );
    let mut stream = wrap_streaming_tool_detection_with_error(
        inner,
        "test".into(),
        "resp-test".into(),
        vec![],
        None,
        None,
    );
    let error = stream.next().await.unwrap().unwrap_err();
    assert!(matches!(error, StreamError::Transport(_)));
    assert_eq!(error.to_string(), "body deadline");
    assert!(stream.next().await.is_none());
    assert_eq!(state.polls.load(Ordering::SeqCst), 2);
    assert_eq!(state.drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn typed_detector_cancellation_releases_pending_upstream() {
    let (inner, state) = probe::<Error>(vec![], true);
    let mut stream = wrap_streaming_tool_detection_with_error(
        inner,
        "test".into(),
        "resp-test".into(),
        vec![],
        None,
        None,
    );
    assert!(futures::poll!(stream.next()).is_pending());
    drop(stream);
    assert_eq!(state.drops.load(Ordering::SeqCst), 1);
}
