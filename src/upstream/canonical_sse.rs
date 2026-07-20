use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse};
use crate::protocol::openai;
use crate::protocol::sse_parse::format_sse_event;
use serde_json::json;

pub(crate) fn canonical_response_to_openai_sse_bytes(
    req: &CanonicalRelayRequest,
    upstream_model: &str,
    response: &CanonicalRelayResponse,
) -> Vec<bytes::Bytes> {
    let response_id = format!("chatcmpl_canvas_{}", uuid::Uuid::new_v4().simple());
    let created_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let model = req
        .requested_model
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(upstream_model);
    let mut frames = Vec::new();

    if !response.text.is_empty() {
        frames.push(bytes::Bytes::from(format_sse_event(
            None,
            &openai::build_chat_completions_delta(&response_id, created_at, model, &response.text)
                .to_string(),
        )));
    }

    for (index, tool_call) in response.tool_calls.iter().enumerate() {
        let tool_name = tool_call.name.as_deref().unwrap_or("tool");
        let tool_id = tool_call.id.as_deref().unwrap_or("call_canvas_auto");
        let start_chunk = json!({
            "id": response_id,
            "object": "chat.completion.chunk",
            "created": created_at,
            "model": model,
            "choices": [{
                "index": 0,
                "delta": {
                    "tool_calls": [{
                        "index": index,
                        "id": tool_id,
                        "type": "function",
                        "function": {
                            "name": tool_name,
                            "arguments": "",
                        }
                    }]
                },
                "finish_reason": serde_json::Value::Null,
            }],
        });
        frames.push(bytes::Bytes::from(format_sse_event(
            None,
            &start_chunk.to_string(),
        )));

        if let Some(arguments) = tool_call.arguments.as_deref() {
            let arguments_chunk = json!({
                "id": response_id,
                "object": "chat.completion.chunk",
                "created": created_at,
                "model": model,
                "choices": [{
                    "index": 0,
                    "delta": {
                        "tool_calls": [{
                            "index": index,
                            "function": {
                                "arguments": arguments,
                            }
                        }]
                    },
                    "finish_reason": serde_json::Value::Null,
                }],
            });
            frames.push(bytes::Bytes::from(format_sse_event(
                None,
                &arguments_chunk.to_string(),
            )));
        }
    }

    let finish_reason =
        response
            .finish_reason
            .as_deref()
            .unwrap_or(if response.tool_calls.is_empty() {
                "stop"
            } else {
                "tool_calls"
            });
    let stop_chunk = json!({
        "id": response_id,
        "object": "chat.completion.chunk",
        "created": created_at,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": {},
            "finish_reason": finish_reason,
        }],
        "usage": response.usage.as_ref().map(|usage| json!({
            "prompt_tokens": usage.prompt_tokens,
            "completion_tokens": usage.completion_tokens,
            "total_tokens": usage.total_tokens,
        })).unwrap_or_else(|| json!(null)),
    });
    frames.push(bytes::Bytes::from(format_sse_event(
        None,
        &stop_chunk.to_string(),
    )));
    frames.push(bytes::Bytes::from(format_sse_event(None, "[DONE]")));
    frames
}
