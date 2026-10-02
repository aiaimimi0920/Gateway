use super::*;
use crate::protocol::stream_error::StreamError;
use crate::protocol::stream_error_test_support::probe;
use futures::StreamExt;
use std::sync::atomic::Ordering;

type Error = StreamError<std::io::Error>;

#[tokio::test]
async fn typed_decoder_limit_is_protocol_error_and_terminates() {
    let (source, state) = probe::<Error>(
        vec![
            Ok(Bytes::from_static(b"data: oversized\n\n")),
            Ok(Bytes::from_static(b"late")),
        ],
        false,
    );
    let mut stream = Box::pin(
        translate_openai_chat_sse_to_legacy_completions_with_limit_and_error(
            source,
            "test".into(),
            8,
        ),
    );
    let error = stream.next().await.unwrap().unwrap_err();
    assert!(matches!(error, Error::Protocol(_)));
    assert_eq!(
        error.to_string(),
        "error decoding response body: translated SSE frame exceeded the 8-byte limit"
    );
    assert!(stream.next().await.is_none());
    assert_eq!(state.polls.load(Ordering::SeqCst), 1);
    assert_eq!(state.drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn typed_transport_failure_keeps_partial_text_without_done() {
    let (source, state) = probe(
        vec![
            Ok(Bytes::from_static(
                b"data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n",
            )),
            Err(Error::Transport(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "body reset",
            ))),
            Ok(Bytes::from_static(b"data: [DONE]\n\n")),
        ],
        false,
    );
    let chunks = translate_openai_chat_sse_to_legacy_completions_with_error(source, "test".into())
        .collect::<Vec<_>>()
        .await;
    assert_eq!(chunks.len(), 2);
    let text = String::from_utf8_lossy(chunks[0].as_ref().unwrap());
    assert!(text.contains("partial"));
    assert!(!text.contains("[DONE]"));
    assert!(
        matches!(&chunks[1], Err(Error::Transport(error)) if error.kind() == std::io::ErrorKind::BrokenPipe)
    );
    assert_eq!(state.polls.load(Ordering::SeqCst), 2);
    assert_eq!(state.drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn typed_pending_cancellation_releases_the_upstream() {
    let (source, state) = probe::<Error>(vec![], true);
    let mut stream = Box::pin(translate_openai_chat_sse_to_legacy_completions_with_error(
        source,
        "test".into(),
    ));
    assert!(futures::poll!(stream.next()).is_pending());
    drop(stream);
    assert_eq!(state.drops.load(Ordering::SeqCst), 1);
}
