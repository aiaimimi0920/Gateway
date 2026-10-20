//! Product replay must pass the existing Responses-to-Chat bridge without duplicating deltas.
use bytes::Bytes;
use futures::{stream, StreamExt};
use neuro_gateway::protocol::{
    chatgpt::basispoints::response, responses, stream_error::StreamError,
};
use serde_json::{json, Value};

async fn chat_frames(body: &Value) -> Vec<Value> {
    let source = stream::iter(vec![Ok::<_, StreamError<std::io::Error>>(Bytes::from(
        response::replay(body),
    ))]);
    let chunks =
        responses::translate_responses_sse_to_openai_chat_with_error(source, "fixture".into())
            .collect::<Vec<_>>()
            .await;
    let mut bytes = Vec::new();
    for chunk in chunks {
        bytes.extend_from_slice(&chunk.unwrap());
    }
    String::from_utf8(bytes)
        .unwrap()
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .filter(|line| *line != "[DONE]")
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[tokio::test]
async fn replayed_text_is_delivered_once_by_existing_chat_translation() {
    let body = json!({"id":"r","model":"fixture","status":"completed","output":[{"id":"m","type":"message","role":"assistant",
        "content":[{"type":"output_text","text":"answer","annotations":[]}]}],"usage":{"input_tokens":2,"output_tokens":3,"total_tokens":5}});
    let frames = chat_frames(&body).await;
    let text: String = frames
        .iter()
        .filter_map(|f| f["choices"][0]["delta"]["content"].as_str())
        .collect();
    assert_eq!(text, "answer");
    assert_eq!(
        frames
            .iter()
            .filter(|f| f["choices"][0]["finish_reason"] == "stop")
            .count(),
        1
    );
}

#[tokio::test]
async fn replayed_function_arguments_are_delivered_once_and_custom_fields_survive_buffering() {
    let body = json!({"id":"r","model":"fixture","status":"completed","output":[{"id":"fc","type":"function_call","name":"exec",
        "call_id":"c","arguments":"{\"cmd\":\"pwd\"}"}]});
    let frames = chat_frames(&body).await;
    let args: String = frames
        .iter()
        .filter_map(|f| f["choices"][0]["delta"]["tool_calls"][0]["function"]["arguments"].as_str())
        .collect();
    assert_eq!(args, "{\"cmd\":\"pwd\"}");
    let custom = json!({"id":"r","model":"fixture","status":"completed","output":[{"id":"fc","type":"custom_tool_call","namespace":"shell",
        "name":"patch","call_id":"c","input":"raw patch"}]});
    let canonical = responses::unpack_responses_response(&custom).unwrap();
    let rebuilt = responses::build_responses_success(
        "r",
        "fixture",
        &canonical.text,
        canonical.usage.as_ref(),
        &canonical.tool_calls,
        canonical.finish_reason.as_deref(),
    );
    assert_eq!(rebuilt["output"][0]["type"], "custom_tool_call");
    assert_eq!(rebuilt["output"][0]["namespace"], "shell");
    assert_eq!(rebuilt["output"][0]["input"], "raw patch");
}
