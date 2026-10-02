use std::collections::{BTreeMap, BTreeSet, VecDeque};

use bytes::Bytes;
use futures::Stream;
use serde_json::{json, Value};

use super::stream_support::merge_stream_usage;
use super::unpack_responses_response;
use crate::protocol::canonical::TokenUsage;
use crate::protocol::openai;
use crate::protocol::sse_parse::{format_sse_event, SseFrame};
use crate::protocol::stream_decode::{
    BoundedSseDecoder, DecodeStep, MAX_TRANSLATED_SSE_FRAME_BYTES,
};
use crate::protocol::stream_error::ProtocolStreamError;
use crate::protocol::tool_inject;

pub fn translate_responses_sse_to_openai_chat_with_error<E>(
    inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    model: String,
) -> impl Stream<Item = Result<Bytes, E>> + Send + 'static
where
    E: From<ProtocolStreamError> + Send + 'static,
{
    translate_responses_sse_to_openai_chat_with_limit_and_error(
        inner,
        model,
        MAX_TRANSLATED_SSE_FRAME_BYTES,
    )
}

pub(super) fn translate_responses_sse_to_openai_chat_with_limit_and_error<E>(
    inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    model: String,
    max_frame_bytes: usize,
) -> impl Stream<Item = Result<Bytes, E>> + Send + 'static
where
    E: From<ProtocolStreamError> + Send + 'static,
{
    let state = ResponsesToOpenAiChatState {
        decoder: BoundedSseDecoder::new(max_frame_bytes),
        response_id: format!("chatcmpl_{}", uuid::Uuid::new_v4().as_simple()),
        model,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64,
        outputs: VecDeque::new(),
        passthrough_openai: false,
        latest_usage: None,
        pending_finish_reason: None,
        emitted_text: false,
        tool_call_indexes: BTreeMap::new(),
        announced_tool_calls: BTreeSet::new(),
        next_tool_call_index: 0,
        final_emitted: false,
    };

    futures::stream::unfold(
        (
            Box::pin(inner) as std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, E>> + Send>>,
            state,
            false,
        ),
        |(mut stream, mut st, mut done)| async move {
            use futures::StreamExt;

            if done {
                return None;
            }

            loop {
                if let Some(output) = st.outputs.pop_front() {
                    done = st.final_emitted && st.outputs.is_empty();
                    return Some((Ok(Bytes::from(output)), (stream, st, done)));
                }
                match st.decoder.decode_next() {
                    DecodeStep::Frame(frame) => {
                        st.handle_frame(frame);
                        continue;
                    }
                    DecodeStep::Error(error) => {
                        return Some((Err(error.into()), (stream, st, true)));
                    }
                    DecodeStep::NeedInput => {}
                }

                match stream.next().await {
                    Some(Ok(chunk)) => {
                        st.decoder.push_chunk(chunk);
                    }
                    Some(Err(error)) => return Some((Err(error), (stream, st, true))),
                    None => {
                        if let Some(frame) = st.decoder.flush_pending_frame() {
                            st.handle_frame(frame);
                        }
                        st.emit_final_if_needed(None);
                        if let Some(output) = st.outputs.pop_front() {
                            done = st.final_emitted && st.outputs.is_empty();
                            return Some((Ok(Bytes::from(output)), (stream, st, done)));
                        }
                        return None;
                    }
                }
            }
        },
    )
}

struct ResponsesToOpenAiChatState {
    decoder: BoundedSseDecoder,
    response_id: String,
    model: String,
    created_at: i64,
    outputs: VecDeque<Vec<u8>>,
    passthrough_openai: bool,
    latest_usage: Option<TokenUsage>,
    pending_finish_reason: Option<String>,
    emitted_text: bool,
    tool_call_indexes: BTreeMap<String, usize>,
    announced_tool_calls: BTreeSet<String>,
    next_tool_call_index: usize,
    final_emitted: bool,
}

