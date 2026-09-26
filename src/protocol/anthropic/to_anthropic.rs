use bytes::Bytes;
use futures::Stream;
use serde_json::{json, Value};
use std::collections::{BTreeMap, VecDeque};

use crate::protocol::canonical::TokenUsage;
use crate::protocol::sse_parse::SseFrame;
use crate::protocol::stream_decode::{
    BoundedSseDecoder, DecodeStep, MAX_TRANSLATED_SSE_FRAME_BYTES,
};

const MAX_TRANSLATED_CONTENT_BLOCKS: usize = 4096;

// ---------------------------------------------------------------------------
// OpenAI SSE → Anthropic SSE stream translation
// ---------------------------------------------------------------------------

fn build_anthropic_message_start_event(
    model: &str,
    response_id: &str,
    usage: Option<&TokenUsage>,
) -> String {
    let prompt_tokens = usage.map(|entry| entry.prompt_tokens).unwrap_or(0);
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
                "input_tokens": prompt_tokens,
                "output_tokens": 0
            }
        }
    });
    format!("event: message_start\ndata: {}\n\n", data)
}

fn build_anthropic_text_block_start_event(index: usize) -> String {
    let data = json!({
        "type": "content_block_start",
        "index": index,
        "content_block": {
            "type": "text",
            "text": ""
        }
    });
    format!("event: content_block_start\ndata: {}\n\n", data)
}

fn build_anthropic_tool_block_start_event(index: usize, id: &str, name: &str) -> String {
    let data = json!({
        "type": "content_block_start",
        "index": index,
        "content_block": {
            "type": "tool_use",
            "id": id,
            "name": name,
            "input": {}
        }
    });
    format!("event: content_block_start\ndata: {}\n\n", data)
}

fn build_anthropic_text_block_delta_event(index: usize, text: &str) -> String {
    let data = json!({
        "type": "content_block_delta",
        "index": index,
        "delta": {
            "type": "text_delta",
            "text": text
        }
    });
    format!("event: content_block_delta\ndata: {}\n\n", data)
}

fn build_anthropic_tool_block_delta_event(index: usize, partial_json: &str) -> String {
    let data = json!({
        "type": "content_block_delta",
        "index": index,
        "delta": {
            "type": "input_json_delta",
            "partial_json": partial_json
        }
    });
    format!("event: content_block_delta\ndata: {}\n\n", data)
}

fn build_anthropic_content_block_stop(index: usize) -> String {
    let data = json!({
        "type": "content_block_stop",
        "index": index
    });
    format!("event: content_block_stop\ndata: {}\n\n", data)
}

fn build_anthropic_message_delta(stop_reason: &str, usage: Option<&TokenUsage>) -> String {
    let mut data = json!({
        "type": "message_delta",
        "delta": {
            "stop_reason": stop_reason,
            "stop_sequence": null
        }
    });

    if let Some(u) = usage {
        data["usage"] = json!({
            "input_tokens": u.prompt_tokens,
            "output_tokens": u.completion_tokens
        });
    } else {
        data["usage"] = json!({
            "input_tokens": 0,
            "output_tokens": 0
        });
    }

    format!("event: message_delta\ndata: {}\n\n", data)
}

fn build_anthropic_message_stop_event() -> String {
    let data = json!({
        "type": "message_stop"
    });
    format!("event: message_stop\ndata: {}\n\n", data)
}

/// Wrap an OpenAI-format SSE byte stream and translate each event to Anthropic
/// SSE event format on-the-fly.
///
/// The translator implements a state machine:
/// 1. On the first content chunk: emit `message_start` + `content_block_start`
/// 2. For each content delta: emit `content_block_delta`
/// 3. On `finish_reason` or `[DONE]`: emit `content_block_stop` +
///    `message_delta` + `message_stop`
pub fn translate_openai_sse_to_anthropic(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    model: String,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    translate_openai_sse_to_anthropic_with_limit(inner, model, MAX_TRANSLATED_SSE_FRAME_BYTES)
}

