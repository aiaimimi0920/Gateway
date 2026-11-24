//! Validate native Messages before publication; release terminal frames only after clean EOF.
use crate::protocol::{
    stream_decode::{BoundedSseDecoder, DecodeStep, MAX_TRANSLATED_SSE_FRAME_BYTES},
    stream_error::ProtocolStreamError,
};
use bytes::Bytes;
use futures::{Stream, StreamExt};
use std::collections::{BTreeMap, VecDeque};
#[path = "native_stream_blocks.rs"]
mod blocks;
use blocks::Block;

pub(crate) fn guard<S, E>(
    input: S,
    model: String,
    error: fn() -> ProtocolStreamError,
    require_thinking_signature: bool,
) -> impl Stream<Item = Result<Bytes, E>> + Send
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: From<ProtocolStreamError> + Send + 'static,
{
    futures::stream::unfold(
        (
            Some(Box::pin(input)),
            State::new(model, require_thinking_signature),
        ),
        move |(mut input, mut state)| async move {
            if state.eof {
                return state.held.pop_front().map(|b| (Ok(b), (input, state)));
            }
            input.as_ref()?;
            let mut steps = 0usize;
            loop {
                steps += 1;
                if steps % 32 == 0 {
                    tokio::task::yield_now().await;
                }
                match state.decoder.decode_next() {
                    DecodeStep::Frame(frame) => match state.accept(frame) {
                        Ok(Some(bytes)) => return Some((Ok(bytes), (input, state))),
                        Ok(None) => continue,
                        Err(()) => {
                            return Some((
                                Err(error().into()),
                                (None, State::new(String::new(), require_thinking_signature)),
                            ))
                        }
                    },
                    DecodeStep::Error(_) => {
                        return Some((
                            Err(error().into()),
                            (None, State::new(String::new(), require_thinking_signature)),
                        ))
                    }
                    DecodeStep::NeedInput => {}
                }
                match input.as_mut().unwrap().next().await {
                    Some(Ok(bytes)) => state.decoder.push_chunk(bytes),
                    Some(Err(_)) => {
                        return Some((
                            Err(error().into()),
                            (None, State::new(String::new(), require_thinking_signature)),
                        ))
                    }
                    None => {
                        if !state.stopped || state.decoder.finish_strict().is_err() {
                            return Some((
                                Err(error().into()),
                                (None, State::new(String::new(), require_thinking_signature)),
                            ));
                        }
                        state.decoder.clear();
                        state.eof = true;
                        return state.held.pop_front().map(|b| (Ok(b), (None, state)));
                    }
                }
            }
        },
    )
}

