//! Normal termination is never synthesized after invalid or unsupported upstream content.
use super::fixture::{decode, output, sse};
use neuro_gateway::protocol::stream_error::StreamError;
use serde_json::{json, Value};

fn terminal() -> Value {
    json!({"choices":[{"delta":{},"finish_reason":"stop"}],
        "usage":{"prompt_tokens":1,"completion_tokens":2,"total_tokens":3}})
}

async fn rejects(wire: &[u8]) {
    for size in [1, wire.len()] {
        let frames = output(wire, size).await;
        assert!(matches!(frames.last(), Some(Err(StreamError::Protocol(_)))));
        assert_eq!(frames.iter().filter(|v| v.is_err()).count(), 1);
        for frame in frames.iter().filter_map(|v| v.as_ref().ok()) {
            let kind = decode(frame).0;
            assert!(!matches!(kind.as_str(), "messageStop" | "metadata"));
        }
        assert!(!frames
            .last()
            .unwrap()
            .as_ref()
            .unwrap_err()
            .to_string()
            .contains("private-sentinel"));
    }
}

#[tokio::test]
async fn native_rejects_missing_or_unrepresentable_usage_instead_of_inventing_zero() {
    for usage in [
        Value::Null,
        json!({}),
        json!({"prompt_tokens":1}),
        json!([]),
        json!({"prompt_tokens":-1,"completion_tokens":2,"total_tokens":1}),
        json!({"prompt_tokens":1,"completion_tokens":2,"total_tokens":2147483648u64}),
        json!({"prompt_tokens":1,"completion_tokens":2,"total_tokens":3,"cache_read_input_tokens":-1}),
    ] {
        let mut end = terminal();
        end["usage"] = usage;
        rejects(&sse(&[end])).await;
    }
}

#[tokio::test]
async fn native_rejects_duplicate_or_out_of_order_terminal_events() {
    let content = json!({"choices":[{"delta":{"content":"private-sentinel"}}]});
    for frames in [
        vec![terminal(), terminal()],
        vec![terminal(), content],
        vec![json!({"choices":[{"delta":{},"finish_reason":"future-reason"}]})],
        vec![json!({"choices":[{"delta":{},"finish_reason":"tool_calls"}]})],
    ] {
        rejects(&sse(&frames)).await;
    }
    rejects(b"data: [DONE]\n\n").await;
    let mut duplicate = sse(&[terminal()]);
    duplicate.extend_from_slice(b"data: [DONE]\n\n");
    rejects(&duplicate).await;
}

#[tokio::test]
async fn native_rejects_multiple_choices_and_unknown_nonnull_content_without_silent_loss() {
    for delta in [
        json!({"refusal":"private-sentinel"}),
        json!({"reasoning_content":"private-sentinel"}),
        json!({"audio":{}}),
        json!({"content":[]}),
        json!({"role":"user"}),
    ] {
        rejects(&sse(&[json!({"choices":[{"delta":delta}]}), terminal()])).await;
    }
    for bad in [
        json!({"choices":[{"delta":{}},{"delta":{}}]}),
        json!({"choices":[{"index":1,"delta":{}}]}),
        json!({"choices":[{"delta":null}]}),
        json!({}),
        json!([]),
    ] {
        rejects(&sse(&[bad, terminal()])).await;
    }
}

#[tokio::test]
async fn native_rejects_incomplete_or_invalid_tool_payloads_and_wrong_types() {
    for call in [
        json!({"index":0,"function":{"arguments":"{}"}}),
        json!({"index":0,"id":"call-a","function":{"name":"lookup","arguments":"{"}}),
        json!({"index":0,"id":"call-a","type":"other"}),
        json!({"index":0,"id":5}),
        json!({"index":0,"function":"private-sentinel"}),
        json!({"index":0,"function":{"arguments":{}}}),
    ] {
        let mut end = terminal();
        end["choices"][0]["finish_reason"] = json!("tool_calls");
        rejects(&sse(&[
            json!({"choices":[{"delta":{"tool_calls":[call]}}]}),
            end,
        ]))
        .await;
    }
}

#[tokio::test]
async fn native_rejects_error_events_bad_json_utf8_and_truncated_frames_without_echoing_input() {
    for wire in [
        b"event: error\ndata: private-sentinel\n\n".to_vec(),
        sse(&[json!({"error":{"message":"private-sentinel"}})]),
        b"data: private-sentinel\n\n".to_vec(),
        b"data: \xff\n\n".to_vec(),
        b"data: {\"private-sentinel\":".to_vec(),
    ] {
        rejects(&wire).await;
    }
}

#[tokio::test]
async fn native_null_optional_error_and_content_fields_are_not_failures() {
    let wire = sse(&[
        json!({"error":null,"choices":[{"delta":{
        "refusal":null,"audio":null,"content":null,"role":null,"tool_calls":null}}]}),
        terminal(),
    ]);
    let frames = output(&wire, 1).await;
    let events: Vec<_> = frames.iter().map(|v| decode(v.as_ref().unwrap())).collect();
    assert_eq!(
        events.iter().map(|v| v.0.as_str()).collect::<Vec<_>>(),
        ["messageStart", "messageStop", "metadata"]
    );
}
