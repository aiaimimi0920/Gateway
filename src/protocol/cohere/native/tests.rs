use super::*;
use futures::{stream, TryStreamExt};
use serde_json::{json, Value};

fn frame(kind: &str, delta: Value) -> String {
    format!(
        "event: {kind}\r\ndata: {}\r\n\r\n",
        json!({"type":kind,"delta":delta,"index":0})
    )
}

fn text_fixture() -> String {
    frame("message-start", json!({"message":{"role":"assistant"}}))
        + &frame(
            "content-delta",
            json!({"message":{"content":{"text":"你好 OK"}}}),
        )
        + &frame(
            "message-end",
            json!({"finish_reason":"COMPLETE","usage":{"tokens":{"input_tokens":7.0,"output_tokens":2.0},
            "billed_units":{"input_tokens":3,"output_tokens":1},"cached_tokens":2}}),
        )
}

#[test]
fn native_sse_handles_fragmented_utf8_and_nested_actual_tokens() {
    let mut decoder = Decoder::new("command-r7b", true);
    for byte in text_fixture().as_bytes() {
        decoder.feed(&[*byte]).unwrap();
    }
    decoder.complete().unwrap();
    let result = unpack(decoder.events.collected.as_ref().unwrap()).unwrap();
    assert_eq!(result.text, "你好 OK");
    let usage = result.usage.unwrap();
    assert_eq!(
        (
            usage.prompt_tokens,
            usage.completion_tokens,
            usage.total_tokens
        ),
        (7, 2, 9)
    );
    assert_eq!(usage.cache_read_input_tokens, Some(2));
}

#[tokio::test]
async fn native_stream_outputs_text_usage_and_only_a_real_terminal_done() {
    let chunks = text_fixture()
        .bytes()
        .map(|byte| Ok::<_, StreamError<std::io::Error>>(Bytes::from(vec![byte])))
        .collect::<Vec<_>>();
    let output = translate(stream::iter(chunks), "command-r7b".into())
        .try_collect::<Vec<_>>()
        .await
        .unwrap();
    let output = String::from_utf8(output.concat()).unwrap();
    assert!(output.contains("你好 OK"));
    assert!(output.contains("\"finish_reason\":\"stop\""));
    assert!(output.contains("\"total_tokens\":9"));
    assert_eq!(output.matches("data: [DONE]").count(), 1);
    assert!(output.ends_with("data: [DONE]\n\n"));
}

#[test]
fn native_tool_fragments_accumulate_and_stream_without_retaining_arguments() {
    let fixture = frame(
        "tool-call-start",
        json!({"message":{"tool_calls":{"id":"call-1","function":{"name":"weather","arguments":""}}}}),
    ) + &frame(
        "tool-call-delta",
        json!({"message":{"tool_calls":{"function":{"arguments":"{\"city\":"}}}}),
    ) + &frame(
        "tool-call-delta",
        json!({"message":{"tool_calls":{"function":{"arguments":"\"上海\"}"}}}}),
    ) + &frame("message-end", json!({"finish_reason":"TOOL_CALL"}));
    let mut decoder = Decoder::new("command-r7b", true);
    decoder.feed(fixture.as_bytes()).unwrap();
    let result = unpack(decoder.events.collected.as_ref().unwrap()).unwrap();
    assert_eq!(result.finish_reason.as_deref(), Some("tool_calls"));
    assert_eq!(
        result.tool_calls[0].arguments.as_deref(),
        Some("{\"city\":\"上海\"}")
    );
    let mut decoder = Decoder::new("command-r7b", false);
    decoder.feed(fixture.as_bytes()).unwrap();
    assert!(decoder.events.collected.is_none());
    assert_eq!(decoder.outputs.len(), 5);
}

#[tokio::test]
async fn native_stream_does_not_convert_truncation_errors_or_empty_output_to_success() {
    for fixture in [
        frame(
            "content-delta",
            json!({"message":{"content":{"text":"partial"}}}),
        ),
        frame("message-end", json!({"finish_reason":"COMPLETE"})),
        frame(
            "message-end",
            json!({"finish_reason":"ERROR","error":"fixture-private-message"}),
        ),
        "data: not-json\n\n".into(),
        format!("data: {}", "x".repeat(FRAME_LIMIT)),
    ] {
        let stream = stream::iter(vec![Ok::<_, StreamError<std::io::Error>>(Bytes::from(
            fixture,
        ))]);
        let result = translate(stream, "command-r7b".into())
            .try_collect::<Vec<_>>()
            .await;
        assert!(matches!(result, Err(StreamError::Protocol(_))));
        assert!(!result.unwrap_err().to_string().contains("fixture-private"));
    }
    let sentinel = std::io::Error::new(std::io::ErrorKind::ConnectionReset, "sentinel");
    let result = translate(
        stream::iter(vec![Err(StreamError::Transport(sentinel))]),
        "model".into(),
    )
    .try_collect::<Vec<_>>()
    .await;
    assert!(matches!(result, Err(StreamError::Transport(_))));
}

#[test]
fn native_complete_response_never_substitutes_billed_units_for_tokens() {
    let response = unpack(
        &json!({"model":"command-r7b","message":{"content":[{"type":"text","text":"OK"}]},
        "finish_reason":"COMPLETE","usage":{"billed_units":{"input_tokens":9,"output_tokens":4}}}),
    )
    .unwrap();
    assert!(response.usage.is_none());
    assert_eq!(response.text, "OK");
    assert!(unpack(&json!({"message":{"content":[]},"finish_reason":"COMPLETE"})).is_err());
}

#[tokio::test]
async fn native_terminal_and_large_output_are_independent_of_http_chunk_sizes() {
    let fixture = text_fixture() + "data: invalid-trailing-extension\n\n";
    let mut results = Vec::new();
    for size in [1, 4096, fixture.len()] {
        let chunks = fixture
            .as_bytes()
            .chunks(size)
            .map(|v| Ok::<_, StreamError<std::io::Error>>(Bytes::copy_from_slice(v)))
            .collect::<Vec<_>>();
        let output = translate(stream::iter(chunks), "model".into())
            .try_collect::<Vec<_>>()
            .await
            .unwrap();
        let text = String::from_utf8(output.concat()).unwrap();
        assert!(text.ends_with("data: [DONE]\n\n"));
        results.push(text.matches("你好 OK").count());
    }
    assert_eq!(results, [1, 1, 1]);
    let fixture = frame(
        "content-delta",
        json!({"message":{"content":{"text":"x".repeat(8192)}}}),
    )
    .repeat(300)
        + &frame("message-end", json!({"finish_reason":"COMPLETE"}));
    let chunks = vec![Ok::<_, StreamError<std::io::Error>>(Bytes::from(fixture))];
    let output = translate(stream::iter(chunks), "model".into())
        .try_collect::<Vec<_>>()
        .await
        .unwrap();
    assert_eq!(output.len(), 302);
}