impl ResponsesToOpenAiChatState {
    fn handle_frame(&mut self, frame: SseFrame) {
        if frame.data.is_empty() {
            return;
        }

        if self.passthrough_openai || frame_is_openai_chat(frame.data.as_str()) {
            self.passthrough_openai = true;
            self.outputs
                .push_back(format_sse_event(frame.event_name.as_deref(), &frame.data).into_bytes());
            if frame.data == "[DONE]" {
                self.final_emitted = true;
            }
            return;
        }

        if frame.data == "[DONE]" {
            self.emit_final_if_needed(None);
            return;
        }

        let Ok(payload) = serde_json::from_str::<Value>(&frame.data) else {
            return;
        };
        let event_type = frame
            .event_name
            .as_deref()
            .or_else(|| payload.get("type").and_then(|value| value.as_str()))
            .unwrap_or_default();

        match event_type {
            "error" | "response.failed" | "response.cancelled" => {
                // Never turn an explicit upstream failure into a successful stop at EOF.
                let error = json!({"error": {
                    "message": "Responses upstream stream failed before completion.",
                    "type": "upstream_error", "code": "responses_stream_upstream_error"
                }});
                self.outputs
                    .push_back(format_sse_event(Some("error"), &error.to_string()).into_bytes());
                self.final_emitted = true;
            }
            "response.created" | "response.in_progress" => {
                if let Some(response) = payload.get("response") {
                    if let Some(id) = response.get("id").and_then(|value| value.as_str()) {
                        if !id.trim().is_empty() {
                            self.response_id = id.to_string();
                        }
                    }
                    if let Some(model) = response.get("model").and_then(|value| value.as_str()) {
                        if !model.trim().is_empty() {
                            self.model = model.to_string();
                        }
                    }
                    if let Some(created_at) =
                        response.get("created_at").and_then(|value| value.as_i64())
                    {
                        self.created_at = created_at;
                    }
                }
            }
            "response.output_text.delta" => {
                if let Some(delta) = payload.get("delta").and_then(|value| value.as_str()) {
                    if !delta.is_empty() {
                        self.emitted_text = true;
                        self.outputs.push_back(
                            format_sse_event(
                                None,
                                &openai::build_chat_completions_delta(
                                    &self.response_id,
                                    self.created_at,
                                    &self.model,
                                    delta,
                                )
                                .to_string(),
                            )
                            .into_bytes(),
                        );
                    }
                }
            }
            "response.output_item.added" => {
                let Some(item) = payload.get("item") else {
                    return;
                };
                if item.get("type").and_then(|value| value.as_str()) != Some("function_call") {
                    return;
                }
                let item_id = item
                    .get("id")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string();
                let call_id = item
                    .get("call_id")
                    .and_then(|value| value.as_str())
                    .unwrap_or(item_id.as_str())
                    .to_string();
                let name = item
                    .get("name")
                    .and_then(|value| value.as_str())
                    .unwrap_or("tool")
                    .to_string();
                let index = self.tool_call_index_for(&item_id);
                if self.announced_tool_calls.insert(item_id.clone()) {
                    self.outputs.push_back(
                        format_sse_event(
                            None,
                            &build_openai_chat_tool_call_start_chunk(
                                &self.response_id,
                                self.created_at,
                                &self.model,
                                index,
                                &call_id,
                                &name,
                            )
                            .to_string(),
                        )
                        .into_bytes(),
                    );
                }
            }
            "response.function_call_arguments.delta" => {
                let item_id = payload
                    .get("item_id")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string();
                let delta = payload
                    .get("delta")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default();
                if item_id.is_empty() || delta.is_empty() {
                    return;
                }
                let index = self.tool_call_index_for(&item_id);
                self.outputs.push_back(
                    format_sse_event(
                        None,
                        &build_openai_chat_tool_call_arguments_chunk(
                            &self.response_id,
                            self.created_at,
                            &self.model,
                            index,
                            delta,
                        )
                        .to_string(),
                    )
                    .into_bytes(),
                );
            }
            "response.completed" | "response.incomplete" => {
                if let Some(response) = payload.get("response") {
                    if let Some(id) = response.get("id").and_then(|value| value.as_str()) {
                        if !id.trim().is_empty() {
                            self.response_id = id.to_string();
                        }
                    }
                    if let Some(model) = response.get("model").and_then(|value| value.as_str()) {
                        if !model.trim().is_empty() {
                            self.model = model.to_string();
                        }
                    }
                    if let Some(created_at) =
                        response.get("created_at").and_then(|value| value.as_i64())
                    {
                        self.created_at = created_at;
                    }
                    if let Ok(mut canonical) = unpack_responses_response(response) {
                        if canonical.tool_calls.is_empty() && !canonical.text.is_empty() {
                            let parse_result =
                                tool_inject::parse_tool_calls_from_text(&canonical.text);
                            if parse_result.had_tool_calls
                                && canonical.finish_reason.as_deref() != Some("incomplete")
                            {
                                canonical.text = parse_result.clean_text;
                                canonical.tool_calls = parse_result.tool_calls;
                                canonical.finish_reason = Some("tool_calls".to_string());
                            }
                        }
                        self.latest_usage =
                            merge_stream_usage(self.latest_usage.take(), canonical.usage.clone());
                        self.pending_finish_reason = canonical.finish_reason.clone();
                        if !self.emitted_text && !canonical.text.is_empty() {
                            self.emitted_text = true;
                            self.outputs.push_back(
                                format_sse_event(
                                    None,
                                    &openai::build_chat_completions_delta(
                                        &self.response_id,
                                        self.created_at,
                                        &self.model,
                                        &canonical.text,
                                    )
                                    .to_string(),
                                )
                                .into_bytes(),
                            );
                        }
                        for tool_call in canonical.tool_calls {
                            let item_id = tool_call
                                .id
                                .clone()
                                .unwrap_or_else(|| format!("call_{}", self.next_tool_call_index));
                            let index = self.tool_call_index_for(&item_id);
                            if self.announced_tool_calls.insert(item_id.clone()) {
                                self.outputs.push_back(
                                    format_sse_event(
                                        None,
                                        &build_openai_chat_tool_call_start_chunk(
                                            &self.response_id,
                                            self.created_at,
                                            &self.model,
                                            index,
                                            tool_call.id.as_deref().unwrap_or(item_id.as_str()),
                                            tool_call.name.as_deref().unwrap_or("tool"),
                                        )
                                        .to_string(),
                                    )
                                    .into_bytes(),
                                );
                            }
                            if let Some(arguments) = tool_call.arguments.as_deref() {
                                if !arguments.is_empty() {
                                    self.outputs.push_back(
                                        format_sse_event(
                                            None,
                                            &build_openai_chat_tool_call_arguments_chunk(
                                                &self.response_id,
                                                self.created_at,
                                                &self.model,
                                                index,
                                                arguments,
                                            )
                                            .to_string(),
                                        )
                                        .into_bytes(),
                                    );
                                }
                            }
                        }
                    }
                }
                self.emit_final_if_needed(None);
            }
            _ => {}
        }
    }

