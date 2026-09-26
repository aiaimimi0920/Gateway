use super::*;

async fn translate_chunks(chunks: Vec<Value>) -> Vec<Value> {
    let chunks = chunks
        .into_iter()
        .map(|chunk| Ok(Bytes::from(format_sse_event(None, &chunk.to_string()))))
        .collect();
    collect_sse_frames(translate_openai_sse_to_responses(
        make_bytes_stream(chunks),
        "model".into(),
    ))
    .await
    .into_iter()
    .map(|frame| serde_json::from_str(&frame.data).unwrap())
    .collect()
}

#[tokio::test]
async fn mixed_items_close_in_output_order_after_delayed_tool_announcement() {
    let events = translate_chunks(vec![
        json!({"choices": [{"delta": {"tool_calls": [{
            "index": 7, "id": "first", "function": {"arguments": "{"}
        }]}}]}),
        json!({"choices": [{"delta": {"content": "text", "tool_calls": [{
            "index": 2, "id": "second", "function": {"name": "second_tool", "arguments": "{}"}
        }]}}]}),
        json!({"choices": [{"delta": {"tool_calls": [{
            "index": 7, "function": {"name": "first_tool", "arguments": "}"}
        }]}}]}),
    ])
    .await;

    let added = events
        .iter()
        .filter(|event| event["type"] == "response.output_item.added")
        .map(|event| event["output_index"].as_u64().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(added, vec![1, 2, 0]);
    let closed = events
        .iter()
        .filter(|event| event["type"] == "response.output_item.done")
        .map(|event| event["output_index"].as_u64().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(closed, vec![0, 1, 2]);
    let first_arguments = events
        .iter()
        .filter(|event| {
            event["type"] == "response.function_call_arguments.delta"
                && event["item_id"] == "fc_first"
        })
        .map(|event| event["delta"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(first_arguments, vec!["{", "}"]);
    for (index, event) in events.iter().enumerate() {
        assert_eq!(event["sequence_number"], (index + 1) as u64);
    }

    let completed = events.last().unwrap();
    assert_eq!(completed["type"], "response.completed");
    let output = completed["response"]["output"].as_array().unwrap();
    assert_eq!(output.len(), 3);
    assert_eq!(output[0]["call_id"], "first");
    assert_eq!(output[0]["arguments"], "{}");
    assert_eq!(output[1]["content"][0]["text"], "text");
    assert_eq!(output[2]["call_id"], "second");
}

#[tokio::test]
async fn eof_announces_unnamed_tool_and_preserves_announced_call_identity() {
    let events = translate_chunks(vec![
        json!({"choices": [{"delta": {"tool_calls": [
            {"index": 3, "id": "unnamed", "function": {"arguments": " "}},
            {"index": 4, "id": "stable", "function": {"name": "tool", "arguments": "{"}}
        ]}}]}),
        json!({"choices": [{"delta": {"tool_calls": [{
            "index": 4, "id": "late", "function": {"arguments": "}"}
        }]}}]}),
    ])
    .await;

    let unnamed_added = events
        .iter()
        .position(|event| {
            event["type"] == "response.output_item.added" && event["item"]["call_id"] == "unnamed"
        })
        .unwrap();
    assert_eq!(events[unnamed_added]["item"]["name"], "tool_3");
    assert_eq!(
        events[unnamed_added + 1]["type"],
        "response.function_call_arguments.done"
    );
    assert_eq!(events[unnamed_added + 1]["arguments"], "{}");
    assert_eq!(
        events[unnamed_added + 2]["type"],
        "response.output_item.done"
    );
    let output = events.last().unwrap()["response"]["output"]
        .as_array()
        .unwrap();
    assert_eq!(output[0]["call_id"], "unnamed");
    assert_eq!(output[0]["arguments"], "{}");
    assert_eq!(output[1]["id"], "fc_stable");
    assert_eq!(output[1]["call_id"], "stable");
    assert_eq!(output[1]["arguments"], "{}");
}
