use super::*;
use crate::protocol::stream_error::StreamError;
use crate::protocol::stream_error_test_support::{
    http_body_error as synthetic_stream_error, source_messages,
};

fn wrap_with_test_limits(
    chunks: Vec<Result<Bytes, rquest::Error>>,
    max_original_bytes: usize,
    max_accumulated_text_bytes: usize,
    max_chunks: usize,
) -> std::pin::Pin<
    Box<dyn futures::Stream<Item = Result<Bytes, StreamError<rquest::Error>>> + Send + 'static>,
> {
    wrap_streaming_tool_detection_with_limits(
        futures::stream::iter(chunks),
        "deepseek-chat".to_string(),
        "resp-test".to_string(),
        vec![],
        None,
        None,
        max_original_bytes,
        max_accumulated_text_bytes,
        max_chunks,
    )
}

// ── Streaming tool call detection ──────────────────────────────────

#[test]
fn has_tool_call_opening_detects_tags() {
    assert!(has_tool_call_opening("text before <tool_calls> text after"));
    assert!(has_tool_call_opening("<function_calls>"));
    assert!(has_tool_call_opening("<invoke name=\"test\">"));
    assert!(!has_tool_call_opening("no tool calls here"));
    assert!(!has_tool_call_opening(""));
}

#[test]
fn extract_sse_content_basic() {
    let line = r#"data: {"choices":[{"delta":{"content":"hello"}}]}"#;
    assert_eq!(extract_sse_content(line), Some("hello".to_string()));
}

#[test]
fn extract_sse_content_done() {
    assert_eq!(extract_sse_content("data: [DONE]"), None);
}

#[test]
fn extract_sse_content_no_content_field() {
    let line = r#"data: {"choices":[{"delta":{"role":"assistant"}}]}"#;
    assert_eq!(extract_sse_content(line), None);
}

#[test]
fn extract_sse_content_non_data_line() {
    assert_eq!(extract_sse_content(": ping"), None);
    assert_eq!(extract_sse_content(""), None);
}

#[test]
fn build_tool_calls_chunk_format() {
    let calls = vec![CanonicalToolCall {
        id: Some("call_abc".to_string()),
        call_type: "function".to_string(),
        name: Some("read_file".to_string()),
        arguments: Some("{\"path\":\"/tmp/x\"}".to_string()),
        raw: HashMap::new(),
    }];

    let chunk_bytes = build_tool_calls_sse_chunk(&calls, "test-model", "resp-1");
    let chunk_str = String::from_utf8(chunk_bytes).unwrap();
    assert!(chunk_str.starts_with("data: "));
    assert!(chunk_str.ends_with("\n\n"));

    let json_str = chunk_str.strip_prefix("data: ").unwrap().trim();
    let parsed: Value = serde_json::from_str(json_str).unwrap();
    assert_eq!(parsed["id"], "resp-1");
    assert_eq!(parsed["model"], "test-model");
    assert_eq!(parsed["choices"][0]["finish_reason"], "tool_calls");
    let tc = &parsed["choices"][0]["delta"]["tool_calls"][0];
    assert_eq!(tc["id"], "call_abc");
    assert_eq!(tc["function"]["name"], "read_file");
    assert_eq!(tc["function"]["arguments"], "{\"path\":\"/tmp/x\"}");
}