    fn tool_call_index_for(&mut self, item_id: &str) -> usize {
        if let Some(index) = self.tool_call_indexes.get(item_id).copied() {
            return index;
        }
        let index = self.next_tool_call_index;
        self.next_tool_call_index += 1;
        self.tool_call_indexes.insert(item_id.to_string(), index);
        index
    }

    fn emit_final_if_needed(&mut self, fallback_finish_reason: Option<&str>) {
        if self.final_emitted || self.passthrough_openai {
            return;
        }
        self.outputs.push_back(
            format_sse_event(
                None,
                &build_openai_chat_stop_chunk(
                    &self.response_id,
                    self.created_at,
                    &self.model,
                    self.latest_usage.as_ref(),
                    self.pending_finish_reason
                        .as_deref()
                        .or(fallback_finish_reason),
                )
                .to_string(),
            )
            .into_bytes(),
        );
        self.outputs
            .push_back(format_sse_event(None, "[DONE]").into_bytes());
        self.final_emitted = true;
    }
}

fn frame_is_openai_chat(data: &str) -> bool {
    serde_json::from_str::<Value>(data)
        .ok()
        .and_then(|value| {
            value
                .get("choices")
                .and_then(|choices| choices.as_array())
                .and_then(|choices| {
                    choices.first().map(|choice| {
                        choice.get("delta").is_some()
                            || choice
                                .get("message")
                                .and_then(|message| message.get("content"))
                                .is_some()
                    })
                })
        })
        .unwrap_or(false)
}

fn build_openai_chat_tool_call_start_chunk(
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
                    "function": {
                        "name": name,
                        "arguments": "",
                    }
                }]
            },
            "finish_reason": null,
        }],
    })
}

fn build_openai_chat_tool_call_arguments_chunk(
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
                "tool_calls": [{
                    "index": index,
                    "function": {
                        "arguments": arguments_delta,
                    }
                }]
            },
            "finish_reason": null,
        }],
    })
}

fn build_openai_chat_stop_chunk(
    response_id: &str,
    created_at: i64,
    model: &str,
    usage: Option<&TokenUsage>,
    finish_reason: Option<&str>,
) -> Value {
    let mut chunk = json!({
        "id": response_id,
        "object": "chat.completion.chunk",
        "created": created_at,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": {},
            "finish_reason": match finish_reason.unwrap_or("stop") {
                "completed" => "stop",
                "incomplete" => "length",
                other => other,
            },
        }],
    });

    if let Some(usage) = usage {
        chunk["usage"] = json!({
            "prompt_tokens": usage.prompt_tokens,
            "completion_tokens": usage.completion_tokens,
            "total_tokens": usage.total_tokens,
        });
    }

    chunk
}

#[cfg(test)]
#[path = "to_openai_chat_error_tests.rs"]
mod error_tests;
