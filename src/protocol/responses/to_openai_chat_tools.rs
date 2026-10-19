//! Wire builders for incremental Responses function calls translated to Chat tool calls.
use serde_json::{json, Value};

pub(super) fn build_openai_chat_tool_call_start_chunk(
    response_id: &str,
    created_at: i64,
    model: &str,
    index: usize,
    call_id: &str,
    name: &str,
) -> Value {
    json!({
        "id": response_id,
        "object": "chat.completion.chunk",
        "created": created_at,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": {
                "tool_calls": [{
                    "index": index,
                    "id": call_id,
                    "type": "function",
                    "function": { "name": name, "arguments": "" }
                }]
            },
            "finish_reason": null,
        }],
    })
}

pub(super) fn build_openai_chat_tool_call_arguments_chunk(
    response_id: &str,
    created_at: i64,
    model: &str,
    index: usize,
    arguments_delta: &str,
) -> Value {
    json!({
        "id": response_id,
        "object": "chat.completion.chunk",
        "created": created_at,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": {
                "tool_calls": [{ "index": index, "function": { "arguments": arguments_delta } }]
            },
            "finish_reason": null,
        }],
    })
}