#[tokio::test]
async fn streaming_detection_with_tool_calls() {
    use futures::StreamExt;

    // Simulate SSE chunks containing XML tool calls in content.
    let sse_events = vec![
        format!(
            "data: {}\n\n",
            json!({
                "choices": [{"delta": {"content": "<tool_calls>\n<tool_call>\n"}}]
            })
        ),
        format!(
            "data: {}\n\n",
            json!({
                "choices": [{"delta": {"content": "<tool_name>read_file</tool_name>\n"}}]
            })
        ),
        format!(
            "data: {}\n\n",
            json!({
                "choices": [{"delta": {"content": "<parameters>{\"path\":\"/tmp/test.txt\"}</parameters>\n"}}]
            })
        ),
        format!(
            "data: {}\n\n",
            json!({
                "choices": [{"delta": {"content": "</tool_call>\n</tool_calls>"}}]
            })
        ),
        "data: [DONE]\n\n".to_string(),
    ];

    let chunks: Vec<Result<Bytes, rquest::Error>> =
        sse_events.into_iter().map(|s| Ok(Bytes::from(s))).collect();

    let inner = futures::stream::iter(chunks);
    let mut wrapped = wrap_streaming_tool_detection(
        inner,
        "deepseek-chat".to_string(),
        "resp-test".to_string(),
        vec![],
        None,
        None,
    );

    let mut collected: Vec<String> = Vec::new();
    while let Some(Ok(bytes)) = wrapped.next().await {
        collected.push(String::from_utf8(bytes.to_vec()).unwrap());
    }

    // Should emit tool_calls chunk + [DONE], NOT the original text chunks.
    assert_eq!(
        collected.len(),
        2,
        "expected 2 chunks: tool_calls + DONE, got: {collected:?}"
    );

    // First chunk should be tool_calls.
    let json_str = collected[0].strip_prefix("data: ").unwrap().trim();
    let parsed: Value = serde_json::from_str(json_str).unwrap();
    assert_eq!(parsed["choices"][0]["finish_reason"], "tool_calls");
    let tc = &parsed["choices"][0]["delta"]["tool_calls"][0];
    assert_eq!(tc["function"]["name"], "read_file");

    // Second chunk should be [DONE].
    assert_eq!(collected[1], "data: [DONE]\n\n");
}

#[tokio::test]
async fn streaming_detection_handles_sse_line_split_across_chunks() {
    use futures::StreamExt;

    let content = concat!(
        "<tool_calls><tool_call><tool_name>read_file</tool_name>",
        "<parameters>{\"path\":\"/tmp/test.txt\"}</parameters>",
        "</tool_call></tool_calls>"
    );
    let event = format!(
        "data: {}\n\ndata: [DONE]\n\n",
        json!({"choices": [{"delta": {"content": content}}]})
    );
    let split_at = event
        .find("<tool_name>")
        .expect("fixture should contain an ASCII split point");
    let chunks = vec![
        Ok(Bytes::copy_from_slice(&event.as_bytes()[..split_at])),
        Ok(Bytes::copy_from_slice(&event.as_bytes()[split_at..])),
    ];
    let mut wrapped = wrap_with_test_limits(chunks, event.len(), content.len(), 2);

    let first = wrapped
        .next()
        .await
        .expect("tool call chunk should be emitted")
        .expect("split valid SSE line should parse");
    let first = std::str::from_utf8(&first).expect("tool call chunk should be UTF-8");
    let payload: Value = serde_json::from_str(
        first
            .strip_prefix("data: ")
            .expect("tool call chunk should use data prefix")
            .trim(),
    )
    .expect("tool call chunk should contain JSON");
    assert_eq!(
        payload["choices"][0]["delta"]["tool_calls"][0]["function"]["name"],
        "read_file"
    );
    assert_eq!(
        wrapped
            .next()
            .await
            .expect("DONE should be emitted")
            .unwrap(),
        Bytes::from_static(b"data: [DONE]\n\n")
    );
    assert!(wrapped.next().await.is_none());
}

#[tokio::test]
async fn streaming_detection_without_tool_calls() {
    use futures::StreamExt;

    // Regular text response — no tool calls.
    let sse_events = vec![
        format!(
            "data: {}\n\n",
            json!({"choices": [{"delta": {"content": "Hello, "}}]})
        ),
        format!(
            "data: {}\n\n",
            json!({"choices": [{"delta": {"content": "world!"}}]})
        ),
        "data: [DONE]\n\n".to_string(),
    ];

    let original: Vec<Bytes> = sse_events.into_iter().map(Bytes::from).collect();
    let chunks: Vec<Result<Bytes, rquest::Error>> = original.iter().cloned().map(Ok).collect();

    let inner = futures::stream::iter(chunks);
    let mut wrapped = wrap_streaming_tool_detection(
        inner,
        "deepseek-chat".to_string(),
        "resp-test".to_string(),
        vec![],
        None,
        None,
    );

    let mut collected: Vec<Bytes> = Vec::new();
    while let Some(Ok(bytes)) = wrapped.next().await {
        collected.push(bytes);
    }

    // Should replay original chunks unchanged.
    assert_eq!(collected, original);
}

