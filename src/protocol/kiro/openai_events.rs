//! OpenAI Chat SSE event encoding for Kiro.
use super::stream_state::StreamToolCall;
use super::{
    estimate_tokens, prompt_tokens_from_context, restore_tool_name, KiroEvent, TranslatorState,
};
use serde_json::{json, Value};

pub(super) fn push_openai_events(state: &mut TranslatorState, event: KiroEvent) -> bool {
    let mut terminal_error = false;
    match event {
        KiroEvent::AssistantResponse { content } => {
            if content.is_empty() {
                return false;
            }
            state.completion_tokens = state
                .completion_tokens
                .saturating_add(estimate_tokens(&content));
            state.outputs.push_back(
                format!(
                    "data: {}\n\n",
                    json!({
                        "id": state.response_id,
                        "object": "chat.completion.chunk",
                        "created": state.created,
                        "model": state.model,
                        "choices": [{
                            "index": 0,
                            "delta": { "content": content },
                            "finish_reason": Value::Null,
                        }],
                    })
                )
                .into_bytes(),
            );
        }
        KiroEvent::ToolUse {
            name,
            tool_use_id,
            input,
            stop,
        } => {
            state.tool_calls_seen = true;
            let tool_position = next_openai_tool_index(state, &tool_use_id);
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
            if !input.is_empty() {
                state.completion_tokens = state
                    .completion_tokens
                    .saturating_add(estimate_tokens(&input));
                state.outputs.push_back(
                    format!(
                        "data: {}\n\n",
                        json!({
                            "id": state.response_id,
                            "object": "chat.completion.chunk",
                            "created": state.created,
                            "model": state.model,
                            "choices": [{
                                "index": 0,
                                "delta": {
                                    "tool_calls": [{
                                        "index": tool_position,
                                        "id": if entry.announced { Value::Null } else { json!(entry.id.clone()) },
                                        "type": if entry.announced { Value::Null } else { json!("function") },
                                        "function": {
                                            "name": if entry.announced { Value::Null } else { json!(entry.name.clone()) },
                                            "arguments": input,
                                        }
                                    }]
                                },
                                "finish_reason": Value::Null,
                            }],
                        })
                    )
                    .into_bytes(),
                );
                entry.announced = true;
            } else if stop && !entry.announced {
                state.outputs.push_back(
                    format!(
                        "data: {}\n\n",
                        json!({
                            "id": state.response_id,
                            "object": "chat.completion.chunk",
                            "created": state.created,
                            "model": state.model,
                            "choices": [{
                                "index": 0,
                                "delta": {
                                    "tool_calls": [{
                                        "index": tool_position,
                                        "id": entry.id.clone(),
                                        "type": "function",
                                        "function": {
                                            "name": entry.name.clone(),
                                            "arguments": "",
                                        }
                                    }]
                                },
                                "finish_reason": Value::Null,
                            }],
                        })
                    )
                    .into_bytes(),
                );
                entry.announced = true;
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
            state.outputs.push_back(
                format!(
                    "data: {}\n\n",
                    json!({ "error": { "message": error_message, "code": error_code } })
                )
                .into_bytes(),
            );
        }
        KiroEvent::Exception {
            exception_type,
            message,
        } => {
            if exception_type == "ContentLengthExceededException" {
                state.context_overflow = true;
            } else {
                terminal_error = true;
                state.outputs.push_back(
                    format!(
                        "data: {}\n\n",
                        json!({ "error": { "message": message, "code": exception_type } })
                    )
                    .into_bytes(),
                );
            }
        }
        KiroEvent::Unknown => {}
    }
    terminal_error
}

pub(super) fn push_openai_finish(state: &mut TranslatorState) {
    let finish_reason = if state.tool_calls_seen {
        "tool_calls"
    } else if state.context_overflow {
        "length"
    } else {
        "stop"
    };
    state.outputs.push_back(
        format!(
            "data: {}\n\n",
            json!({
                "id": state.response_id,
                "object": "chat.completion.chunk",
                "created": state.created,
                "model": state.model,
                "choices": [{
                    "index": 0,
                    "delta": {},
                    "finish_reason": finish_reason,
                }],
                "usage": {
                    "prompt_tokens": state.prompt_tokens,
                    "completion_tokens": state.completion_tokens,
                    "total_tokens": state.prompt_tokens.saturating_add(state.completion_tokens),
                }
            })
        )
        .into_bytes(),
    );
    state.outputs.push_back(b"data: [DONE]\n\n".to_vec());
}

fn next_openai_tool_index(state: &TranslatorState, tool_use_id: &str) -> usize {
    state
        .pending_tools
        .get(tool_use_id)
        .map(|entry| entry.index)
        .unwrap_or(state.pending_tools.len())
}
