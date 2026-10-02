//! Bedrock stream state, ordered event projection and binary framing.

use super::map_bedrock_finish_reason;
use crate::protocol::canonical::TokenUsage;
use crate::protocol::stream_decode::{
    BoundedSseDecoder, DecodeStep, MAX_TRANSLATED_SSE_FRAME_BYTES,
};
use crate::protocol::stream_error::{ProtocolStreamError, StreamError};
use crate::protocol::stream_error_legacy::with_transport_error;
use bytes::Bytes;
use crc::{Crc, CRC_32_ISO_HDLC};
use futures::Stream;
use futures::StreamExt;
use serde_json::json;
use serde_json::Value;
use std::collections::HashMap;
use std::collections::{BTreeSet, VecDeque};

const MAX_TOOL_CALLS: usize = 4096;
const MAX_TOOL_STATE_BYTES: usize = 4 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;
const MAX_OUTPUT_EVENTS: usize = 2 * MAX_TOOL_CALLS + 4;

pub fn translate_openai_sse_to_bedrock_eventstream(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    model: String,
) -> impl Stream<Item = Result<Bytes, StreamError<rquest::Error>>> + Send + 'static {
    translate_openai_sse_to_bedrock_eventstream_with_error(with_transport_error(inner), model)
}

pub fn translate_openai_sse_to_bedrock_eventstream_with_error<E>(
    inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    model: String,
) -> impl Stream<Item = Result<Bytes, E>> + Send + 'static
where
    E: From<ProtocolStreamError> + Send + 'static,
{
    futures::stream::unfold(
        Some((
            Box::pin(inner) as std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, E>> + Send>>,
            BedrockStreamState::new(model),
        )),
        |state| async move {
            let (mut inner, mut state) = state?;
            loop {
                if let Some(output) = state.pop_output() {
                    return Some((Ok(output), Some((inner, state))));
                }

                // Decode one event only after the previous event's outputs drain.
                match state.decoder.decode_next() {
                    DecodeStep::Frame(frame) => {
                        if let Err(error) = state.handle_frame(frame) {
                            return Some((Err(error.into()), None));
                        }
                        continue;
                    }
                    DecodeStep::Error(error) => return Some((Err(error.into()), None)),
                    DecodeStep::NeedInput => {}
                }

                let next_chunk = inner.next().await;
                match next_chunk {
                    Some(Ok(bytes)) => state.decoder.push_chunk(bytes),
                    // Terminal errors drop both upstream and buffered state before yield.
                    Some(Err(error)) => return Some((Err(error), None)),
                    None => return None,
                }
            }
        },
    )
}

struct PendingBedrockToolCall {
    id: String,
    name: String,
}

struct BedrockStreamState {
    model: String,
    decoder: BoundedSseDecoder,
    outputs: VecDeque<Bytes>,
    output_bytes: usize,
    started: bool,
    text_started: bool,
    closed_tool_blocks: BTreeSet<usize>,
    pending_tools: HashMap<usize, PendingBedrockToolCall>,
    tool_state_bytes: usize,
    latest_usage: Option<TokenUsage>,
}

impl BedrockStreamState {
    fn new(model: String) -> Self {
        Self {
            model,
            decoder: BoundedSseDecoder::new(MAX_TRANSLATED_SSE_FRAME_BYTES),
            outputs: VecDeque::new(),
            output_bytes: 0,
            started: false,
            text_started: false,
            closed_tool_blocks: BTreeSet::new(),
            pending_tools: HashMap::new(),
            tool_state_bytes: 0,
            latest_usage: None,
        }
    }

    fn pop_output(&mut self) -> Option<Bytes> {
        let output = self.outputs.pop_front()?;
        self.output_bytes -= output.len();
        Some(output)
    }

