use super::*;
use futures::StreamExt;
use std::sync::{Arc, Mutex};

type MetricsCap = Arc<Mutex<Option<(StreamMetrics, bool)>>>;

fn make_bytes_stream(
    chunks: Vec<Result<Bytes, rquest::Error>>,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    futures::stream::iter(chunks)
}

fn capture_callback() -> (
    MetricsCap,
    impl FnOnce(StreamMetrics, bool) + Send + 'static,
) {
    let cap: MetricsCap = Arc::new(Mutex::new(None));
    let cap2 = Arc::clone(&cap);
    let cb = move |m: StreamMetrics, success: bool| {
        *cap2.lock().unwrap() = Some((m, success));
    };
    (cap, cb)
}

// ── basic metrics ─────────────────────────────────────────────────────

#[tokio::test]
async fn tracks_chunks_and_bytes() {
    let chunks = vec![Ok(Bytes::from("hello")), Ok(Bytes::from(" world"))];
    let (cap, cb) = capture_callback();
    let mut stream = TrackedStream::new(make_bytes_stream(chunks), cb);

    while stream.next().await.is_some() {}

    let (m, success) = cap.lock().unwrap().clone().unwrap();
    assert_eq!(m.chunk_count, 2);
    assert_eq!(m.total_bytes, 11); // "hello" (5) + " world" (6)
    assert!(success);
}

#[tokio::test]
async fn records_first_token_latency() {
    let chunks = vec![Ok(Bytes::from("first"))];
    let (cap, cb) = capture_callback();
    let mut stream = TrackedStream::new(make_bytes_stream(chunks), cb);

    while stream.next().await.is_some() {}

    let (m, _) = cap.lock().unwrap().clone().unwrap();
    assert!(m.first_token_latency_ms.is_some());
}

#[tokio::test]
async fn started_at_includes_elapsed_before_stream_construction() {
    let started_at = Instant::now();
    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    let chunks = vec![Ok(Bytes::from("first"))];
    let (cap, cb) = capture_callback();
    let mut stream = TrackedStream::new_with_started_at(make_bytes_stream(chunks), started_at, cb);

    while stream.next().await.is_some() {}

    let (metrics, _) = cap.lock().unwrap().clone().unwrap();
    assert!(metrics.first_token_latency_ms.unwrap_or_default() >= 20);
    assert!(metrics.total_duration_ms >= 20);
}

#[tokio::test]
async fn no_first_token_latency_when_empty_stream() {
    let chunks: Vec<Result<Bytes, rquest::Error>> = vec![];
    let (cap, cb) = capture_callback();
    let mut stream = TrackedStream::new(make_bytes_stream(chunks), cb);

    while stream.next().await.is_some() {}

    let (m, _) = cap.lock().unwrap().clone().unwrap();
    assert_eq!(m.first_token_latency_ms, None);
    assert_eq!(m.chunk_count, 0);
}

#[tokio::test]
async fn callback_called_with_success_true_on_clean_end() {
    let chunks = vec![Ok(Bytes::from("data"))];
    let (cap, cb) = capture_callback();
    let mut stream = TrackedStream::new(make_bytes_stream(chunks), cb);

    while stream.next().await.is_some() {}

    let (_, success) = cap.lock().unwrap().clone().unwrap();
    assert!(success);
}

// ── Drop behaviour ────────────────────────────────────────────────────

#[tokio::test]
async fn callback_fires_on_drop_with_false() {
    let chunks = vec![
        Ok(Bytes::from("chunk1")),
        Ok(Bytes::from("chunk2")),
        Ok(Bytes::from("chunk3")),
    ];
    let (cap, cb) = capture_callback();

    {
        let mut stream = TrackedStream::new(make_bytes_stream(chunks), cb);
        // Read only the first chunk then drop.
        let _ = stream.next().await;
        // stream dropped here
    }

    let result = cap.lock().unwrap().clone();
    assert!(result.is_some(), "callback should have fired on drop");
    let (m, success) = result.unwrap();
    assert!(!success, "success should be false when dropped early");
    assert_eq!(m.chunk_count, 1);
}

// ── callback fires exactly once ────────────────────────────────────────

#[tokio::test]
async fn callback_fires_only_once() {
    let fire_count = Arc::new(Mutex::new(0u32));
    let fc2 = Arc::clone(&fire_count);

    let chunks = vec![Ok(Bytes::from("x"))];
    let stream = TrackedStream::new(make_bytes_stream(chunks), move |_, _| {
        *fc2.lock().unwrap() += 1;
    });

    // Fully consume then drop.
    let mut pinned = Box::pin(stream);
    while pinned.next().await.is_some() {}
    drop(pinned);

    assert_eq!(*fire_count.lock().unwrap(), 1);
}

// ── metrics snapshot ──────────────────────────────────────────────────