pub(super) fn translate_openai_sse_to_anthropic_with_limit(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    model: String,
    max_frame_bytes: usize,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    let response_id = format!("msg_{}", uuid::Uuid::new_v4().as_simple());

    let state = OpenAiToAnthropicState {
        decoder: BoundedSseDecoder::new(max_frame_bytes),
        model,
        response_id,
        outputs: VecDeque::new(),
        message_started: false,
        open_text_index: None,
        next_block_index: 0,
        tool_blocks: BTreeMap::new(),
        pending_finish_reason: None,
        latest_usage: None,
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
                        if let Err(error) = st.handle_frame(frame) {
                            st.abort();
                            return Some((Err(error), (stream, st, true)));
                        }
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
                            if let Err(error) = st.handle_frame(frame) {
                                st.abort();
                                return Some((Err(error), (stream, st, true)));
                            }
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

/// Internal state for the OpenAI-to-Anthropic stream translator.
struct OpenAiToAnthropicState {
    decoder: BoundedSseDecoder,
    model: String,
    response_id: String,
    outputs: VecDeque<Vec<u8>>,
    message_started: bool,
    open_text_index: Option<usize>,
    next_block_index: usize,
    tool_blocks: BTreeMap<usize, PendingAnthropicToolBlock>,
    pending_finish_reason: Option<String>,
    latest_usage: Option<TokenUsage>,
    final_emitted: bool,
}

struct PendingAnthropicToolBlock {
    block_index: usize,
}

impl OpenAiToAnthropicState {
    fn handle_frame(&mut self, frame: SseFrame) -> Result<(), rquest::Error> {
        if frame.data.is_empty() {
            return Ok(());
        }
        if frame.data == "[DONE]" {
            self.emit_final_if_needed(None);
            return Ok(());
        }

        let Ok(chunk) = serde_json::from_str::<Value>(&frame.data) else {
            return Ok(());
        };
        if let Some(usage) = extract_openai_stream_usage(&chunk) {
            self.latest_usage = Some(usage);
        }

        let choice = chunk
            .get("choices")
            .and_then(|choices| choices.as_array())
            .and_then(|choices| choices.first());

        if let Some(choice) = choice {
            self.handle_choice(choice)?;
        }

        if self.pending_finish_reason.is_some() && choice.is_none() && self.latest_usage.is_some() {
            self.emit_final_if_needed(None);
        }
        Ok(())
    }

    fn handle_choice(&mut self, choice: &Value) -> Result<(), rquest::Error> {
        let delta = choice.get("delta");
        let finish_reason = choice
            .get("finish_reason")
            .and_then(|value| value.as_str())
            .and_then(map_openai_finish_reason);

        if let Some(tool_calls) = delta
            .and_then(|entry| entry.get("tool_calls"))
            .and_then(|entry| entry.as_array())
        {
            self.close_text_block();
            self.ensure_message_started();
            for (fallback_index, tool_call) in tool_calls.iter().enumerate() {
                self.handle_tool_call_delta(tool_call, fallback_index)?;
            }
        }

        if let Some(text) = delta
            .and_then(|entry| entry.get("content"))
            .and_then(|entry| entry.as_str())
        {
            if !text.is_empty() {
                self.close_tool_blocks();
                self.ensure_message_started();
                let index = self.open_or_create_text_block()?;
                self.outputs
                    .push_back(build_anthropic_text_block_delta_event(index, text).into_bytes());
            }
        }

        if let Some(reason) = finish_reason {
            self.pending_finish_reason = Some(reason);
            if self.latest_usage.is_some() {
                self.emit_final_if_needed(None);
            }
        }
        Ok(())
    }

    fn handle_tool_call_delta(
        &mut self,
        tool_call: &Value,
        fallback_index: usize,
    ) -> Result<(), rquest::Error> {
        let openai_index = tool_call
            .get("index")
            .and_then(|value| value.as_u64())
            .map(|value| value as usize)
            .unwrap_or(fallback_index);
        let block_index = if let Some(entry) = self.tool_blocks.get(&openai_index) {
            entry.block_index
        } else {
            let block_index = self.allocate_block_index()?;
            let id = tool_call
                .get("id")
                .and_then(|value| value.as_str())
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| format!("toolu_{}", uuid::Uuid::new_v4().as_simple()));
            let name = tool_call
                .get("function")
                .and_then(|function| function.get("name"))
                .and_then(|value| value.as_str())
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| format!("tool_{}", openai_index));
            self.tool_blocks
                .insert(openai_index, PendingAnthropicToolBlock { block_index });
            self.outputs.push_back(
                build_anthropic_tool_block_start_event(block_index, &id, &name).into_bytes(),
            );
            block_index
        };

        if let Some(arguments) = tool_call
            .get("function")
            .and_then(|function| function.get("arguments"))
            .and_then(|value| value.as_str())
            .filter(|value| !value.is_empty())
        {
            self.outputs.push_back(
                build_anthropic_tool_block_delta_event(block_index, arguments).into_bytes(),
            );
        }
        Ok(())
    }

    fn ensure_message_started(&mut self) {
        if self.message_started {
            return;
        }
        self.message_started = true;
        self.outputs.push_back(
            build_anthropic_message_start_event(
                &self.model,
                &self.response_id,
                self.latest_usage.as_ref(),
            )
            .into_bytes(),
        );
    }

    fn open_or_create_text_block(&mut self) -> Result<usize, rquest::Error> {
        if let Some(index) = self.open_text_index {
            return Ok(index);
        }
        let index = self.allocate_block_index()?;
        self.open_text_index = Some(index);
        self.outputs
            .push_back(build_anthropic_text_block_start_event(index).into_bytes());
        Ok(index)
    }

    fn close_text_block(&mut self) {
        if let Some(index) = self.open_text_index.take() {
            self.outputs
                .push_back(build_anthropic_content_block_stop(index).into_bytes());
        }
    }

    fn close_tool_blocks(&mut self) {
        for (_, entry) in std::mem::take(&mut self.tool_blocks) {
            self.outputs
                .push_back(build_anthropic_content_block_stop(entry.block_index).into_bytes());
        }
    }

    fn allocate_block_index(&mut self) -> Result<usize, rquest::Error> {
        if self.next_block_index >= MAX_TRANSLATED_CONTENT_BLOCKS {
            return Err(translation_error(format!(
                "translated Anthropic stream exceeded the {MAX_TRANSLATED_CONTENT_BLOCKS}-content-block limit"
            )));
        }
        let index = self.next_block_index;
        self.next_block_index += 1;
        Ok(index)
    }

    fn abort(&mut self) {
        self.decoder.clear();
        self.outputs.clear();
        self.tool_blocks.clear();
        self.open_text_index = None;
        self.pending_finish_reason = None;
        self.latest_usage = None;
    }

    fn emit_final_if_needed(&mut self, fallback_reason: Option<&str>) {
        if self.final_emitted {
            return;
        }
        self.ensure_message_started();
        self.close_text_block();
        self.close_tool_blocks();
        let reason = self
            .pending_finish_reason
            .clone()
            .or_else(|| fallback_reason.map(str::to_string))
            .unwrap_or_else(|| "end_turn".to_string());
        self.outputs.push_back(
            build_anthropic_message_delta(&reason, self.latest_usage.as_ref()).into_bytes(),
        );
        self.outputs
            .push_back(build_anthropic_message_stop_event().into_bytes());
        self.final_emitted = true;
    }
}

