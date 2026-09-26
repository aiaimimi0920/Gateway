//! Public translator wire characterization, independent of internal state layout.
use std::collections::HashMap;

use bytes::Bytes;
use futures::StreamExt;
use serde_json::{json, Value};

use super::event_stream_tests::{event_frame, request};
use super::{
    pack_kiro, translate_kiro_event_stream_to_anthropic_sse,
    translate_kiro_event_stream_to_openai_sse,
};
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalTool};

fn tool(id: &str, name: &str, input: &str, stop: bool) -> Vec<u8> {
    event_frame(
        "toolUseEvent",
        json!({"toolUseId":id,"name":name,"input":input,"stop":stop}),
    )
}

async fn output(
    anthropic: bool,
    frames: Vec<Vec<u8>>,
    req: CanonicalRelayRequest,
) -> (String, Vec<Value>) {
    let inner = futures::stream::iter(frames.into_iter().map(|f| Ok(Bytes::from(f))));
    let chunks = if anthropic {
        translate_kiro_event_stream_to_anthropic_sse(inner, "claude-sonnet-4.6".into(), req)
            .collect::<Vec<_>>()
            .await
    } else {
        translate_kiro_event_stream_to_openai_sse(inner, "claude-sonnet-4.6".into(), req)
            .collect::<Vec<_>>()
            .await
    };
    let wire = chunks
        .into_iter()
        .map(|chunk| String::from_utf8(chunk.unwrap().to_vec()).unwrap())
        .collect::<String>();
    let events = wire
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .filter(|data| *data != "[DONE]")
        .map(|data| serde_json::from_str(data).unwrap())
        .collect();
    (wire, events)
}

#[tokio::test]
async fn openai_tool_deltas_restore_names_and_keep_first_seen_indices() {
    let mut req = request();
    let name = format!("read_{}", "workspace_tree_".repeat(6));
    req.tools.push(CanonicalTool {
        tool_type: "function".into(),
        name: Some(name.clone()),
        description: None,
        input_schema: Some(json!({"type":"object"})),
        raw: HashMap::new(),
    });
    let packed = pack_kiro(&req, "claude-sonnet-4.6").unwrap();
    let short = packed["conversationState"]["currentMessage"]["userInputMessage"]
        ["userInputMessageContext"]["tools"][0]["toolSpecification"]["name"]
        .as_str()
        .unwrap();
    let frames = vec![
        tool("a", short, "{\"x\":", false),
        tool("b", "empty", "", false),
        tool("a", short, "1}", true),
        tool("b", "empty", "", true),
    ];
    let (wire, events) = output(false, frames, req).await;
    assert_eq!(events.len(), 4);
    let first = &events[0]["choices"][0]["delta"]["tool_calls"][0];
    assert_eq!(
        first,
        &json!({"index":0,"id":"a","type":"function",
        "function":{"name":name,"arguments":"{\"x\":"}})
    );
    assert_eq!(
        events[1]["choices"][0]["delta"]["tool_calls"][0],
        json!({"index":0,"id":null,"type":null,"function":{"name":null,"arguments":"1}"}})
    );
    assert_eq!(
        events[2]["choices"][0]["delta"]["tool_calls"][0]["index"],
        1
    );
    assert_eq!(events[2]["choices"][0]["delta"]["tool_calls"][0]["id"], "b");
    assert_eq!(events[3]["choices"][0]["finish_reason"], "tool_calls");
    assert_eq!(wire.matches("[DONE]").count(), 1);
}

#[tokio::test]
async fn anthropic_text_tool_text_preserves_block_and_delta_order() {
    let frames = vec![
        event_frame("assistantResponseEvent", json!({"content":"before"})),
        tool("a", "read", "{", false),
        tool("a", "read", "}", true),
        event_frame("assistantResponseEvent", json!({"content":"after"})),
    ];
    let (_, events) = output(true, frames, request()).await;
    let kinds: Vec<_> = events.iter().map(|v| v["type"].as_str().unwrap()).collect();
    assert_eq!(
        kinds,
        [
            "message_start",
            "content_block_start",
            "content_block_delta",
            "content_block_stop",
            "content_block_start",
            "content_block_delta",
            "content_block_delta",
            "content_block_stop",
            "content_block_start",
            "content_block_delta",
            "content_block_stop",
            "message_delta",
            "message_stop"
        ]
    );
    assert_eq!(events[1]["index"], 0);
    assert_eq!(events[4]["index"], 1);
    assert_eq!(
        events[4]["content_block"],
        json!({"type":"tool_use","id":"a","name":"read","input":{}})
    );
    assert_eq!(
        events[5]["delta"],
        json!({"type":"input_json_delta","partial_json":"{"})
    );
    assert_eq!(
        events[6]["delta"],
        json!({"type":"input_json_delta","partial_json":"}"})
    );
    assert_eq!(events[8]["index"], 2);
    assert_eq!(events[11]["delta"]["stop_reason"], "tool_use");
}

#[tokio::test]
async fn context_usage_is_monotonic_and_tool_finish_takes_precedence() {
    for anthropic in [false, true] {
        for with_tool in [false, true] {
            let mut frames = vec![
                event_frame("contextUsageEvent", json!({"contextUsagePercentage":100.0})),
                event_frame("contextUsageEvent", json!({"contextUsagePercentage":1.0})),
            ];
            if with_tool {
                frames.push(tool("a", "read", "{}", true));
            }
            let (_, events) = output(anthropic, frames, request()).await;
            let terminal = if anthropic {
                &events[events.len() - 2]
            } else {
                events.last().unwrap()
            };
            let expected_tokens = super::context_window_for_model("claude-sonnet-4.6");
            if anthropic {
                assert_eq!(terminal["usage"]["input_tokens"], expected_tokens);
                assert_eq!(
                    terminal["delta"]["stop_reason"],
                    if with_tool { "tool_use" } else { "max_tokens" }
                );
            } else {
                assert_eq!(terminal["usage"]["prompt_tokens"], expected_tokens);
                assert_eq!(
                    terminal["choices"][0]["finish_reason"],
                    if with_tool { "tool_calls" } else { "length" }
                );
            }
        }
    }
}
