//! Independent AWS wire assertions, not the encoder's private framing helper.

#[path = "bedrock_native_outbound_contract/errors.rs"]
mod errors;
#[path = "bedrock_native_outbound_contract/fixture.rs"]
mod fixture;
#[path = "bedrock_native_outbound_contract/lifetime.rs"]
mod lifetime;
#[path = "bedrock_native_outbound_contract/tools.rs"]
mod tools;

use fixture::{decode, output, sse};
use serde_json::json;

#[tokio::test]
async fn native_outbound_headers_payload_blocks_and_trailing_usage_are_real_aws_wire() {
    let wire = sse(&[
        json!({"choices":[{"delta":{"content":"你好"}}]}),
        json!({"choices":[{"delta":{},"finish_reason":"stop"}]}),
        json!({"choices":[],"usage":{"prompt_tokens":5,"completion_tokens":8,"total_tokens":13}}),
    ]);
    for chunk in [1, 7, wire.len()] {
        let frames = output(&wire, chunk).await;
        let events: Vec<_> = frames
            .iter()
            .map(|frame| decode(frame.as_ref().unwrap()))
            .collect();
        assert_eq!(
            events
                .iter()
                .map(|event| event.0.as_str())
                .collect::<Vec<_>>(),
            [
                "messageStart",
                "contentBlockDelta",
                "contentBlockStop",
                "messageStop",
                "metadata"
            ]
        );
        assert_eq!(events[0].1, json!({"role":"assistant"}));
        assert_eq!(
            events[1].1,
            json!({"contentBlockIndex":0,"delta":{"text":"你好"}})
        );
        assert_eq!(events[2].1, json!({"contentBlockIndex":0}));
        assert_eq!(events[3].1, json!({"stopReason":"end_turn"}));
        assert_eq!(
            events[4].1["usage"],
            json!({"inputTokens":5,"outputTokens":8,"totalTokens":13})
        );
        assert!(events[4].1["metrics"]["latencyMs"].as_u64().is_some());
    }
}

#[tokio::test]
async fn native_outbound_text_and_first_tool_never_share_a_content_block() {
    let wire = sse(&[
        json!({"choices":[{"delta":{"content":"calling"}}]}),
        json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call-a",
            "function":{"name":"lookup","arguments":"{}"}}]}}]}),
        json!({"choices":[{"delta":{},"finish_reason":"tool_calls"}],
            "usage":{"prompt_tokens":1,"completion_tokens":2,"total_tokens":3}}),
    ]);
    let frames = output(&wire, 1).await;
    let events: Vec<_> = frames
        .iter()
        .map(|frame| decode(frame.as_ref().unwrap()))
        .collect();
    let start = events
        .iter()
        .find(|event| event.0 == "contentBlockStart")
        .unwrap();
    assert_eq!(start.1["contentBlockIndex"], 1);
    let stops: Vec<_> = events
        .iter()
        .filter(|event| event.0 == "contentBlockStop")
        .map(|event| event.1["contentBlockIndex"].as_u64().unwrap())
        .collect();
    assert_eq!(stops, [0, 1]);
}

#[tokio::test]
async fn native_outbound_eof_without_finish_is_a_terminal_failure() {
    let wire = format!(
        "data: {}\n\n",
        json!({"choices":[{"delta":{"content":"partial"}}]})
    );
    let frames = output(wire.as_bytes(), 3).await;
    assert!(
        frames.last().unwrap().is_err(),
        "incomplete upstream must not become successful EOF"
    );
}

#[tokio::test]
async fn native_outbound_eof_after_partial_sse_frame_is_not_silently_ignored() {
    let mut wire = sse(&[json!({"choices":[{"delta":{},"finish_reason":"stop"}],
            "usage":{"prompt_tokens":0,"completion_tokens":0,"total_tokens":0}})]);
    wire.extend_from_slice(b"data: {\"choices\":");
    let frames = output(&wire, 1).await;
    assert!(
        frames.last().unwrap().is_err(),
        "trailing partial frame must fail even after DONE"
    );
}