    fn handle_frame(
        &mut self,
        frame: crate::protocol::sse_parse::SseFrame,
    ) -> Result<(), ProtocolStreamError> {
        if frame.data == "[DONE]" {
            return Ok(());
        }
        let Ok(raw) = serde_json::from_str::<Value>(&frame.data) else {
            return Ok(());
        };
        let choice = raw
            .get("choices")
            .and_then(|value| value.as_array())
            .and_then(|value| value.first());

        if let Some(usage) = raw.get("usage") {
            self.latest_usage = Some(TokenUsage {
                prompt_tokens: usage
                    .get("prompt_tokens")
                    .and_then(|value| value.as_u64())
                    .unwrap_or(0),
                completion_tokens: usage
                    .get("completion_tokens")
                    .and_then(|value| value.as_u64())
                    .unwrap_or(0),
                total_tokens: usage
                    .get("total_tokens")
                    .and_then(|value| value.as_u64())
                    .unwrap_or(0),
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            });
        }

        if let Some(text) = choice
            .and_then(|value| value.get("delta"))
            .and_then(|value| value.get("content"))
            .and_then(|value| value.as_str())
        {
            self.ensure_message_started()?;
            if !self.text_started {
                self.text_started = true;
            }
            self.emit_event(json!({
                "contentBlockDelta": {
                    "contentBlockIndex": 0,
                    "delta": { "text": text }
                }
            }))?;
        }

        if let Some(tool_calls) = choice
            .and_then(|value| value.get("delta"))
            .and_then(|value| value.get("tool_calls"))
            .and_then(|value| value.as_array())
        {
            for (fallback_index, tool_call) in tool_calls.iter().enumerate() {
                self.handle_tool_call_delta(tool_call, fallback_index)?;
            }
        }

        if let Some(reason) = choice
            .and_then(|value| value.get("finish_reason"))
            .and_then(|value| value.as_str())
        {
            self.emit_event(json!({
                "messageStop": {
                    "stopReason": map_bedrock_finish_reason(Some(reason), &[])
                }
            }))?;
            if let Some(usage) = &self.latest_usage {
                self.emit_event(json!({
                    "metadata": {
                        "usage": {
                            "inputTokens": usage.prompt_tokens,
                            "outputTokens": usage.completion_tokens,
                            "totalTokens": usage.total_tokens
                        }
                    }
                }))?;
            }
        }
        Ok(())
    }

    fn handle_tool_call_delta(
        &mut self,
        tool_call: &Value,
        fallback_index: usize,
    ) -> Result<(), ProtocolStreamError> {
        let index = tool_call
            .get("index")
            .and_then(|value| value.as_u64())
            .map(usize::try_from)
            .transpose()
            .map_err(|_| stream_error("Bedrock tool index does not fit the output field"))?
            .unwrap_or(fallback_index);
        let wire_index = i64::try_from(index)
            .map_err(|_| stream_error("Bedrock tool index does not fit the output field"))?;
        let id = tool_call.get("id").and_then(|value| value.as_str());
        let name = tool_call
            .get("function")
            .and_then(|value| value.get("name"))
            .and_then(|value| value.as_str());
        self.admit_tool_delta(index, id, name)?;
        self.ensure_message_started()?;
        let mut start_payload = None;
        let mut delta_payload = None;

        {
            let entry = self
                .pending_tools
                .entry(index)
                .or_insert_with(|| PendingBedrockToolCall {
                    id: id
                        .map(str::to_string)
                        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                    name: String::new(),
                });

            if let Some(id) = id {
                if !id.trim().is_empty() {
                    entry.id = id.to_string();
                }
            }

            if let Some(name) = name {
                if !name.trim().is_empty() {
                    entry.name = name.to_string();
                }
            }

            if !self.closed_tool_blocks.contains(&index) && !entry.name.is_empty() {
                start_payload = Some(json!({
                    "contentBlockStart": {
                        "contentBlockIndex": wire_index,
                        "start": {
                            "toolUse": {
                                "toolUseId": entry.id,
                                "name": entry.name
                            }
                        }
                    }
                }));
            }

            // Argument fragments retain whitespace and empty strings verbatim.
            if let Some(arguments) = tool_call
                .get("function")
                .and_then(|value| value.get("arguments"))
                .and_then(|value| value.as_str())
            {
                delta_payload = Some(json!({
                    "contentBlockDelta": {
                        "contentBlockIndex": wire_index,
                        "delta": {
                            "toolUse": {
                                "input": arguments
                            }
                        }
                    }
                }));
            }
        }

        if let Some(payload) = start_payload {
            self.emit_event(payload)?;
            self.closed_tool_blocks.insert(index);
        }
        if let Some(payload) = delta_payload {
            self.emit_event(payload)?;
        }
        Ok(())
    }

