use bytes::Bytes;
use futures::StreamExt;
use neuro_gateway::protocol::{
    responses::{
        translate_openai_sse_to_responses_with_error,
        translate_responses_sse_to_openai_chat_with_error,
    },
    stream_error::ProtocolStreamError,
};
use serde_json::{json, Value};

fn wire(values: &[Value]) -> Bytes {
    Bytes::from(
        values
            .iter()
            .map(|value| format!("data: {value}\n\n"))
            .collect::<String>(),
    )
}

fn events(bytes: Vec<Bytes>) -> Vec<Value> {
    let text = bytes
        .into_iter()
        .flat_map(|bytes| bytes.to_vec())
        .collect::<Vec<_>>();
    String::from_utf8(text)
        .unwrap()
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .filter_map(|data| serde_json::from_str(data).ok())
        .collect()
}

#[tokio::test]
async fn chat_custom_input_deltas_reach_responses_without_json_or_entity_rewrites() {
    let chunks = wire(&[
        json!({"model":"actual","choices":[{"delta":{"tool_calls":[{"index":0,"id":"call-1","type":"custom","custom":{"name":"shell","input":"echo "}}]},"finish_reason":null}]}),
        json!({"choices":[{"delta":{"tool_calls":[{"index":0,"custom":{"input":"'&amp;'"}}]},"finish_reason":null}]}),
        json!({"choices":[{"delta":{},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":1,"completion_tokens":2}}),
    ]);
    let input = futures::stream::iter(vec![
        Ok::<_, ProtocolStreamError>(chunks),
        Ok(Bytes::from_static(b"data: [DONE]\n\n")),
    ]);
    let output = translate_openai_sse_to_responses_with_error(input, "route".into())
        .collect::<Vec<_>>()
        .await;
    let events = events(output.into_iter().collect::<Result<Vec<_>, _>>().unwrap());
    let terminal = events
        .iter()
        .find(|event| event["type"] == "response.completed")
        .unwrap();
    assert_eq!(
        terminal["response"]["output"][0]["type"],
        "custom_tool_call"
    );
    assert_eq!(terminal["response"]["output"][0]["input"], "echo '&amp;'");
    let deltas = events
        .iter()
        .filter(|event| event["type"] == "response.custom_tool_call_input.delta")
        .filter_map(|event| event["delta"].as_str())
        .collect::<String>();
    assert_eq!(deltas, "echo '&amp;'");
}

#[tokio::test]
async fn responses_custom_input_is_not_lost_or_duplicated_by_terminal_snapshot() {
    let item = json!({"type":"custom_tool_call","id":"item-1","call_id":"call-1","name":"shell","input":""});
    let mut complete = item.clone();
    complete["input"] = json!("a &quot; b");
    let chunks = wire(&[
        json!({"type":"response.created","response":{"id":"response-1","model":"actual"}}),
        json!({"type":"response.output_item.added","output_index":0,"item":item}),
        json!({"type":"response.custom_tool_call_input.delta","item_id":"item-1","output_index":0,"delta":"a &quot; b"}),
        json!({"type":"response.completed","response":{"id":"response-1","model":"actual","status":"completed","output":[complete]}}),
    ]);
    let input = futures::stream::iter(vec![Ok::<_, ProtocolStreamError>(chunks)]);
    let output = translate_responses_sse_to_openai_chat_with_error(input, "route".into())
        .collect::<Vec<_>>()
        .await;
    let events = events(output.into_iter().collect::<Result<Vec<_>, _>>().unwrap());
    let calls = events
        .iter()
        .filter_map(|event| event["choices"][0]["delta"]["tool_calls"].as_array())
        .flatten()
        .collect::<Vec<_>>();
    assert_eq!(
        calls.iter().filter(|call| call.get("id").is_some()).count(),
        1
    );
    assert_eq!(
        calls
            .iter()
            .filter_map(|call| call["custom"]["input"].as_str())
            .collect::<String>(),
        "a &quot; b"
    );
    assert!(calls.iter().all(|call| call["index"] == 0));
}

#[tokio::test]
async fn custom_input_before_name_and_interleaved_calls_keep_independent_identity() {
    let chunks = wire(&[
        json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"first","type":"custom","custom":{"input":"甲"}}]}}]}),
        json!({"choices":[{"delta":{"tool_calls":[{"index":1,"id":"second","type":"custom","custom":{"name":"shell","input":"乙"}}]}}]}),
        json!({"choices":[{"delta":{"tool_calls":[{"index":0,"custom":{"name":"shell","input":"尾"}}]},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":1,"completion_tokens":2}}),
    ]);
    let input = futures::stream::iter(vec![Ok::<_, ProtocolStreamError>(chunks)]);
    let output = translate_openai_sse_to_responses_with_error(input, "route".into())
        .collect::<Vec<_>>()
        .await;
    let events = events(output.into_iter().collect::<Result<Vec<_>, _>>().unwrap());
    let terminal = events
        .iter()
        .find(|event| event["type"] == "response.completed")
        .unwrap();
    let calls = terminal["response"]["output"].as_array().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0]["call_id"], "first");
    assert_eq!(calls[0]["input"], "甲尾");
    assert_eq!(calls[1]["call_id"], "second");
    assert_eq!(calls[1]["input"], "乙");
    let first_deltas = events
        .iter()
        .filter(|event| {
            event["type"] == "response.custom_tool_call_input.delta"
                && event["item_id"] == "fc_first"
        })
        .map(|event| event["delta"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(first_deltas, vec!["甲", "尾"]);
}

#[tokio::test]
async fn conflicting_terminal_input_fails_instead_of_replaying_or_claiming_success() {
    let chunks = wire(&[
        json!({"type":"response.output_item.added","item":{"type":"custom_tool_call","id":"item","call_id":"call","name":"shell","input":""}}),
        json!({"type":"response.custom_tool_call_input.delta","item_id":"item","delta":"sent"}),
        json!({"type":"response.completed","response":{"status":"completed","output":[{"type":"custom_tool_call","id":"item","call_id":"call","name":"shell","input":"different"}]}}),
    ]);
    let input = futures::stream::iter(vec![Ok::<_, ProtocolStreamError>(chunks)]);
    let output = translate_responses_sse_to_openai_chat_with_error(input, "route".into())
        .collect::<Vec<_>>()
        .await;
    assert!(output
        .last()
        .unwrap()
        .as_ref()
        .unwrap_err()
        .to_string()
        .contains("conflicts"));
    let events = events(output.into_iter().filter_map(Result::ok).collect());
    assert!(!events
        .iter()
        .any(|event| event["choices"][0]["finish_reason"].is_string()));
}

#[tokio::test]
async fn too_many_tool_calls_fail_before_an_unbounded_map_can_grow() {
    let calls = (0..129).map(|index| json!({"index":index,"id":format!("call-{index}"),"type":"custom","custom":{"name":"shell","input":""}})).collect::<Vec<_>>();
    let chunks =
        wire(&[json!({"choices":[{"delta":{"tool_calls":calls},"finish_reason":"tool_calls"}]})]);
    let input = futures::stream::iter(vec![Ok::<_, ProtocolStreamError>(chunks)]);
    let output = translate_openai_sse_to_responses_with_error(input, "route".into())
        .collect::<Vec<_>>()
        .await;
    assert!(output
        .last()
        .unwrap()
        .as_ref()
        .unwrap_err()
        .to_string()
        .contains("count limit"));
}

#[tokio::test]
async fn delayed_tool_names_cannot_accumulate_unbounded_delta_metadata() {
    // Small deltas stay below the byte limit but must still bound chunk metadata.
    let chunks = (0..8193).map(|_| Ok::<_, ProtocolStreamError>(wire(&[
        json!({"choices":[{"delta":{"tool_calls":[{"index":0,"type":"custom","custom":{"input":"x"}}]}}]})
    ])));
    let output =
        translate_openai_sse_to_responses_with_error(futures::stream::iter(chunks), "route".into())
            .collect::<Vec<_>>()
            .await;
    assert!(output
        .last()
        .unwrap()
        .as_ref()
        .unwrap_err()
        .to_string()
        .contains("pending tool delta limit"));
    assert!(!events(output.into_iter().filter_map(Result::ok).collect())
        .iter()
        .any(|event| event["type"] == "response.completed"));
}