#[test]
fn metrics_snapshot_reflects_no_data() {
    let stream = TrackedStream::new(make_bytes_stream(vec![]), |_, _| {});
    let m = stream.metrics();
    assert_eq!(m.chunk_count, 0);
    assert_eq!(m.total_bytes, 0);
    assert_eq!(m.first_token_latency_ms, None);
}

#[tokio::test]
async fn tap_sse_usage_reads_openai_final_usage_chunk() {
    let chunks = vec![
        Ok(Bytes::from_static(
            br#"data: {"choices":[{"delta":{"content":"Hi"},"finish_reason":null}]}

"#,
        )),
        Ok(Bytes::from_static(
            br#"data: {"choices":[{"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":11,"completion_tokens":7,"total_tokens":18}}

"#,
        )),
        Ok(Bytes::from_static(b"data: [DONE]\n\n")),
    ];

    let (mut stream, usage) = tap_sse_usage(make_bytes_stream(chunks));
    while stream.next().await.is_some() {}

    let usage = snapshot_tapped_usage(&usage).unwrap();
    assert_eq!(usage.prompt_tokens, 11);
    assert_eq!(usage.completion_tokens, 7);
    assert_eq!(usage.total_tokens, 18);
}

#[tokio::test]
async fn tap_sse_usage_combines_anthropic_message_start_and_delta() {
    let chunks = vec![
        Ok(Bytes::from_static(
            br#"event: message_start
data: {"type":"message_start","message":{"usage":{"input_tokens":13,"output_tokens":0}}}

"#,
        )),
        Ok(Bytes::from_static(
            br#"event: message_delta
data: {"type":"message_delta","usage":{"output_tokens":9,"cache_read_input_tokens":4}}

"#,
        )),
    ];

    let (mut stream, usage) = tap_sse_usage(make_bytes_stream(chunks));
    while stream.next().await.is_some() {}

    let usage = snapshot_tapped_usage(&usage).unwrap();
    assert_eq!(usage.prompt_tokens, 13);
    assert_eq!(usage.completion_tokens, 9);
    assert_eq!(usage.total_tokens, 22);
    assert_eq!(usage.cache_read_input_tokens, Some(4));
}

#[tokio::test]
async fn tap_sse_usage_reads_nested_responses_completed_usage() {
    let chunks = vec![Ok(Bytes::from_static(
        br#"event: response.completed
data: {"type":"response.completed","response":{"id":"resp_1","usage":{"input_tokens":17,"output_tokens":9,"total_tokens":26,"input_tokens_details":{"cached_tokens":5}}}}

"#,
    ))];

    let (mut stream, usage) = tap_sse_usage(make_bytes_stream(chunks));
    while stream.next().await.is_some() {}

    let usage = snapshot_tapped_usage(&usage).unwrap();
    assert_eq!(usage.prompt_tokens, 17);
    assert_eq!(usage.completion_tokens, 9);
    assert_eq!(usage.total_tokens, 26);
    assert_eq!(usage.cache_read_input_tokens, Some(5));
}

#[tokio::test]
async fn tap_sse_completion_semantics_reads_openai_finish_reason() {
    let chunks = vec![
        Ok(Bytes::from_static(
            br#"data: {"choices":[{"delta":{"content":"Hi"},"finish_reason":null}]}

"#,
        )),
        Ok(Bytes::from_static(
            br#"data: {"choices":[{"delta":{},"finish_reason":"tool_calls"}]}

"#,
        )),
        Ok(Bytes::from_static(b"data: [DONE]\n\n")),
    ];

    let (mut stream, semantics) = tap_sse_completion_semantics(make_bytes_stream(chunks));
    while stream.next().await.is_some() {}

    assert_eq!(
        snapshot_tapped_completion_semantics(&semantics).as_deref(),
        Some("tool_calls")
    );
}

#[tokio::test]
async fn tap_sse_completion_semantics_reads_anthropic_stop_reason() {
    let chunks = vec![Ok(Bytes::from_static(
        br#"event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"tool_use"}}

"#,
    ))];

    let (mut stream, semantics) = tap_sse_completion_semantics(make_bytes_stream(chunks));
    while stream.next().await.is_some() {}

    assert_eq!(
        snapshot_tapped_completion_semantics(&semantics).as_deref(),
        Some("tool_calls")
    );
}

#[tokio::test]
async fn tap_sse_completion_semantics_reads_responses_completed_status() {
    let chunks = vec![Ok(Bytes::from_static(
        br#"event: response.completed
data: {"type":"response.completed","response":{"id":"resp_1","status":"completed","output":[{"type":"function_call","call_id":"call_1","name":"weather","arguments":"{}"}]}}

"#,
    ))];

    let (mut stream, semantics) = tap_sse_completion_semantics(make_bytes_stream(chunks));
    while stream.next().await.is_some() {}

    assert_eq!(
        snapshot_tapped_completion_semantics(&semantics).as_deref(),
        Some("tool_calls")
    );
}
