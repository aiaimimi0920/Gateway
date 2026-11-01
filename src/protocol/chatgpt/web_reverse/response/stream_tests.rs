//! Streaming must preserve terminal failures instead of manufacturing a successful EOF.
use super::*;
use crate::protocol::stream_error::StreamError;
use futures::StreamExt;

const TEXT: &str = "data: {\"message\":{\"author\":{\"role\":\"assistant\"},\"content\":{\"parts\":[\"answer\"]}}}\n\n";

async fn render(body: String, chunk_size: usize) -> (String, Vec<String>) {
    let chunks = body
        .as_bytes()
        .chunks(chunk_size)
        .map(|chunk| Ok(Bytes::copy_from_slice(chunk)))
        .collect::<Vec<_>>();
    let results = translate_chatgpt_web_stream(futures::stream::iter(chunks), "exact-model".into())
        .collect::<Vec<_>>()
        .await;
    let mut text = String::new();
    let mut errors = Vec::new();
    for result in results {
        match result {
            Ok(bytes) => text.push_str(std::str::from_utf8(&bytes).unwrap()),
            Err(error) => {
                assert!(matches!(error, StreamError::Protocol(_)));
                errors.push(error.to_string());
            }
        }
    }
    (text, errors)
}

#[tokio::test]
async fn stream_terminal_failures_never_emit_success_or_done() {
    for (event, code) in [
        ("", "chatgpt_web_incomplete_stream"),
        ("data: failed\n\n", "chatgpt_web_generation_failed"),
        ("data: \"failed\"\n\n", "chatgpt_web_generation_failed"),
        (
            "data: {\"message\":{\"author\":{\"role\":\"assistant\"},\"status\":\"failed\"}}\n\n",
            "chatgpt_web_generation_failed",
        ),
        (
            "data: {\"p\":\"/message/status\",\"o\":\"replace\",\"v\":\"failed\"}\n\n",
            "chatgpt_web_generation_failed",
        ),
        (
            "data: {\"error\":{\"message\":\"private-upstream-marker\"}}\n\n",
            "chatgpt_web_generation_failed",
        ),
        (
            "data: {\"type\":\"moderation\",\"moderation_response\":{\"blocked\":true}}\n\n",
            "chatgpt_web_blocked",
        ),
    ] {
        let done = if event.is_empty() {
            ""
        } else {
            "data: [DONE]\n\n"
        };
        let (text, errors) = render(format!("{TEXT}{event}{done}"), 7).await;
        assert_eq!(errors.len(), 1, "{event:?}: {text}");
        assert!(errors[0].contains(code), "{errors:?}");
        assert!(!errors[0].contains("private-upstream-marker"));
        assert!(!text.contains("\"finish_reason\":\"stop\""), "{text}");
        assert!(!text.contains("data: [DONE]"), "{text}");
    }
}

#[tokio::test]
async fn stream_preserves_explicit_success_and_partial_completion() {
    for (event, reason) in [
        ("data: [DONE]\n\n", "stop"),
        ("data: finished_successfully\n\n", "stop"),
        ("data: \"finished_successfully\"\n\n", "stop"),
        ("data: finished_partial_completion\n\n", "length"),
        (
            "data: {\"p\":\"/message/status\",\"o\":\"replace\",\"v\":\"finished_partial_completion\"}\n\ndata: [DONE]\n\n",
            "length",
        ),
    ] {
        let (text, errors) = render(format!("{TEXT}{event}"), 11).await;
        assert!(errors.is_empty(), "{event:?}: {errors:?}");
        assert_eq!(text.matches(&format!("\"finish_reason\":\"{reason}\"")).count(), 1, "{text}");
        assert_eq!(text.matches("data: [DONE]").count(), 1, "{text}");
        assert!(text.contains("\"model\":\"exact-model\""));
        if reason == "length" {
            assert!(!text.contains("\"finish_reason\":\"stop\""));
        }
    }
}

#[tokio::test]
async fn stream_empty_or_unterminated_response_is_not_completed() {
    for body in ["", "data: [DONE]\n\n", "data: finished_successfully\n\n"] {
        let (text, errors) = render(body.into(), 3).await;
        assert_eq!(errors.len(), 1, "{body:?}: {text}");
        assert!(!text.contains("\"finish_reason\":\"stop\""));
        assert!(!text.contains("data: [DONE]"));
    }
    let (text, errors) = render(format!("{TEXT}data: [DONE]\n"), 3).await;
    assert_eq!(errors.len(), 1, "{text}");
    assert!(errors[0].contains("chatgpt_web_incomplete_stream"));
}

#[tokio::test]
async fn stream_decodes_fragmented_utf8_bom_crlf_and_stops_at_done() {
    let body = format!(
        "\u{feff}{}data: [DONE]\n\ndata: failed\n\n",
        TEXT.replace("answer", "巴黎")
    )
    .replace('\n', "\r\n");
    let (text, errors) = render(body, 1).await;
    assert!(errors.is_empty(), "{errors:?}");
    assert!(text.contains("巴黎"), "{text}");
    assert_eq!(text.matches("\"finish_reason\":\"stop\"").count(), 1);
    assert_eq!(text.matches("data: [DONE]").count(), 1);
}

#[tokio::test]
async fn stream_preserves_transport_failure_without_manufactured_completion() {
    let error = rquest::Client::new().get("http://[").build().unwrap_err();
    let source = futures::stream::iter(vec![Ok(Bytes::from_static(TEXT.as_bytes())), Err(error)]);
    let chunks = translate_chatgpt_web_stream(source, "exact-model".into())
        .collect::<Vec<_>>()
        .await;
    assert_eq!(chunks.len(), 2);
    assert!(matches!(&chunks[1], Err(StreamError::Transport(_))));
}

#[tokio::test]
async fn stream_is_lazy_and_releases_source_on_completion_or_cancellation() {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    struct Source {
        polls: Arc<AtomicUsize>,
        drops: Arc<AtomicUsize>,
    }
    impl futures::Stream for Source {
        type Item = Result<Bytes, rquest::Error>;
        fn poll_next(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Option<Self::Item>> {
            let count = self.polls.fetch_add(1, Ordering::SeqCst);
            std::task::Poll::Ready(Some(Ok(Bytes::from_static(if count == 0 {
                TEXT.as_bytes()
            } else {
                b"data: [DONE]\n\n"
            }))))
        }
    }
    impl Drop for Source {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }
    for complete in [false, true] {
        let polls = Arc::new(AtomicUsize::new(0));
        let drops = Arc::new(AtomicUsize::new(0));
        let mut output = Box::pin(translate_chatgpt_web_stream(
            Source {
                polls: polls.clone(),
                drops: drops.clone(),
            },
            "exact-model".into(),
        ));
        assert_eq!(polls.load(Ordering::SeqCst), 0);
        assert!(output.next().await.unwrap().is_ok());
        assert_eq!(polls.load(Ordering::SeqCst), 1);
        if complete {
            assert!(output.next().await.unwrap().is_ok());
            assert_eq!(
                drops.load(Ordering::SeqCst),
                1,
                "release before reporting completion"
            );
            assert_eq!(
                output.next().await.unwrap().unwrap(),
                Bytes::from_static(b"data: [DONE]\n\n")
            );
            assert!(output.next().await.is_none());
            assert_eq!(
                polls.load(Ordering::SeqCst),
                2,
                "never drain a completed source"
            );
        }
        drop(output);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }
}
