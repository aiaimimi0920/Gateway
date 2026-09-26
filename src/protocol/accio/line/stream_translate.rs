use bytes::Bytes;
use futures::Stream;
use serde_json::json;
use std::collections::{HashMap, VecDeque};

use crate::protocol::canonical::TokenUsage;

use super::{drain_parsed_events, parse_sse_line, ParsedEvent, PendingToolCall};

struct TranslatorState {
    buffer: Vec<u8>,
    model: String,
    response_id: String,
    created: i64,
    usage: Option<TokenUsage>,
    pending: HashMap<i64, PendingToolCall>,
    outputs: VecDeque<Vec<u8>>,
    finish_seen: bool,
    done_sent: bool,
}

pub fn translate_accio_sse_to_openai(
    line: &[u8],
    model: &str,
    response_id: &str,
    created: i64,
) -> Option<Vec<u8>> {
    let events = parse_sse_line(line)?;
    for event in events {
        match event {
            ParsedEvent::ProviderError { .. } => {}
            ParsedEvent::Text(text) => {
                return Some(text_chunk(response_id, model, created, &text));
            }
            ParsedEvent::ToolCall {
                id,
                name,
                arguments,
            } => {
                return Some(tool_chunk(
                    response_id,
                    model,
                    created,
                    0,
                    Some(&id),
                    Some(&name),
                    &arguments,
                    true,
                ));
            }
            ParsedEvent::Finish { reason, usage } => {
                return Some(finish_chunk(
                    response_id,
                    model,
                    created,
                    reason.as_deref().unwrap_or("stop"),
                    usage.as_ref(),
                ));
            }
            ParsedEvent::Done => return Some(b"data: [DONE]\n\n".to_vec()),
            ParsedEvent::Start { .. }
            | ParsedEvent::ToolStart { .. }
            | ParsedEvent::ToolDelta { .. }
            | ParsedEvent::ToolEnd { .. } => {}
        }
    }
    None
}

pub fn translate_anthropic_like_stream_to_openai(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    model: String,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    let state = TranslatorState {
        buffer: Vec::new(),
        model,
        response_id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        created: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64,
        usage: None,
        pending: HashMap::new(),
        outputs: VecDeque::new(),
        finish_seen: false,
        done_sent: false,
    };

    futures::stream::unfold(
        (
            Box::pin(inner)
                as std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, rquest::Error>> + Send>>,
            state,
            false,
        ),
        |(mut stream, mut state, done)| async move {
            use futures::StreamExt;
            if done {
                return None;
            }
            loop {
                if let Some(output) = state.outputs.pop_front() {
                    return Some((Ok(Bytes::from(output)), (stream, state, done)));
                }
                let drained = drain_parsed_events(&mut state.buffer);
                if !drained.is_empty() {
                    for output in translate_events_with_state(drained, &mut state) {
                        state.outputs.push_back(output);
                    }
                    continue;
                }
                match stream.next().await {
                    Some(Ok(chunk)) => state.buffer.extend_from_slice(&chunk),
                    Some(Err(error)) => return Some((Err(error), (stream, state, true))),
                    None => {
                        if state.finish_seen && !state.done_sent {
                            state.done_sent = true;
                            return Some((
                                Ok(Bytes::from_static(b"data: [DONE]\n\n")),
                                (stream, state, true),
                            ));
                        }
                        return None;
                    }
                }
            }
        },
    )
}

pub fn translate_accio_stream(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    model: String,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    translate_anthropic_like_stream_to_openai(inner, model)
}

fn merge_usage(existing: Option<TokenUsage>, incoming: Option<TokenUsage>) -> Option<TokenUsage> {
    match (existing, incoming) {
        (None, None) => None,
        (Some(current), None) => Some(current),
        (None, Some(new_usage)) => Some(new_usage),
        (Some(current), Some(new_usage)) => Some(TokenUsage {
            prompt_tokens: current.prompt_tokens.max(new_usage.prompt_tokens),
            completion_tokens: current.completion_tokens.max(new_usage.completion_tokens),
            total_tokens: current.total_tokens.max(new_usage.total_tokens).max(
                current
                    .prompt_tokens
                    .max(new_usage.prompt_tokens)
                    .saturating_add(current.completion_tokens.max(new_usage.completion_tokens)),
            ),
            cache_creation_input_tokens: new_usage
                .cache_creation_input_tokens
                .or(current.cache_creation_input_tokens),
            cache_read_input_tokens: new_usage
                .cache_read_input_tokens
                .or(current.cache_read_input_tokens),
        }),
    }
}

fn text_chunk(response_id: &str, model: &str, created: i64, text: &str) -> Vec<u8> {
    format!(
        "data: {}\n\n",
        json!({
            "id": response_id,
            "object": "chat.completion.chunk",
            "created": created,
            "model": model,
            "choices": [{"index": 0, "delta": {"content": text}, "finish_reason": null}]
        })
    )
    .into_bytes()
}

