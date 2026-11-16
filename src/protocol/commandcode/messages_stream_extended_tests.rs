//! Native streaming extensions must retain signatures, citations and complete server-tool blocks.
use bytes::Bytes;
use futures::StreamExt;
use serde_json::{json, Value};

pub(super) fn events() -> Vec<Value> {
    vec![
        json!({"type":"message_start","message":{"type":"message","id":"msg-rich","role":"assistant","model":"upstream","content":[],"usage":{"input_tokens":2,"output_tokens":0}}}),
        json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}),
        json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"reason"}}),
        json!({"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"signed"}}),
        json!({"type":"content_block_stop","index":0}),
        json!({"type":"content_block_start","index":1,"content_block":{"type":"redacted_thinking","data":"opaque"}}),
        json!({"type":"content_block_stop","index":1}),
        json!({"type":"content_block_start","index":2,"content_block":{"type":"server_tool_use","id":"srvtoolu_1","name":"web_search","input":{}}}),
        json!({"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"{\"query\":\"x\"}"}}),
        json!({"type":"content_block_stop","index":2}),
        json!({"type":"content_block_start","index":3,"content_block":{"type":"web_search_tool_result","tool_use_id":"srvtoolu_1","content":[{"type":"web_search_result","url":"https://example.com","title":"source","encrypted_content":"opaque"}]}}),
        json!({"type":"content_block_stop","index":3}),
        json!({"type":"content_block_start","index":4,"content_block":{"type":"text","text":""}}),
        json!({"type":"content_block_delta","index":4,"delta":{"type":"text_delta","text":"answer"}}),
        json!({"type":"content_block_delta","index":4,"delta":{"type":"citations_delta","citation":{"type":"web_search_result_location","url":"https://example.com","title":"source","encrypted_index":"opaque","cited_text":"source"}}}),
        json!({"type":"content_block_stop","index":4}),
        json!({"type":"provider_trace","trace":{"opaque":"preserved"},"usage":{"input_tokens":999,"output_tokens":999},"status":"incomplete","output":[{"type":"function_call"}]}),
        json!({"type":"message_delta","delta":{"stop_reason":null},"usage":{"output_tokens":2}}),
        json!({"type":"ping"}),
        json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":3,"prompt_tokens":999,"completion_tokens":999,"total_tokens":999}}),
        json!({"type":"provider_trace","usage":{"input_tokens":999,"output_tokens":999},"status":"incomplete","output":[{"type":"function_call"}]}),
        json!({"type":"message_stop"}),
    ]
}

pub(super) fn encode(events: &[Value]) -> Vec<u8> {
    events
        .iter()
        .map(|v| format!("event: {}\ndata: {v}\n\n", v["type"].as_str().unwrap()))
        .collect::<String>()
        .into_bytes()
}

async fn collect(events: &[Value], fragment: usize) -> (Vec<Value>, bool) {
    let input = futures::stream::iter(
        encode(events)
            .chunks(fragment)
            .map(|chunk| {
                Ok::<_, crate::protocol::stream_error::ProtocolStreamError>(Bytes::copy_from_slice(
                    chunk,
                ))
            })
            .collect::<Vec<_>>(),
    );
    let mut output = Box::pin(super::messages_stream::guard(input, "public".into()));
    let mut values = Vec::new();
    let mut failed = false;
    while let Some(result) = output.next().await {
        match result {
            Ok(bytes) => {
                let text = String::from_utf8(bytes.to_vec()).unwrap();
                for line in text.lines().filter_map(|line| line.strip_prefix("data: ")) {
                    values.push(serde_json::from_str(line).unwrap());
                }
            }
            Err(_) => failed = true,
        }
    }
    (values, failed)
}

#[tokio::test]
async fn commandcode_messages_sse_extensions_are_lossless_under_fragmentation() {
    for fragment in [1, 47, 8192] {
        let mut expected = events();
        let (actual, failed) = collect(&expected, fragment).await;
        expected[0]["message"]["model"] = "public".into();
        assert!(!failed);
        assert_eq!(actual, expected);
    }
}

#[tokio::test]
async fn commandcode_messages_sse_extensions_keep_terminal_failure_and_memory_boundaries() {
    let base = events();
    let mut invalid = Vec::new();
    let mut missing_signature = base.clone();
    missing_signature.remove(3);
    invalid.push(missing_signature);
    let mut wrong_delta = base.clone();
    wrong_delta[2]["delta"] = json!({"type":"text_delta","text":"wrong block"});
    invalid.push(wrong_delta);
    let mut no_terminal_reason = base.clone();
    no_terminal_reason[19]["delta"]["stop_reason"] = Value::Null;
    invalid.push(no_terminal_reason);
    let mut malformed_tool = base.clone();
    malformed_tool[8]["delta"]["partial_json"] = "{".into();
    invalid.push(malformed_tool);
    let mut conflicting_reason = base.clone();
    conflicting_reason[17]["delta"]["stop_reason"] = "max_tokens".into();
    invalid.push(conflicting_reason);
    let mut invalid_type = base.clone();
    invalid_type[16]["type"] = "provider_trace\nevent: message_delta".into();
    invalid.push(invalid_type);
    let mut tail_error = base.clone();
    tail_error.push(json!({"type":"error","error":{"message":"secret"}}));
    invalid.push(tail_error);
    for events in invalid {
        let (actual, failed) = collect(&events, 8192).await;
        assert!(failed);
        assert!(!actual.iter().any(|v| v["type"] == "message_stop"));
        assert!(!serde_json::to_string(&actual).unwrap().contains("secret"));
    }
}