#[tokio::test]
async fn streaming_detection_accepts_exact_replay_limits() {
    use futures::StreamExt;

    let original = vec![Bytes::from_static(b"a"), Bytes::from_static(b"bc")];
    let chunks = original.iter().cloned().map(Ok).collect();
    let collected: Vec<_> = wrap_with_test_limits(chunks, 3, 0, 2)
        .map(|item| item.expect("exact limits should be accepted"))
        .collect()
        .await;

    assert_eq!(collected, original);
}

#[tokio::test]
async fn streaming_detection_rejects_replay_byte_overflow_once() {
    use futures::StreamExt;

    let chunks = vec![
        Ok(Bytes::from_static(b"1234")),
        Ok(Bytes::from_static(b"5")),
    ];
    let mut wrapped = wrap_with_test_limits(chunks, 4, 64, 2);

    let error = wrapped
        .next()
        .await
        .expect("overflow should emit an error")
        .expect_err("overflow must not replay partial data");
    assert!(error
        .to_string()
        .contains("tool_injection_stream_too_large"));
    assert!(error
        .to_string()
        .contains("4-byte tool detection replay limit"));
    assert!(wrapped.next().await.is_none());
}

#[tokio::test]
async fn streaming_detection_rejects_content_overflow_once() {
    use futures::StreamExt;

    let chunk = Bytes::from(format!(
        "data: {}\n\n",
        json!({"choices": [{"delta": {"content": "hello"}}]})
    ));
    let mut wrapped = wrap_with_test_limits(vec![Ok(chunk)], 1024, 4, 1);

    let error = wrapped
        .next()
        .await
        .expect("overflow should emit an error")
        .expect_err("overflow must not replay partial data");
    assert!(error
        .to_string()
        .contains("tool_injection_stream_too_large"));
    assert!(error.to_string().contains("4-byte tool detection limit"));
    assert!(wrapped.next().await.is_none());
}

#[tokio::test]
async fn streaming_detection_rejects_chunk_count_overflow_once() {
    use futures::StreamExt;

    let chunks = vec![Ok(Bytes::new()), Ok(Bytes::new())];
    let mut wrapped = wrap_with_test_limits(chunks, 0, 0, 1);

    let error = wrapped
        .next()
        .await
        .expect("overflow should emit an error")
        .expect_err("overflow must not replay partial data");
    assert!(error
        .to_string()
        .contains("tool_injection_stream_too_large"));
    assert!(error.to_string().contains("1-chunk tool detection limit"));
    assert!(wrapped.next().await.is_none());
}

#[tokio::test]
async fn streaming_detection_forwards_upstream_error_once_without_replay() {
    use futures::StreamExt;

    let chunks = vec![
        Ok(Bytes::from_static(b"buffered")),
        Err(synthetic_stream_error("upstream_test_error").await),
        Ok(Bytes::from_static(b"unreachable")),
    ];
    let mut wrapped = wrap_with_test_limits(chunks, 64, 64, 3);

    let error = wrapped
        .next()
        .await
        .expect("upstream error should be forwarded")
        .expect_err("buffered chunks must not be replayed after an upstream error");
    assert!(matches!(error, StreamError::Transport(_)));
    assert!(source_messages(&error).contains("upstream_test_error"));
    assert!(wrapped.next().await.is_none());
}

#[tokio::test]
async fn streaming_detection_empty_stream() {
    use futures::StreamExt;

    let chunks: Vec<Result<Bytes, rquest::Error>> = vec![];
    let inner = futures::stream::iter(chunks);
    let mut wrapped = wrap_streaming_tool_detection(
        inner,
        "deepseek-chat".to_string(),
        "resp-test".to_string(),
        vec![],
        None,
        None,
    );

    let mut count = 0;
    while let Some(Ok(_)) = wrapped.next().await {
        count += 1;
    }
    assert_eq!(count, 0);
}
