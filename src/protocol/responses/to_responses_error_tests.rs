use super::*;
use std::sync::atomic::Ordering;

use crate::protocol::stream_error::StreamError;
use crate::protocol::stream_error_test_support::probe;
use futures::StreamExt;

type Error = StreamError<std::io::Error>;

#[tokio::test]
async fn typed_exact_frame_limit_and_fragmented_input_preserve_bytes() {
    let wire = b"event: response.created\ndata: {}\n\n";
    let inner = futures::stream::iter(vec![
        Ok::<_, Error>(Bytes::from_static(b"event: response.created\ndata: {}\n")),
        Ok(Bytes::from_static(b"\n")),
    ]);
    let chunks =
        translate_openai_sse_to_responses_with_limit_and_error(inner, "test".into(), wire.len())
            .collect::<Vec<_>>()
            .await;
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks.into_iter().next().unwrap().unwrap().as_ref(), wire);
}

#[tokio::test]
async fn typed_frame_limit_is_protocol_terminal_and_stops_upstream_polling() {
    let (inner, state) = probe(
        vec![
            Ok::<_, Error>(Bytes::from_static(b"data: too-large\n\n")),
            Ok(Bytes::from_static(b"data: must-not-read\n\n")),
        ],
        false,
    );
    let mut stream = Box::pin(translate_openai_sse_to_responses_with_limit_and_error(
        inner,
        "test".into(),
        8,
    ));
    let error = stream.next().await.unwrap().unwrap_err();
    assert!(matches!(error, StreamError::Protocol(_)));
    assert_eq!(
        error.to_string(),
        "error decoding response body: translated SSE frame exceeded the 8-byte limit"
    );
    assert!(stream.next().await.is_none());
    assert_eq!(state.polls.load(Ordering::SeqCst), 1);
    assert_eq!(state.drops.load(Ordering::SeqCst), 1);
    drop(stream);
    assert_eq!(state.drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn typed_transport_error_after_partial_output_does_not_synthesize_completion() {
    let (inner, state) = probe(
        vec![
            Ok(Bytes::from_static(
                b"data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n",
            )),
            Err(Error::Transport(std::io::Error::new(
                std::io::ErrorKind::ConnectionReset,
                "body reset",
            ))),
            Ok(Bytes::from_static(b"data: [DONE]\n\n")),
        ],
        false,
    );
    let mut stream = Box::pin(translate_openai_sse_to_responses_with_error(
        inner,
        "test".into(),
    ));
    let mut output = String::new();
    loop {
        match stream.next().await.unwrap() {
            Ok(bytes) => output.push_str(&String::from_utf8(bytes.to_vec()).unwrap()),
            Err(error) => {
                assert!(matches!(error, Error::Transport(_)));
                assert_eq!(error.to_string(), "body reset");
                break;
            }
        }
    }
    assert!(output.contains("response.output_text.delta"));
    assert!(!output.contains("response.completed"));
    assert!(stream.next().await.is_none());
    assert_eq!(state.polls.load(Ordering::SeqCst), 2);
    assert_eq!(state.drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn typed_cancellation_drops_pending_upstream() {
    let (inner, state) = probe::<Error>(vec![], true);
    let mut stream = Box::pin(translate_openai_sse_to_responses_with_error(
        inner,
        "test".into(),
    ));
    assert!(futures::poll!(stream.next()).is_pending());
    drop(stream);
    assert_eq!(state.polls.load(Ordering::SeqCst), 1);
    assert_eq!(state.drops.load(Ordering::SeqCst), 1);
}
