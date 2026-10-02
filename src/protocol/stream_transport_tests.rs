//! 非标准下游编码仍保留传输错误，协议错误不伪装成 HTTP 失败。
use super::stream_error::{ProtocolStreamError, StreamError};
use super::stream_error_test_support::{http_body_error, probe, source_messages};
use super::{bedrock_converse, cohere, kiro, openai};
use crate::upstream::stream::TrackedStream;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use std::pin::Pin;
use std::sync::atomic::Ordering;

type Error = StreamError<std::io::Error>;
type ByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, Error>> + Send>>;

#[tokio::test]
async fn endpoint_encoders_preserve_typed_transport_errors_until_tracking() {
    for encoder in 0..4 {
        if encoder >= 2 && !cfg!(feature = "line-kiro-official-vendor-api") {
            continue;
        }
        let (source, state) = probe(
            vec![
                Err(Error::Transport(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "wire failed",
                ))),
                Ok(Bytes::from_static(b"late")),
            ],
            false,
        );
        let req = openai::normalize_chat_completions(serde_json::json!({
            "model": "test", "messages": [{"role": "user", "content": "hello"}]
        }))
        .unwrap();
        let translated: ByteStream = match encoder {
            0 => Box::pin(
                bedrock_converse::translate_openai_sse_to_bedrock_eventstream_with_error(
                    source,
                    "test".into(),
                ),
            ),
            1 => Box::pin(cohere::translate_openai_sse_to_cohere_stream_with_error(
                source,
                "test".into(),
            )),
            2 => Box::pin(kiro::translate_kiro_event_stream_to_openai_sse_with_error(
                source,
                "test".into(),
                req,
            )),
            _ => Box::pin(
                kiro::translate_kiro_event_stream_to_anthropic_sse_with_error(
                    source,
                    "test".into(),
                    req,
                ),
            ),
        };
        let mut tracked = TrackedStream::new_with_error(translated, |_, success| assert!(!success));
        let error = tracked.next().await.unwrap().unwrap_err();
        assert!(
            matches!(error, Error::Transport(ref value) if value.kind() == std::io::ErrorKind::BrokenPipe)
        );
        assert_eq!(error.to_string(), "wire failed");
        assert!(tracked.next().await.is_none());
        assert_eq!(state.polls.load(Ordering::SeqCst), 1);
        assert_eq!(state.drops.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn bedrock_local_state_failure_is_protocol_and_drops_before_delivery() {
    let (source, state) = probe::<Error>(vec![Ok(Bytes::from_static(
        b"data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":18446744073709551615,\"function\":{\"name\":\"test\"}}]}}]}\n\n"
    )), Ok(Bytes::from_static(b"late"))], false);
    let mut translated = Box::pin(
        bedrock_converse::translate_openai_sse_to_bedrock_eventstream_with_error(
            source,
            "test".into(),
        ),
    );
    let error = translated.next().await.unwrap().unwrap_err();
    assert!(matches!(error, Error::Protocol(_)));
    assert!(error
        .to_string()
        .contains("Bedrock tool index does not fit"));
    assert_eq!(state.drops.load(Ordering::SeqCst), 1);
    assert!(translated.next().await.is_none());
    assert_eq!(state.polls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn raw_transport_adapter_keeps_the_original_http_source_and_flags() {
    use std::error::Error as _;
    let original = http_body_error("legacy source").await;
    let source = original.source().unwrap() as *const dyn std::error::Error as *const ();
    let expected = (
        original.to_string(),
        original.is_timeout(),
        original.is_connect(),
        original.is_decode(),
    );
    let mut adapted = Box::pin(super::stream_error_legacy::with_transport_error(
        futures::stream::iter([Err::<Bytes, _>(original)]),
    ));
    let typed = adapted.next().await.unwrap().unwrap_err();
    let StreamError::Transport(recovered) = typed else {
        panic!("expected unchanged transport")
    };
    assert_eq!(
        recovered.source().unwrap() as *const dyn std::error::Error as *const (),
        source
    );
    assert_eq!(
        (
            recovered.to_string(),
            recovered.is_timeout(),
            recovered.is_connect(),
            recovered.is_decode()
        ),
        expected
    );
    assert!(source_messages(&recovered).contains("legacy source"));
    assert!(adapted.next().await.is_none());
}

#[test]
fn typed_protocol_boundary_preserves_decode_text_without_a_client_error() {
    let original = ProtocolStreamError::invalid_data("local limit");
    let expected = original.to_string();
    let recovered = StreamError::<rquest::Error>::Protocol(original);
    assert_eq!(recovered.to_string(), expected);
    assert!(matches!(recovered, StreamError::Protocol(_)));
}
