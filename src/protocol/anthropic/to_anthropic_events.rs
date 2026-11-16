//! Wire formatting is separate from the incremental Messages state and its lifecycle.
use crate::protocol::{
    bedrock_converse::StreamRelay, canonical::TokenUsage, vertex_turn::VertexAssistantTurn,
};
use serde_json::json;

pub(super) fn build_anthropic_message_start_event(
    model: &str,
    response_id: &str,
    usage: Option<&TokenUsage>,
) -> String {
    let data = json!({
        "type": "message_start",
        "message": {
            "id": response_id,
            "type": "message",
            "role": "assistant",
            "content": [],
            "model": model,
            "stop_reason": null,
            "stop_sequence": null,
            "usage": {
                "input_tokens": usage.map(|v| v.prompt_tokens).unwrap_or(0),
                "output_tokens": 0
            }
        }
    });
    format!("event: message_start\ndata: {data}\n\n")
}
pub(super) fn build_anthropic_text_block_start_event(index: usize) -> String {
    let data = json!({
        "type": "content_block_start",
        "index": index,
        "content_block": {"type": "text", "text": ""}
    });
    format!("event: content_block_start\ndata: {data}\n\n")
}
pub(super) fn build_anthropic_tool_block_start_event(index: usize, id: &str, name: &str) -> String {
    let data = json!({
        "type": "content_block_start",
        "index": index,
        "content_block": {"type": "tool_use", "id": id, "name": name, "input": {}}
    });
    format!("event: content_block_start\ndata: {data}\n\n")
}
pub(super) fn build_anthropic_text_block_delta_event(index: usize, text: &str) -> String {
    let data = json!({
        "type": "content_block_delta",
        "index": index,
        "delta": {"type": "text_delta", "text": text}
    });
    format!("event: content_block_delta\ndata: {data}\n\n")
}
pub(super) fn build_anthropic_tool_block_delta_event(index: usize, partial_json: &str) -> String {
    let data = json!({
        "type": "content_block_delta",
        "index": index,
        "delta": {"type": "input_json_delta", "partial_json": partial_json}
    });
    format!("event: content_block_delta\ndata: {data}\n\n")
}
pub(super) fn build_anthropic_content_block_stop(index: usize) -> String {
    let data = json!({"type": "content_block_stop", "index": index});
    format!("event: content_block_stop\ndata: {data}\n\n")
}
pub(super) fn build_anthropic_message_delta(
    stop_reason: &str,
    usage: Option<&TokenUsage>,
    turn: Option<&VertexAssistantTurn>,
    bedrock: &StreamRelay,
) -> String {
    let mut data = json!({
        "type": "message_delta",
        "delta": {"stop_reason": stop_reason, "stop_sequence": null},
        "usage": {
            "input_tokens": usage.map(|v| v.prompt_tokens).unwrap_or(0),
            "output_tokens": usage.map(|v| v.completion_tokens).unwrap_or(0)
        }
    });
    if let Some(usage) = usage {
        if let Some(read) = usage.cache_read_input_tokens {
            data["usage"]["cache_read_input_tokens"] = read.into();
        }
        if let Some(created) = usage.cache_creation_input_tokens {
            data["usage"]["cache_creation_input_tokens"] = created.into();
        }
    }
    crate::protocol::vertex_turn::project(&mut data, turn);
    bedrock.project(&mut data);
    format!("event: message_delta\ndata: {data}\n\n")
}
pub(super) fn build_anthropic_message_stop_event() -> String {
    let data = json!({"type": "message_stop"});
    format!("event: message_stop\ndata: {data}\n\n")
}
