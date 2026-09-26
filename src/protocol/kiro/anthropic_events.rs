//! Anthropic Messages SSE event encoding and block wire order for Kiro.
use super::stream_state::StreamToolCall;
use super::{
    estimate_tokens, prompt_tokens_from_context, restore_tool_name, KiroEvent, TranslatorState,
};
use serde_json::{json, Value};

pub(super) fn push_anthropic_events(state: &mut TranslatorState, event: KiroEvent) -> bool {
    let mut terminal_error = false;
    ensure_anthropic_message_started(state);
    match event {
        KiroEvent::AssistantResponse { content } => {
            if content.is_empty() {
                return false;
            }
            state.completion_tokens = state
                .completion_tokens
                .saturating_add(estimate_tokens(&content));
            let index = open_or_create_text_block(state);
            state.outputs.push_back(anthropic_event_bytes(
                "content_block_delta",
                json!({
                    "type": "content_block_delta",
                    "index": index,
                    "delta": {
                        "type": "text_delta",
                        "text": content,
                    }
                }),
            ));
        }
        KiroEvent::ToolUse {
            name,
            tool_use_id,
            input,
            stop,
        } => {
            state.tool_calls_seen = true;
            if let Some(index) = state.open_text_index.take() {
                state.outputs.push_back(anthropic_event_bytes(
                    "content_block_stop",
                    json!({ "type": "content_block_stop", "index": index }),
                ));
            }
            let tool_position = next_anthropic_block_index(state, &tool_use_id);
            if !state.pending_tools.contains_key(&tool_use_id) {
                state.pending_tools.insert(
                    tool_use_id.clone(),
                    StreamToolCall {
                        id: tool_use_id.clone(),
                        name: restore_tool_name(&state.tool_name_map, &name),
                        announced: false,
                        index: tool_position,
                    },
                );
            }
            let entry = state
                .pending_tools
                .get_mut(&tool_use_id)
                .expect("tool exists");
            if !entry.announced {
                state.outputs.push_back(anthropic_event_bytes(
                    "content_block_start",
                    json!({
                        "type": "content_block_start",
                        "index": tool_position,
                        "content_block": {
                            "type": "tool_use",
                            "id": entry.id.clone(),
                            "name": entry.name.clone(),
                            "input": {},
                        }
                    }),
                ));
                entry.announced = true;
            }
            if !input.is_empty() {
                state.completion_tokens = state
                    .completion_tokens
                    .saturating_add(estimate_tokens(&input));
                state.outputs.push_back(anthropic_event_bytes(
                    "content_block_delta",
                    json!({
                        "type": "content_block_delta",
                        "index": tool_position,
                        "delta": {
                            "type": "input_json_delta",
                            "partial_json": input,
                        }
                    }),
                ));
            }
            if stop {
                state.outputs.push_back(anthropic_event_bytes(
                    "content_block_stop",
                    json!({ "type": "content_block_stop", "index": tool_position }),
                ));
            }
        }
        KiroEvent::ContextUsage {
            context_usage_percentage,
        } => {
            state.prompt_tokens =
                prompt_tokens_from_context(&state.model, context_usage_percentage)
                    .max(state.prompt_tokens);
            if context_usage_percentage >= 100.0 {
                state.context_overflow = true;
            }
        }
        KiroEvent::Error {
            error_code,
            error_message,
        } => {
            terminal_error = true;
            state.outputs.push_back(anthropic_event_bytes(
                "error",
                json!({
                    "type": "error",
                    "error": {
                        "type": "api_error",
                        "message": format!("{error_code}: {error_message}"),
                    }
                }),
            ));
        }
        KiroEvent::Exception {
            exception_type,
            message,
        } => {
            if exception_type == "ContentLengthExceededException" {
                state.context_overflow = true;
            } else {
                terminal_error = true;
                state.outputs.push_back(anthropic_event_bytes(
                    "error",
                    json!({
                        "type": "error",
                        "error": {
                            "type": "api_error",
                            "message": format!("{exception_type}: {message}"),
                        }
                    }),
                ));
            }
        }
        KiroEvent::Unknown => {}
    }
    terminal_error
}

pub(super) fn push_anthropic_finish(state: &mut TranslatorState) {
    ensure_anthropic_message_started(state);
    if let Some(index) = state.open_text_index.take() {
        state.outputs.push_back(anthropic_event_bytes(
            "content_block_stop",
            json!({ "type": "content_block_stop", "index": index }),
        ));
    }
    let stop_reason = if state.tool_calls_seen {
        "tool_use"
    } else if state.context_overflow {
        "max_tokens"
    } else {
        "end_turn"
    };
    state.outputs.push_back(anthropic_event_bytes(
        "message_delta",
        json!({
            "type": "message_delta",
            "delta": {
                "stop_reason": stop_reason,
                "stop_sequence": Value::Null,
            },
            "usage": {
                "input_tokens": state.prompt_tokens,
                "output_tokens": state.completion_tokens,
            }
        }),
    ));
    state.outputs.push_back(anthropic_event_bytes(
        "message_stop",
        json!({ "type": "message_stop" }),
    ));
}

fn ensure_anthropic_message_started(state: &mut TranslatorState) {
    if state.message_started {
        return;
    }
    state.message_started = true;
    state.outputs.push_back(anthropic_event_bytes(
        "message_start",
        json!({
            "type": "message_start",
            "message": {
                "id": state.anthropic_message_id,
                "type": "message",
                "role": "assistant",
                "content": [],
                "model": state.model,
                "stop_reason": Value::Null,
                "stop_sequence": Value::Null,
                "usage": {
                    "input_tokens": state.prompt_tokens,
                    "output_tokens": 0,
                }
            }
        }),
    ));
}

fn open_or_create_text_block(state: &mut TranslatorState) -> usize {
    if let Some(index) = state.open_text_index {
        return index;
    }
    let index = state.next_block_index;
    state.next_block_index += 1;
    state.open_text_index = Some(index);
    state.outputs.push_back(anthropic_event_bytes(
        "content_block_start",
        json!({
            "type": "content_block_start",
            "index": index,
            "content_block": {
                "type": "text",
                "text": "",
            }
        }),
    ));
    index
}

fn next_anthropic_block_index(state: &mut TranslatorState, tool_use_id: &str) -> usize {
    if let Some(index) = state
        .pending_tools
        .get(tool_use_id)
        .map(|entry| entry.index)
    {
        return index;
    }
    let index = state.next_block_index;
    state.next_block_index += 1;
    index
}

pub(super) fn anthropic_event_bytes(event: &str, data: Value) -> Vec<u8> {
    format!(
        "event: {event}\ndata: {}\n\n",
        serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string())
    )
    .into_bytes()
}