struct State {
    decoder: BoundedSseDecoder,
    model: String,
    require_thinking_signature: bool,
    started: bool,
    finishing: bool,
    stopped: bool,
    eof: bool,
    blocks: BTreeMap<u64, Block>,
    argument_bytes: usize,
    next_index: u64,
    saw_tool: bool,
    prompt: Option<u64>,
    output: Option<u64>,
    terminal_reason: Option<String>,
    held: VecDeque<Bytes>,
    held_bytes: usize,
}
impl State {
    fn new(model: String, require_thinking_signature: bool) -> Self {
        Self {
            decoder: BoundedSseDecoder::new_strict(MAX_TRANSLATED_SSE_FRAME_BYTES),
            model,
            require_thinking_signature,
            started: false,
            finishing: false,
            stopped: false,
            eof: false,
            blocks: BTreeMap::new(),
            argument_bytes: 0,
            next_index: 0,
            saw_tool: false,
            prompt: None,
            output: None,
            terminal_reason: None,
            held: VecDeque::new(),
            held_bytes: 0,
        }
    }
    fn accept(&mut self, frame: crate::protocol::sse_parse::SseFrame) -> Result<Option<Bytes>, ()> {
        if frame.data.is_empty() {
            return Ok(None);
        }
        let mut value =
            crate::protocol::unique_json::parse(frame.data.as_bytes()).map_err(|_| ())?;
        let kind = value["type"].as_str().ok_or(())?.to_owned();
        // The native type is also emitted as an SSE field, not just JSON string data.
        if kind.contains(['\r', '\n'])
            || frame.event_name.as_deref().is_some_and(|name| name != kind)
            || !value["error"].is_null()
            || !value["message"]["error"].is_null()
            || self.stopped
        {
            return Err(());
        }
        if kind == "ping" && !self.started {
            return Ok(None);
        }
        match kind.as_str() {
            "message_start"
                if !self.started
                    && value["message"]["type"] == "message"
                    && value["message"]["role"] == "assistant"
                    && value["message"]["id"]
                        .as_str()
                        .is_some_and(|s| !s.is_empty())
                    && value["message"]["model"]
                        .as_str()
                        .is_some_and(|s| !s.is_empty())
                    && value["message"]["content"]
                        .as_array()
                        .is_some_and(Vec::is_empty) =>
            {
                super::native_admission::usage(&value["message"]["usage"]).map_err(|_| ())?;
                self.prompt = value["message"]["usage"]["input_tokens"].as_u64();
                self.output = value["message"]["usage"]["output_tokens"].as_u64();
                self.started = true;
                // Observation already saw the upstream model; public replies keep the caller alias.
                value["message"]["model"] = self.model.clone().into();
            }
            "content_block_start" if self.started && !self.finishing => {
                let index = value["index"].as_u64().ok_or(())?;
                if index != self.next_index || index >= 4096 {
                    return Err(());
                }
                let block = &value["content_block"];
                let kind = Block::start(block)?;
                self.saw_tool |= block["type"] == "tool_use";
                self.blocks.insert(index, kind);
                self.next_index += 1;
            }
            "content_block_delta" if !self.finishing => {
                let index = value["index"].as_u64().ok_or(())?;
                self.blocks
                    .get_mut(&index)
                    .ok_or(())?
                    .delta(&value["delta"], &mut self.argument_bytes)?;
            }
            "content_block_stop" if !self.finishing => {
                let block = self
                    .blocks
                    .remove(&value["index"].as_u64().ok_or(())?)
                    .ok_or(())?;
                block.finish(&mut self.argument_bytes, self.require_thinking_signature)?;
            }
            "message_delta" if self.started && self.blocks.is_empty() => {
                if !value["delta"].is_object() {
                    return Err(());
                }
                match value["delta"]["stop_reason"].as_str() {
                    Some(
                        "end_turn"
                        | "stop_sequence"
                        | "max_tokens"
                        | "pause_turn"
                        | "refusal"
                        | "model_context_window_exceeded",
                    ) => {}
                    Some("tool_use") if self.saw_tool => {}
                    None if value["delta"]["stop_reason"].is_null() => {}
                    _ => return Err(()),
                }
                if let Some(reason) = value["delta"]["stop_reason"].as_str() {
                    if self
                        .terminal_reason
                        .as_deref()
                        .is_some_and(|prior| prior != reason)
                    {
                        return Err(());
                    }
                    self.terminal_reason = Some(reason.to_owned());
                }
                super::native_admission::usage(&value["usage"]).map_err(|_| ())?;
                self.prompt = value["usage"]["input_tokens"].as_u64().or(self.prompt);
                self.output = value["usage"]["output_tokens"].as_u64().or(self.output);
                if let (Some(prompt), Some(output)) = (self.prompt, self.output) {
                    prompt.checked_add(output).ok_or(())?;
                }
                self.finishing = true;
            }
            "message_stop" if self.finishing && self.terminal_reason.is_some() => {
                self.stopped = true
            }
            "message_start"
            | "content_block_start"
            | "content_block_delta"
            | "content_block_stop"
            | "message_delta"
            | "message_stop"
            | "error" => return Err(()),
            _ if self.started && !kind.is_empty() => {}
            _ => return Err(()),
        }
        // Always emit an event name: the usage tap needs it to identify provisional usage.
        let bytes = Bytes::from(format!("event: {kind}\ndata: {value}\n\n"));
        if self.finishing {
            // Multiple provisional/terminal updates share one bounded clean-EOF buffer.
            self.held_bytes = self
                .held_bytes
                .checked_add(bytes.len())
                .filter(|n| *n <= MAX_TRANSLATED_SSE_FRAME_BYTES)
                .ok_or(())?;
            self.held.push_back(bytes);
            Ok(None)
        } else {
            Ok(Some(bytes))
        }
    }
}

#[test]
fn commandcode_messages_sse_extensions_bound_terminal_buffer() {
    for held_bytes in [MAX_TRANSLATED_SSE_FRAME_BYTES, usize::MAX] {
        let mut state = State::new("public".into(), true);
        state.started = true;
        state.finishing = true;
        state.terminal_reason = Some("end_turn".into());
        state.held_bytes = held_bytes;
        assert!(state
            .accept(crate::protocol::sse_parse::SseFrame {
                event_name: Some("message_stop".into()),
                data: r#"{"type":"message_stop"}"#.into(),
            })
            .is_err());
        assert!(state.held.is_empty());
    }
}
