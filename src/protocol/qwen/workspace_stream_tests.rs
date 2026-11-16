use super::*;
use futures::{stream, StreamExt};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

fn event(value: Value) -> String {
    format!("data: {value}\n\n")
}
fn delta(text: &str, finish: Value) -> String {
    event(json!({"choices":[{"index":0,"delta":{"content":text},"finish_reason":finish}]}))
}
fn usage() -> String {
    event(
        json!({"choices":[],"usage":{"prompt_tokens":8,"completion_tokens":2,"total_tokens":10,
        "prompt_tokens_details":{"cached_tokens":3}}}),
    )
}
async fn collect(input: Vec<u8>) -> (String, Vec<String>) {
    let chunks = input
        .chunks(1)
        .map(|v| Ok::<_, ProtocolStreamError>(Bytes::copy_from_slice(v)))
        .collect::<Vec<_>>();
    let values = super::require_completion(stream::iter(chunks), invalid)
        .collect::<Vec<_>>()
        .await;
    let mut bytes = Vec::new();
    let mut errors = Vec::new();
    for value in values {
        match value {
            Ok(value) => bytes.extend_from_slice(&value),
            Err(e) => errors.push(e.to_string()),
        }
    }
    (String::from_utf8(bytes).unwrap(), errors)
}
#[tokio::test]
async fn strict_fragments_crlf_bom_and_cache_usage_are_preserved() {
    let body = format!(
        "\u{feff}: ping\n\n{}{}{}data: [DONE]\n\n",
        delta("你好", Value::Null),
        delta("", json!("stop")),
        usage()
    )
    .replace('\n', "\r\n");
    let (body, errors) = collect(body.into_bytes()).await;
    assert!(errors.is_empty(), "{errors:?}");
    assert!(body.contains("你好"));
    assert!(body.contains("\"cached_tokens\":3"));
    assert!(body.ends_with("data: [DONE]\n\n"));
}
#[tokio::test]
async fn eof_done_and_bad_tail_never_release_success_tail() {
    let prefix = delta("VISIBLE", Value::Null);
    for tail in [
        String::new(),
        "data: [DONE]\n\n".into(),
        delta("", json!("stop")),
        format!(
            "{}{}data: [DONE]\n\ndata: {{\"error\":\"private-detail\"}}\n\n",
            delta("", json!("stop")),
            usage()
        ),
        format!("{}data: [DONE]\n\ndata: {{", delta("", json!("stop"))),
        "data: not-json-private-detail\n\n".into(),
        "event: error\ndata: private-detail\n\n".into(),
    ] {
        let (body, errors) = collect(format!("{prefix}{tail}").into_bytes()).await;
        assert!(body.contains("VISIBLE"));
        assert_eq!(errors.len(), 1, "{body}");
        assert!(!body.contains("[DONE]"));
        assert!(!body.contains("\"finish_reason\":\"stop\""));
        assert!(!errors[0].contains("private-detail"));
    }
}
#[tokio::test]
async fn finish_with_last_tool_fragment_is_live_before_terminal_commit() {
    let tool = event(
        json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call-1","type":"function", "function":{"name":"lookup","arguments":"{}"}}]},"finish_reason":"tool_calls"}]}),
    );
    let (body, errors) = collect(format!("{tool}{}data: [DONE]\n\n", usage()).into_bytes()).await;
    assert!(errors.is_empty(), "{errors:?}");
    let frames: Vec<_> = body.split("\n\n").collect();
    assert!(frames[0].contains("lookup"));
    assert!(frames[0].contains("\"finish_reason\":null"));
    assert!(frames[1].contains("\"finish_reason\":\"tool_calls\""));
    for reason in ["length", "content_filter"] {
        let (_, errors) =
            collect(format!("{}data: [DONE]\n\n", delta("", json!(reason))).into_bytes()).await;
        assert!(errors.is_empty(), "{errors:?}");
    }
}
#[tokio::test]
async fn invalid_utf8_and_incomplete_frame_after_done_fail_closed() {
    for suffix in [vec![0xff, b'\n', b'\n'], b"event: unfinished".to_vec()] {
        let mut input = format!("{}data: [DONE]\n\n", delta("", json!("stop"))).into_bytes();
        input.extend(suffix);
        let (body, errors) = collect(input).await;
        assert_eq!(errors.len(), 1);
        assert!(!body.contains("[DONE]"));
    }
}
#[tokio::test]
async fn frame_and_tail_byte_and_object_budgets_are_enforced() {
    for (frame_limit, tail_limit, input) in [
        (32, 1024, delta("too long", Value::Null)),
        (1024, 16, delta("", json!("stop"))),
    ] {
        let mut guard = Guard {
            inner: Some(Box::pin(stream::iter([Ok::<_, ProtocolStreamError>(
                Bytes::from(input),
            )]))),
            decoder: BoundedSseDecoder::new_strict(frame_limit),
            admission: Admission::default(),
            held: VecDeque::new(),
            held_bytes: 0,
            eof: false,
            limit: tail_limit,
            error: invalid,
        };
        assert!(guard.next().await.unwrap().is_err());
        assert!(guard.inner.is_none());
        assert!(guard.held.is_empty());
        assert!(guard.next().await.is_none());
    }
    let mut guard = Guard::<ProtocolStreamError> {
        inner: None,
        decoder: BoundedSseDecoder::new_strict(1024),
        admission: Admission::default(),
        held: VecDeque::new(),
        held_bytes: 0,
        eof: false,
        limit: 1024,
        error: invalid,
    };
    for _ in 0..256 {
        guard.retain(Bytes::from_static(b"x")).unwrap();
    }
    assert!(guard.retain(Bytes::from_static(b"x")).is_err());
}
struct DropStream(Arc<AtomicUsize>);
impl Stream for DropStream {
    type Item = Result<Bytes, ProtocolStreamError>;
    fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Pending
    }
}
impl Drop for DropStream {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
#[tokio::test]
async fn cancellation_drops_transport_without_a_drain_task_or_terminal_flush() {
    let drops = Arc::new(AtomicUsize::new(0));
    let prefix = format!(
        "{}{}data: [DONE]\n\n",
        delta("VISIBLE", json!("stop")),
        usage()
    );
    let input = stream::iter([Ok(Bytes::from(prefix))]).chain(DropStream(drops.clone()));
    let mut guard = Box::pin(super::require_completion(input, invalid));
    assert!(
        String::from_utf8(guard.next().await.unwrap().unwrap().to_vec())
            .unwrap()
            .contains("VISIBLE")
    );
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(20), guard.next())
            .await
            .is_err()
    );
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    drop(guard);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn transport_error_after_done_is_not_replaced_or_committed() {
    let input = stream::iter([
        Ok(Bytes::from(format!(
            "{}data: [DONE]\n\n",
            delta("", json!("stop"))
        ))),
        Err(std::io::Error::other("transport sentinel")),
    ]);
    // A real transport error remains the original generic error, not a fabricated HTTP-client error.
    let input = input.map(|r| r.map_err(|e| ProtocolStreamError::invalid_data(e.to_string())));
    let values = super::require_completion(input, invalid)
        .collect::<Vec<_>>()
        .await;
    assert_eq!(values.iter().filter(|v| v.is_err()).count(), 1);
    assert!(values
        .last()
        .unwrap()
        .as_ref()
        .unwrap_err()
        .to_string()
        .contains("transport sentinel"));
    assert!(values
        .iter()
        .filter_map(|v| v.as_ref().ok())
        .all(|v| !std::str::from_utf8(v).unwrap().contains("[DONE]")));
}