fn tool_chunk(
    response_id: &str,
    model: &str,
    created: i64,
    index: usize,
    tool_id: Option<&str>,
    tool_name: Option<&str>,
    arguments_fragment: &str,
    include_identity: bool,
) -> Vec<u8> {
    let mut tool_delta = json!({
        "index": index,
        "function": {"arguments": arguments_fragment}
    });
    if include_identity {
        tool_delta["id"] = json!(tool_id.unwrap_or_default());
        tool_delta["type"] = json!("function");
        tool_delta["function"]["name"] = json!(tool_name.unwrap_or_default());
    }
    format!(
        "data: {}\n\n",
        json!({
            "id": response_id,
            "object": "chat.completion.chunk",
            "created": created,
            "model": model,
            "choices": [{"index": 0, "delta": {"tool_calls": [tool_delta]}, "finish_reason": null}]
        })
    )
    .into_bytes()
}

fn finish_chunk(
    response_id: &str,
    model: &str,
    created: i64,
    reason: &str,
    usage: Option<&TokenUsage>,
) -> Vec<u8> {
    let mut chunk = json!({
        "id": response_id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "choices": [{"index": 0, "delta": {}, "finish_reason": reason}]
    });
    if let Some(usage) = usage {
        chunk["usage"] = json!({
            "prompt_tokens": usage.prompt_tokens,
            "completion_tokens": usage.completion_tokens,
            "total_tokens": usage.total_tokens,
            "cache_creation_input_tokens": usage.cache_creation_input_tokens,
            "cache_read_input_tokens": usage.cache_read_input_tokens
        });
    }
    format!("data: {}\n\n", chunk).into_bytes()
}

fn translate_events_with_state(
    events: Vec<ParsedEvent>,
    state: &mut TranslatorState,
) -> Vec<Vec<u8>> {
    let mut outputs = Vec::new();
    for event in events {
        match event {
            ParsedEvent::ProviderError { .. } => {}
            ParsedEvent::Text(text) => outputs.push(text_chunk(
                &state.response_id,
                &state.model,
                state.created,
                &text,
            )),
            ParsedEvent::ToolStart { index, id, name } => {
                state.pending.insert(
                    index,
                    PendingToolCall {
                        id,
                        name,
                        arguments: String::new(),
                        announced: false,
                    },
                );
            }
            ParsedEvent::ToolDelta { index, partial } => {
                if let Some(call) = state.pending.get_mut(&index) {
                    call.arguments.push_str(&partial);
                    outputs.push(tool_chunk(
                        &state.response_id,
                        &state.model,
                        state.created,
                        index.max(0) as usize,
                        Some(&call.id),
                        Some(&call.name),
                        &partial,
                        !call.announced,
                    ));
                    call.announced = true;
                }
            }
            ParsedEvent::ToolEnd { index } => {
                if let Some(call) = state.pending.remove(&index) {
                    if !call.announced {
                        outputs.push(tool_chunk(
                            &state.response_id,
                            &state.model,
                            state.created,
                            index.max(0) as usize,
                            Some(&call.id),
                            Some(&call.name),
                            "",
                            true,
                        ));
                    }
                }
            }
            ParsedEvent::ToolCall {
                id,
                name,
                arguments,
            } => outputs.push(tool_chunk(
                &state.response_id,
                &state.model,
                state.created,
                0,
                Some(&id),
                Some(&name),
                &arguments,
                true,
            )),
            ParsedEvent::Finish { reason, usage } => {
                state.finish_seen = true;
                state.usage = merge_usage(state.usage.clone(), usage.clone());
                let mut pending_indexes: Vec<i64> = state.pending.keys().copied().collect();
                pending_indexes.sort_unstable();
                for index in pending_indexes {
                    if let Some(call) = state.pending.remove(&index) {
                        outputs.push(tool_chunk(
                            &state.response_id,
                            &state.model,
                            state.created,
                            index.max(0) as usize,
                            Some(&call.id),
                            Some(&call.name),
                            &call.arguments,
                            true,
                        ));
                    }
                }
                outputs.push(finish_chunk(
                    &state.response_id,
                    &state.model,
                    state.created,
                    reason.as_deref().unwrap_or("stop"),
                    state.usage.as_ref(),
                ));
            }
            ParsedEvent::Done => {
                state.done_sent = true;
                outputs.push(b"data: [DONE]\n\n".to_vec());
            }
            ParsedEvent::Start { model, usage } => {
                if let Some(model) = model {
                    state.model = model;
                }
                state.usage = merge_usage(state.usage.clone(), usage);
            }
        }
    }
    outputs
}