fn translation_error(message: String) -> rquest::Error {
    let source = std::io::Error::new(std::io::ErrorKind::InvalidData, message);
    rquest::Error::from(serde_json::Error::io(source))
}

fn extract_openai_stream_usage(chunk: &Value) -> Option<TokenUsage> {
    let usage = chunk.get("usage")?;
    let prompt_tokens = usage
        .get("prompt_tokens")
        .and_then(|value| value.as_u64())
        .or_else(|| usage.get("input_tokens").and_then(|value| value.as_u64()))
        .unwrap_or(0);
    let completion_tokens = usage
        .get("completion_tokens")
        .and_then(|value| value.as_u64())
        .or_else(|| usage.get("output_tokens").and_then(|value| value.as_u64()))
        .unwrap_or(0);
    let total_tokens = usage
        .get("total_tokens")
        .and_then(|value| value.as_u64())
        .unwrap_or_else(|| prompt_tokens.saturating_add(completion_tokens));
    Some(TokenUsage {
        prompt_tokens,
        completion_tokens,
        total_tokens,
        cache_creation_input_tokens: usage
            .get("cache_creation_input_tokens")
            .and_then(|value| value.as_u64()),
        cache_read_input_tokens: usage
            .get("cache_read_input_tokens")
            .and_then(|value| value.as_u64())
            .or_else(|| {
                usage
                    .get("prompt_tokens_details")
                    .and_then(|details| details.get("cached_tokens"))
                    .and_then(|value| value.as_u64())
            })
            .or_else(|| {
                usage
                    .get("input_tokens_details")
                    .and_then(|details| details.get("cached_tokens"))
                    .and_then(|value| value.as_u64())
            }),
    })
}

fn map_openai_finish_reason(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    Some(
        match value {
            "stop" => "end_turn",
            "length" => "max_tokens",
            "tool_calls" => "tool_use",
            other => other,
        }
        .to_string(),
    )
}