    fn admit_tool_delta(
        &mut self,
        index: usize,
        id: Option<&str>,
        name: Option<&str>,
    ) -> Result<(), ProtocolStreamError> {
        let existing = self.pending_tools.get(&index);
        if existing.is_none() && self.pending_tools.len() >= MAX_TOOL_CALLS {
            return Err(stream_error("Bedrock tool call limit exceeded"));
        }
        let previous_bytes = existing.map_or(0, |entry| entry.id.len() + entry.name.len());
        let id_bytes = id
            .filter(|id| existing.is_none() || !id.trim().is_empty())
            .map(str::len)
            // The generated fallback is a 36-byte hyphenated UUID.
            .unwrap_or_else(|| existing.map_or(36, |entry| entry.id.len()));
        let name_bytes = name
            .filter(|name| !name.trim().is_empty())
            .map(str::len)
            .unwrap_or_else(|| existing.map_or(0, |entry| entry.name.len()));
        self.tool_state_bytes = (self.tool_state_bytes - previous_bytes)
            .checked_add(id_bytes)
            .and_then(|bytes| bytes.checked_add(name_bytes))
            .filter(|bytes| *bytes <= MAX_TOOL_STATE_BYTES)
            .ok_or_else(|| stream_error("Bedrock tool state byte limit exceeded"))?;
        Ok(())
    }

    fn ensure_message_started(&mut self) -> Result<(), ProtocolStreamError> {
        if self.started {
            return Ok(());
        }
        self.started = true;
        self.emit_event(json!({
            "messageStart": {
                "model": self.model
            }
        }))
    }

    fn emit_event(&mut self, payload: Value) -> Result<(), ProtocolStreamError> {
        if self.outputs.len() >= MAX_OUTPUT_EVENTS {
            return Err(stream_error("Bedrock output event limit exceeded"));
        }
        let frame = encode_aws_eventstream_json(&payload)?;
        self.output_bytes = self
            .output_bytes
            .checked_add(frame.len())
            .filter(|bytes| *bytes <= MAX_OUTPUT_BYTES)
            .ok_or_else(|| stream_error("Bedrock output bytes exceeded the queue limit"))?;
        self.outputs.push_back(frame);
        Ok(())
    }
}

fn encode_aws_eventstream_json(payload: &Value) -> Result<Bytes, ProtocolStreamError> {
    static CRC32: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

    let payload = serde_json::to_vec(payload).map_err(ProtocolStreamError::from)?;
    let headers_len = 0u32;
    let total_len = payload
        .len()
        .checked_add(16)
        .filter(|length| *length <= MAX_OUTPUT_BYTES)
        .and_then(|length| u32::try_from(length).ok())
        .ok_or_else(|| stream_error("Bedrock output bytes exceeded the frame limit"))?;

    let mut frame = Vec::new();
    frame
        .try_reserve_exact(total_len as usize)
        .map_err(|_| stream_error("failed to reserve Bedrock output bytes"))?;
    frame.extend_from_slice(&total_len.to_be_bytes());
    frame.extend_from_slice(&headers_len.to_be_bytes());

    let prelude_crc = CRC32.checksum(&frame);
    frame.extend_from_slice(&prelude_crc.to_be_bytes());
    frame.extend_from_slice(&payload);

    let message_crc = CRC32.checksum(&frame);
    frame.extend_from_slice(&message_crc.to_be_bytes());
    Ok(Bytes::from(frame))
}

fn stream_error(message: &'static str) -> ProtocolStreamError {
    ProtocolStreamError::invalid_data(message)
}
