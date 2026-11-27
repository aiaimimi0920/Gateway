use std::collections::{BTreeMap, VecDeque};

use bytes::Bytes;
use futures::Stream;
use serde_json::Value;

use super::stream_support::{
    extract_openai_stream_usage, map_openai_finish_reason_to_responses_status, merge_stream_usage,
};
use crate::protocol::canonical::TokenUsage;
use crate::protocol::response_sources::{self, ResponseSource};
use crate::protocol::sse_parse::{format_sse_event, SseFrame};
use crate::protocol::stream_decode::{
    BoundedSseDecoder, DecodeStep, MAX_TRANSLATED_SSE_FRAME_BYTES,
};
use crate::protocol::stream_error::{ProtocolStreamError, StreamError};
use crate::protocol::stream_error_legacy::with_transport_error;

mod completed;
mod cursor_thinking;
pub(crate) use cursor_thinking::translate_cursor_sse_to_responses;
mod message_items;
mod perplexity;
mod tool_calls;

use message_items::PendingResponseMessage;
use tool_calls::PendingResponseToolCall;

/// Wrap an OpenAI chat-completions SSE byte stream and translate it to native
/// OpenAI Responses API SSE events on-the-fly.
///
/// If the upstream is already producing native Responses SSE, the stream is
/// passed through unchanged.
pub fn translate_openai_sse_to_responses(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    model: String,
) -> impl Stream<Item = Result<Bytes, StreamError<rquest::Error>>> + Send + 'static {
    translate_openai_sse_to_responses_with_error(with_transport_error(inner), model)
}

/// Preserve the caller's transport error while emitting local decode failures
/// through its chosen domain error, rather than a synthetic HTTP error.
pub fn translate_openai_sse_to_responses_with_error<E>(
    inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    model: String,
) -> impl Stream<Item = Result<Bytes, E>> + Send + 'static
where
    E: From<ProtocolStreamError> + Send + 'static,
{
    translate_openai_sse_to_responses_with_limit_and_error(
        inner,
        model,
        MAX_TRANSLATED_SSE_FRAME_BYTES,
        None,
    )
}

#[cfg(test)]
pub(super) fn translate_openai_sse_to_responses_with_limit(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    model: String,
    max_frame_bytes: usize,
) -> impl Stream<Item = Result<Bytes, StreamError<rquest::Error>>> + Send + 'static {
    translate_openai_sse_to_responses_with_limit_and_error(
        with_transport_error(inner),
        model,
        max_frame_bytes,
        None,
    )
}

