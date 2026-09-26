use std::collections::VecDeque;

use bytes::Bytes;
use futures::Stream;
use serde_json::Value;

use super::{
    build_legacy_completions_delta, build_legacy_completions_stop, map_openai_finish_reason,
};
use crate::protocol::canonical::TokenUsage;
use crate::protocol::sse_parse::{format_sse_event, SseFrame};
use crate::protocol::stream_decode::{
    BoundedSseDecoder, DecodeStep, MAX_TRANSLATED_SSE_FRAME_BYTES,
};

pub fn translate_openai_chat_sse_to_legacy_completions(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    model: String,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    translate_openai_chat_sse_to_legacy_completions_with_limit(
        inner,
        model,
        MAX_TRANSLATED_SSE_FRAME_BYTES,
    )
}

pub(super) fn translate_openai_chat_sse_to_legacy_completions_with_limit(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    model: String,
    max_frame_bytes: usize,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    let state = OpenAiChatToLegacyCompletionsState {
        decoder: BoundedSseDecoder::new(max_frame_bytes),
        response_id: format!("cmpl_{}", uuid::Uuid::new_v4().as_simple()),
        model,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64,
        outputs: VecDeque::new(),
        native_passthrough: false,
        latest_usage: None,
        pending_finish_reason: None,
        final_emitted: false,
    };

    futures::stream::unfold(
        (
            Box::pin(inner)
                as std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, rquest::Error>> + Send>>,
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
                        st.abort();
                        return Some((Err(error), (stream, st, true)));
                    }
                    DecodeStep::NeedInput => {}
                }

                match stream.next().await {
                    Some(Ok(chunk)) => {
                        st.decoder.push_chunk(chunk);
                    }
                    Some(Err(error)) => {
                        st.abort();
                        return Some((Err(error), (stream, st, true)));
                    }
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

struct OpenAiChatToLegacyCompletionsState {
    decoder: BoundedSseDecoder,
    response_id: String,
    model: String,
    created_at: i64,
    outputs: VecDeque<Vec<u8>>,
    native_passthrough: bool,
    latest_usage: Option<TokenUsage>,
    pending_finish_reason: Option<String>,
    final_emitted: bool,
}

impl OpenAiChatToLegacyCompletionsState {
    fn handle_frame(&mut self, frame: SseFrame) {
        if frame.data.is_empty() {
            return;
        }

        if self.native_passthrough || frame_is_native_legacy_completions(&frame) {
            self.native_passthrough = true;
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

        let Ok(chunk) = serde_json::from_str::<Value>(&frame.data) else {
            return;
        };

        if let Some(model) = chunk.get("model").and_then(|value| value.as_str()) {
            if !model.trim().is_empty() {
                self.model = model.to_string();
            }
        }

        if let Some(usage) = extract_stream_usage(&chunk) {
            self.latest_usage = Some(usage);
        }

        let choice = chunk
            .get("choices")
            .and_then(|choices| choices.as_array())
            .and_then(|choices| choices.first());

        if let Some(choice) = choice {
            if let Some(text) = choice
                .get("delta")
                .and_then(|delta| delta.get("content"))
                .and_then(|value| value.as_str())
            {
                if !text.is_empty() {
                    let payload = build_legacy_completions_delta(
                        &self.response_id,
                        self.created_at,
                        &self.model,
                        text,
                    );
                    self.outputs
                        .push_back(format_sse_event(None, &payload.to_string()).into_bytes());
                }
            }

            if let Some(reason) = choice
                .get("finish_reason")
                .and_then(|value| value.as_str())
                .and_then(|value| map_openai_finish_reason(Some(value)))
            {
                self.pending_finish_reason = Some(reason);
                if self.latest_usage.is_some() {
                    self.emit_final_if_needed(None);
                }
            }
        }
    }

    fn abort(&mut self) {
        self.decoder.clear();
        self.outputs = VecDeque::new();
        self.model = String::new();
        self.latest_usage = None;
        self.pending_finish_reason = None;
    }

    fn emit_final_if_needed(&mut self, fallback_finish_reason: Option<&str>) {
        if self.final_emitted || self.native_passthrough {
            return;
        }

        let payload = build_legacy_completions_stop(
            &self.response_id,
            self.created_at,
            &self.model,
            self.latest_usage.as_ref(),
            self.pending_finish_reason
                .as_deref()
                .or(fallback_finish_reason),
        );
        self.outputs
            .push_back(format_sse_event(None, &payload.to_string()).into_bytes());
        self.outputs
            .push_back(format_sse_event(None, "[DONE]").into_bytes());
        self.final_emitted = true;
    }
}

fn frame_is_native_legacy_completions(frame: &SseFrame) -> bool {
    serde_json::from_str::<Value>(&frame.data)
        .ok()
        .and_then(|value| {
            value
                .get("choices")
                .and_then(|choices| choices.as_array())
                .and_then(|choices| {
                    choices.first().and_then(|choice| {
                        if choice.get("text").is_some() {
                            Some(true)
                        } else {
                            value
                                .get("object")
                                .and_then(|object| object.as_str())
                                .map(|object| object.starts_with("text_completion"))
                        }
                    })
                })
        })
        .unwrap_or(false)
}

fn extract_stream_usage(chunk: &Value) -> Option<TokenUsage> {
    let usage = chunk.get("usage")?;
    let prompt_tokens = usage
        .get("prompt_tokens")
        .or_else(|| usage.get("input_tokens"))
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    let completion_tokens = usage
        .get("completion_tokens")
        .or_else(|| usage.get("output_tokens"))
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    Some(TokenUsage {
        prompt_tokens,
        completion_tokens,
        total_tokens: usage
            .get("total_tokens")
            .and_then(|value| value.as_u64())
            .unwrap_or(prompt_tokens + completion_tokens),
        cache_creation_input_tokens: None,
        cache_read_input_tokens: usage
            .get("input_tokens_details")
            .and_then(|details| details.get("cached_tokens"))
            .and_then(|value| value.as_u64()),
    })
}
