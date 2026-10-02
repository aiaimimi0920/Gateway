use super::*;
use crate::protocol::stream_error::StreamError;
use crate::protocol::stream_error_test_support::probe;
use futures::StreamExt;
use std::sync::atomic::Ordering;

type Error = StreamError<std::io::Error>;

#[tokio::test]
async fn typed_decoder_exact_limit_preserves_native_chat_bytes() {
    let wire = b"data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\n";
    let source = futures::stream::iter(vec![
        Ok::<_, Error>(Bytes::from_static(&wire[..wire.len() - 1])),
        Ok(Bytes::from_static(b"\n")),
    ]);
    let mut stream = Box::pin(translate_responses_sse_to_openai_chat_with_limit_and_error(
        source,
        "test".into(),
        wire.len(),
    ));
    assert_eq!(stream.next().await.unwrap().unwrap().as_ref(), wire);
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn typed_decoder_limit_is_protocol_error_without_later_poll() {
    let (source, state) = probe::<Error>(
        vec![
            Ok(Bytes::from_static(b"data: oversized\n\n")),
            Ok(Bytes::from_static(b"late")),
        ],
        false,
    );
    let mut stream = Box::pin(translate_responses_sse_to_openai_chat_with_limit_and_error(
        source,
        "test".into(),
        8,
    ));
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
async fn typed_transport_failure_preserves_partial_output_and_cancellation_drops() {
    let (source, state) = probe(
        vec![
            Ok(Bytes::from_static(
                b"event: response.output_text.delta\ndata: {\"delta\":\"partial\"}\n\n",
            )),
            Err(Error::Transport(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "body reset",
            ))),
            Ok(Bytes::from_static(b"data: [DONE]\n\n")),
        ],
        false,
    );
    let chunks = translate_responses_sse_to_openai_chat_with_error(source, "test".into())
        .collect::<Vec<_>>()
        .await;
    assert_eq!(chunks.len(), 2);
    assert!(String::from_utf8_lossy(chunks[0].as_ref().unwrap()).contains("partial"));
    assert!(
        matches!(&chunks[1], Err(Error::Transport(error)) if error.kind() == std::io::ErrorKind::BrokenPipe)
    );
    assert_eq!(state.polls.load(Ordering::SeqCst), 2);
    assert_eq!(state.drops.load(Ordering::SeqCst), 1);
    let (source, state) = probe::<Error>(vec![], true);
    let mut stream = Box::pin(translate_responses_sse_to_openai_chat_with_error(
        source,
        "test".into(),
    ));
    assert!(futures::poll!(stream.next()).is_pending());
    drop(stream);
    assert_eq!(state.drops.load(Ordering::SeqCst), 1);
}
