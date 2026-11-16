//! Strict Messages-to-Chat conversion for resolved models. Terminal success waits for clean EOF.
use crate::protocol::{
    stream_decode::{BoundedSseDecoder, DecodeStep},
    stream_error::ProtocolStreamError,
};
use bytes::Bytes;
use futures::{Stream, StreamExt};
use serde_json::{json, Value};
use std::collections::{BTreeMap, VecDeque};
const LIMIT: usize = 16 * 1024 * 1024;
pub(super) fn translate<S, E>(
    input: S,
    model: String,
) -> impl Stream<Item = Result<Bytes, E>> + Send
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: From<ProtocolStreamError> + Send + 'static,
{
    futures::stream::unfold(
        (
            Some(Box::pin(input)
                as std::pin::Pin<
                    Box<dyn Stream<Item = Result<Bytes, E>> + Send>,
                >),
            State::new(model),
        ),
        |(mut input, mut state)| async move {
            if input.is_none() {
                return None;
            }
            let mut steps = 0usize;
            loop {
                steps += 1;
                if steps % 32 == 0 {
                    tokio::task::yield_now().await;
                }
                if let Some(bytes) = state.ready.pop_front() {
                    return Some((Ok(bytes), (input, state)));
                }
                if state.eof {
                    return None;
                }
                match state.decoder.decode_next() {
                    DecodeStep::Frame(frame) => {
                        if state.accept(frame).is_err() {
                            return Some((
                                Err(super::stream_error().into()),
                                (None, State::new(String::new())),
                            ));
                        }
                        continue;
                    }
                    DecodeStep::Error(_) => {
                        return Some((
                            Err(super::stream_error().into()),
                            (None, State::new(String::new())),
                        ))
                    }
                    DecodeStep::NeedInput => {}
                }
                match input.as_mut().unwrap().next().await {
                    Some(Ok(bytes)) => state.decoder.push_chunk(bytes),
                    Some(Err(error)) => {
                        return Some((Err(error), (None, State::new(String::new()))))
                    }
                    None => {
                        if !state.stopped || state.decoder.finish_strict().is_err() {
                            return Some((
                                Err(super::stream_error().into()),
                                (None, State::new(String::new())),
                            ));
                        }
                        // Release the transport before returning any terminal frame.
                        input = Some(Box::pin(futures::stream::empty())
                            as std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, E>> + Send>>);
                        state.eof = true;
                        let finish = state.finish.take().unwrap();
                        state.emit(json!({}), Some(&finish));
                        if let (Some(prompt), Some(completion)) = (state.prompt, state.completion) {
                            let Some(total) = prompt.checked_add(completion) else {
                                return Some((
                                    Err(super::stream_error().into()),
                                    (None, State::new(String::new())),
                                ));
                            };
                            let mut usage = json!({"prompt_tokens":prompt,"completion_tokens":completion,"total_tokens":total});
                            if let Some(n) = state.cache_read {
                                usage["cache_read_input_tokens"] = n.into();
                            }
                            if let Some(n) = state.cache_creation {
                                usage["cache_creation_input_tokens"] = n.into();
                            }
                            state
                                .ready
                                .push_back(encode(json!({"choices":[],"usage":usage})));
                        }
                        state
                            .ready
                            .push_back(Bytes::from_static(b"data: [DONE]\n\n"));
                    }
                }
            }
        },
    )
}
struct State {
    decoder: BoundedSseDecoder,
    model: String,
    ready: VecDeque<Bytes>,
    started: bool,
    stopped: bool,
    eof: bool,
    finish: Option<String>,
    blocks: BTreeMap<u64, (&'static str, bool)>,
    next_index: u64,
    prompt: Option<u64>,
    completion: Option<u64>,
    cache_read: Option<u64>,
    cache_creation: Option<u64>,
}
impl State {
    fn new(model: String) -> Self {
        Self {
            decoder: BoundedSseDecoder::new_strict(LIMIT),
            model,
            ready: VecDeque::new(),
            started: false,
            stopped: false,
            eof: false,
            finish: None,
            blocks: BTreeMap::new(),
            next_index: 0,
            prompt: None,
            completion: None,
            cache_read: None,
            cache_creation: None,
        }
    }
    fn emit(&mut self, delta: Value, finish: Option<&str>) {
        self.ready.push_back(encode(json!({"model":self.model,"choices":[{"index":0,"delta":delta,"finish_reason":finish}]})));
    }
    fn usage(&mut self, value: &Value) -> Result<(), ()> {
        for (key, target) in [
            ("input_tokens", &mut self.prompt),
            ("output_tokens", &mut self.completion),
            ("cache_read_input_tokens", &mut self.cache_read),
            ("cache_creation_input_tokens", &mut self.cache_creation),
        ] {
            if let Some(value) = value.get(key) {
                *target = Some(value.as_u64().ok_or(())?);
            }
        }
        Ok(())
    }
    fn accept(&mut self, frame: crate::protocol::sse_parse::SseFrame) -> Result<(), ()> {
        if frame.data.is_empty() {
            return Ok(());
        }
        let value = crate::protocol::unique_json::parse(frame.data.as_bytes()).map_err(|_| ())?;
        let kind = value["type"].as_str().ok_or(())?;
        if frame.event_name.as_deref().is_some_and(|name| name != kind) || !value["error"].is_null()
        {
            return Err(());
        }
        if kind == "ping" && !self.stopped {
            return Ok(());
        }
        if self.stopped {
            return Err(());
        }
        match kind {
            "message_start"
                if !self.started
                    && value["message"]["type"] == "message"
                    && value["message"]["role"] == "assistant"
                    && value["message"]["content"]
                        .as_array()
                        .is_some_and(Vec::is_empty) =>
            {
                self.started = true;
                self.usage(&value["message"]["usage"])?;
                // message_start output is provisional; only the final delta can settle usage.
                self.completion = None;
            }
            "content_block_start" if self.started && self.finish.is_none() => {
                let index = value["index"].as_u64().ok_or(())?;
                if index != self.next_index || index >= 4096 {
                    return Err(());
                }
                self.next_index += 1;
                let block = &value["content_block"];
                let kind = match block["type"].as_str() {
                    Some("text") => {
                        let text = block["text"].as_str().ok_or(())?;
                        self.emit(json!({"content":text}), None);
                        ("text", false)
                    }
                    Some("tool_use") => {
                        let id = block["id"].as_str().filter(|s| !s.is_empty()).ok_or(())?;
                        let name = block["name"].as_str().filter(|s| !s.is_empty()).ok_or(())?;
                        let args = block["input"].as_object().ok_or(())?;
                        let args = if args.is_empty() {
                            String::new()
                        } else {
                            block["input"].to_string()
                        };
                        self.emit(json!({"tool_calls":[{"index":index,"id":id,"type":"function","function":{"name":name,"arguments":args}}]}),None);
                        ("tool_use", !args.is_empty())
                    }
                    _ => return Err(()),
                };
                self.blocks.insert(index, kind);
            }
            "content_block_delta" if self.finish.is_none() => {
                let index = value["index"].as_u64().ok_or(())?;
                match (
                    self.blocks.get(&index).copied(),
                    value["delta"]["type"].as_str(),
                ) {
                    (Some(("text", _)), Some("text_delta")) => self.emit(
                        json!({"content":value["delta"]["text"].as_str().ok_or(())?}),
                        None,
                    ),
                    (Some(("tool_use", _)), Some("input_json_delta")) => {
                        let args = value["delta"]["partial_json"].as_str().ok_or(())?;
                        if !args.is_empty() {
                            self.blocks.get_mut(&index).unwrap().1 = true;
                        }
                        self.emit(
                            json!({"tool_calls":[{"index":index,"function":{"arguments":args}}]}),
                            None,
                        );
                    }
                    _ => return Err(()),
                }
            }
            "content_block_stop" if self.finish.is_none() => {
                let index = value["index"].as_u64().ok_or(())?;
                if self.blocks.remove(&index).ok_or(())? == ("tool_use", false) {
                    self.emit(
                        json!({"tool_calls":[{"index":index,"function":{"arguments":"{}"}}]}),
                        None,
                    );
                }
            }
            "message_delta" if self.started && self.blocks.is_empty() && self.finish.is_none() => {
                self.finish = Some(
                    match value["delta"]["stop_reason"].as_str() {
                        Some("end_turn" | "stop_sequence") => "stop",
                        Some("max_tokens") => "length",
                        Some("tool_use") => "tool_calls",
                        _ => return Err(()),
                    }
                    .into(),
                );
                self.usage(&value["usage"])?;
            }
            "message_stop" if self.finish.is_some() => self.stopped = true,
            _ => return Err(()),
        }
        Ok(())
    }
}
fn encode(value: Value) -> Bytes {
    Bytes::from(format!("data: {value}\n\n"))
}
