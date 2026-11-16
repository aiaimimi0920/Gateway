use super::fixture::{decode, output, sse};
use bytes::Bytes;
use futures::{stream, StreamExt};
use neuro_gateway::protocol::{
    bedrock_converse::translate_openai_sse_to_bedrock_eventstream_with_error,
    stream_error::StreamError,
};
use serde_json::json;
use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

struct DropSignal(Arc<AtomicBool>);
impl Drop for DropSignal {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}
type Error = StreamError<std::io::Error>;

fn complete() -> Vec<u8> {
    sse(&[
        json!({"choices":[{"delta":{"content":"text"}}]}),
        json!({"choices":[{"delta":{},"finish_reason":"stop"}],
            "usage":{"prompt_tokens":1,"completion_tokens":2,"total_tokens":3}}),
    ])
}

#[tokio::test]
async fn native_waiting_for_eof_never_emits_terminal_events_and_drop_releases_source() {
    let dropped = Arc::new(AtomicBool::new(false));
    let signal = DropSignal(dropped.clone());
    let mut first = Some(Bytes::from(complete()));
    let source = stream::poll_fn(move |_| {
        let _keep = &signal;
        first.take().map_or(std::task::Poll::Pending, |v| {
            std::task::Poll::Ready(Some(Ok::<_, Error>(v)))
        })
    });
    let mut translated = Box::pin(translate_openai_sse_to_bedrock_eventstream_with_error(
        source,
        "model".into(),
    ));
    for kind in ["messageStart", "contentBlockDelta"] {
        assert_eq!(decode(&translated.next().await.unwrap().unwrap()).0, kind);
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(25), translated.next())
            .await
            .is_err()
    );
    assert!(!dropped.load(Ordering::SeqCst));
    drop(translated);
    assert!(dropped.load(Ordering::SeqCst));
}

#[tokio::test]
async fn native_tail_transport_failure_is_typed_terminal_and_does_not_poll_later_input() {
    let dropped = Arc::new(AtomicBool::new(false));
    let polls = Arc::new(AtomicUsize::new(0));
    let signal = DropSignal(dropped.clone());
    let observed = polls.clone();
    let source = stream::iter([
        Ok(Bytes::from(complete())),
        Err(Error::Transport(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "tail failed",
        ))),
        Ok(Bytes::from_static(b"never")),
    ])
    .map(move |value| {
        let _keep = &signal;
        observed.fetch_add(1, Ordering::SeqCst);
        value
    });
    let mut translated = Box::pin(translate_openai_sse_to_bedrock_eventstream_with_error(
        source,
        "model".into(),
    ));
    assert_eq!(
        decode(&translated.next().await.unwrap().unwrap()).0,
        "messageStart"
    );
    assert_eq!(
        decode(&translated.next().await.unwrap().unwrap()).0,
        "contentBlockDelta"
    );
    assert!(
        matches!(translated.next().await.unwrap().unwrap_err(), Error::Transport(e) if e.kind() == std::io::ErrorKind::BrokenPipe)
    );
    assert!(dropped.load(Ordering::SeqCst));
    assert_eq!(polls.load(Ordering::SeqCst), 2);
    assert!(translated.next().await.is_none());
}

#[tokio::test]
async fn native_complete_eof_without_done_is_valid_and_sparse_usage_is_not_zeroed() {
    let mut wire = sse(&[
        json!({"choices":[{"delta":{"content":"text"}}],"usage":{"prompt_tokens":5}}),
        json!({"choices":[{"delta":{},"finish_reason":"length"}],"usage":{"completion_tokens":3}}),
        json!({"choices":[],"usage":{"total_tokens":8,"prompt_tokens_details":{"cached_tokens":2}}}),
        json!({"choices":[],"usage":{"cache_creation_input_tokens":1}}),
    ]);
    wire.truncate(wire.len() - b"data: [DONE]\n\n".len());
    let frames = output(&wire, 1).await;
    let events: Vec<_> = frames.iter().map(|v| decode(v.as_ref().unwrap())).collect();
    assert_eq!(
        events.iter().find(|v| v.0 == "messageStop").unwrap().1["stopReason"],
        "max_tokens"
    );
    assert_eq!(
        events.last().unwrap().1["usage"],
        json!({"inputTokens":5,"outputTokens":3,"totalTokens":8,
        "cacheReadInputTokens":2,"cacheWriteInputTokens":1})
    );
    assert_eq!(events.iter().filter(|v| v.0 == "metadata").count(), 1);
}

#[tokio::test]
async fn native_normalized_sse_accepts_lf_crlf_and_complete_heartbeat_control_lines() {
    for ending in ["\n", "\r\n"] {
        let mut wire = String::from_utf8(complete())
            .unwrap()
            .replace('\n', ending)
            .into_bytes();
        wire.extend_from_slice(
            format!(":heartbeat{ending}id: next{ending}retry: 10{ending}").as_bytes(),
        );
        let frames = output(&wire, 1).await;
        assert!(frames.iter().all(Result::is_ok));
        assert_eq!(
            decode(frames.last().unwrap().as_ref().unwrap()).0,
            "metadata"
        );
    }
}