fn translate_openai_sse_to_responses_with_limit_and_error<E>(
    inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    model: String,
    max_frame_bytes: usize,
    cursor_thinking: Option<crate::protocol::cursor_remote::thinking::PublicThinking>,
) -> impl Stream<Item = Result<Bytes, E>> + Send + 'static
where
    E: From<ProtocolStreamError> + Send + 'static,
{
    let state = OpenAiToResponsesState {
        decoder: BoundedSseDecoder::new(max_frame_bytes),
        response_id: format!("resp_{}", uuid::Uuid::new_v4().as_simple()),
        model,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64,
        outputs: VecDeque::new(),
        native_passthrough: false,
        response_started: false,
        pending_finish_status: None,
        latest_usage: None,
        sources: None,
        perplexity: Default::default(),
        vertex_turn: Default::default(),
        bedrock: Default::default(),
        message: None,
        tool_calls: BTreeMap::new(),
        next_output_index: 0,
        final_emitted: false,
        sequence_number: 0,
        failure: None,
        cursor_thinking,
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
                if let Some(error) = st.failure.take() {
                    stream = Box::pin(futures::stream::empty());
                    return Some((Err(error.into()), (stream, st, true)));
                }
                if let Some(output) = st.outputs.pop_front() {
                    return Some((Ok(Bytes::from(output)), (stream, st, done)));
                }
                if st.final_emitted {
                    // Bedrock can finalize only after observed EOF; an unfold source is not fused.
                    if st.bedrock.active() {
                        return None;
                    }
                    // Protocol completion is not HTTP EOF. Drain in the request's own
                    // lifetime so keep-alive remains reusable; cancellation still drops it.
                    match stream.next().await {
                        Some(Ok(_)) => continue,
                        Some(Err(error)) => return Some((Err(error), (stream, st, true))),
                        None => return None,
                    }
                }
                match st.decoder.decode_next() {
                    DecodeStep::Frame(frame) => {
                        st.handle_frame(frame);
                        continue;
                    }
                    DecodeStep::Error(error) => {
                        stream = Box::pin(futures::stream::empty());
                        return Some((Err(error.into()), (stream, st, true)));
                    }
                    DecodeStep::NeedInput => {}
                }

                match stream.next().await {
                    Some(Ok(chunk)) => {
                        st.decoder.push_chunk(chunk);
                    }
                    Some(Err(error)) => {
                        stream = Box::pin(futures::stream::empty());
                        return Some((Err(error), (stream, st, true)));
                    }
                    None => {
                        if st.bedrock.active() {
                            stream = Box::pin(futures::stream::empty());
                            if let Err(error) = st
                                .decoder
                                .finish_strict()
                                .and_then(|()| st.bedrock.finish())
                            {
                                stream = Box::pin(futures::stream::empty());
                                return Some((Err(error.into()), (stream, st, true)));
                            }
                        }
                        if let Some(frame) = st.decoder.flush_pending_frame() {
                            st.handle_frame(frame);
                        }
                        if let Err(error) = st
                            .perplexity
                            .require_terminal(st.pending_finish_status.is_some())
                        {
                            st.failure = Some(error);
                        }
                        if let Some(error) = st.failure.take() {
                            stream = Box::pin(futures::stream::empty());
                            return Some((Err(error.into()), (stream, st, true)));
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

struct OpenAiToResponsesState {
    decoder: BoundedSseDecoder,
    response_id: String,
    model: String,
    created_at: i64,
    outputs: VecDeque<Vec<u8>>,
    native_passthrough: bool,
    response_started: bool,
    pending_finish_status: Option<String>,
    latest_usage: Option<TokenUsage>,
    sources: Option<Vec<ResponseSource>>,
    perplexity: crate::protocol::perplexity_web::public_snapshot::Relay,
    vertex_turn: crate::protocol::vertex_turn::wire::TerminalTurn,
    bedrock: crate::protocol::bedrock_converse::StreamRelay,
    message: Option<PendingResponseMessage>,
    tool_calls: BTreeMap<usize, PendingResponseToolCall>,
    next_output_index: usize,
    final_emitted: bool,
    sequence_number: u64,
    failure: Option<ProtocolStreamError>,
    cursor_thinking: Option<crate::protocol::cursor_remote::thinking::PublicThinking>,
}

impl OpenAiToResponsesState {
    fn handle_frame(&mut self, frame: SseFrame) {
        if frame.data.is_empty() {
            return;
        }

        let parsed = serde_json::from_str::<Value>(&frame.data).ok();
        if parsed
            .as_ref()
            .is_some_and(|chunk| self.handle_perplexity_snapshot(chunk))
        {
            return;
        }
        let has_bedrock = parsed.as_ref().is_some_and(|v| {
            v.get("gateway_bedrock_event").is_some() || v.get("gateway_bedrock_terminal").is_some()
        });
        if !self.bedrock.active()
            && !has_bedrock
            && (self.native_passthrough || frame_is_native_responses(&frame))
        {
            self.native_passthrough = true;
            self.outputs
                .push_back(format_sse_event(frame.event_name.as_deref(), &frame.data).into_bytes());
            if frame.data == "[DONE]" {
                self.final_emitted = true;
            }
            return;
        }

        if frame.data == "[DONE]" {
            if let Err(error) = self.bedrock.done().and_then(|()| {
                self.perplexity
                    .require_terminal(self.pending_finish_status.is_some())
            }) {
                self.failure = Some(error);
                return;
            }
            self.emit_final_if_needed(None);
            return;
        }

        let Some(chunk) = parsed else {
            if self.bedrock.active() {
                self.failure = Some(ProtocolStreamError::invalid_data(
                    "Invalid Bedrock stream JSON",
                ));
            }
            return;
        };
        match self.bedrock.observe(&chunk) {
            Ok(Some((_, body))) => {
                self.ensure_response_started();
                self.outputs.push_back(
                    crate::protocol::bedrock_converse::StreamRelay::public_event(&chunk, &body),
                );
            }
            Ok(None) => {}
            Err(error) => {
                self.failure = Some(error);
                return;
            }
        }

        // A source-bearing finish freezes the answer even while usage is pending.
        // Its own final delta is processed below; only later mutations are rejected.
        let delta = &chunk["choices"][0]["delta"];
        if self.sources.is_some()
            && (delta["content"]
                .as_str()
                .is_some_and(|text| !text.is_empty())
                || delta["tool_calls"]
                    .as_array()
                    .is_some_and(|calls| !calls.is_empty()))
        {
            self.failure = Some(ProtocolStreamError::invalid_data(response_sources::INVALID));
            return;
        }
        if let Err(error) = self.vertex_turn.accept(&chunk) {
            self.failure = Some(error);
            return;
        }
        // Sources belong to this answer's terminal choice, never an old snapshot.
        match response_sources::read(&chunk) {
            Ok(Some(sources))
                if self.sources.is_none()
                    && chunk["choices"][0]["finish_reason"]
                        .as_str()
                        .and_then(map_openai_finish_reason_to_responses_status)
                        .is_some() =>
            {
                self.sources = Some(sources);
            }
            Ok(None) => {}
            _ => {
                self.failure = Some(ProtocolStreamError::invalid_data(response_sources::INVALID));
                return;
            }
        }

        if let Some(model) = chunk.get("model").and_then(|value| value.as_str()) {
            if !model.trim().is_empty() {
                self.model = model.to_string();
            }
        }

        if let Some(usage) = extract_openai_stream_usage(&chunk) {
            self.latest_usage = merge_stream_usage(self.latest_usage.take(), Some(usage));
        }

        self.ensure_response_started();

        let choice = chunk
            .get("choices")
            .and_then(|choices| choices.as_array())
            .and_then(|choices| choices.first());

        if let Some(choice) = choice {
            self.handle_choice(choice);
        }

        if self.pending_finish_status.is_some() && choice.is_none() && self.latest_usage.is_some() {
            self.emit_final_if_needed(None);
        }
    }

    fn handle_choice(&mut self, choice: &Value) {
        let delta = choice.get("delta");
        let finish_status = choice
            .get("finish_reason")
            .and_then(|value| value.as_str())
            .and_then(map_openai_finish_reason_to_responses_status);

        if let Some(text) = delta
            .and_then(|entry| entry.get("content"))
            .and_then(|entry| entry.as_str())
        {
            if !text.is_empty() {
                self.handle_text_delta(text);
            }
        }

        if let Some(tool_calls) = delta
            .and_then(|entry| entry.get("tool_calls"))
            .and_then(|entry| entry.as_array())
        {
            for (fallback_index, tool_call) in tool_calls.iter().enumerate() {
                if let Err(error) = self.handle_tool_call_delta(tool_call, fallback_index) {
                    self.failure = Some(error);
                    return;
                }
            }
        }

        if let Some(status) = finish_status {
            self.pending_finish_status = Some(status);
            if self.latest_usage.is_some() {
                self.emit_final_if_needed(None);
            }
        }
    }
}

fn frame_is_native_responses(frame: &SseFrame) -> bool {
    if frame
        .event_name
        .as_deref()
        .map(|name| name.starts_with("response.") || name == "error")
        .unwrap_or(false)
    {
        return true;
    }

    serde_json::from_str::<Value>(&frame.data)
        .ok()
        .and_then(|value| {
            value
                .get("type")
                .and_then(|entry| entry.as_str())
                .map(str::to_string)
        })
        .map(|value| value.starts_with("response.") || value == "error")
        .unwrap_or(false)
}

#[cfg(test)]
#[path = "to_responses_error_tests.rs"]
mod error_tests;
