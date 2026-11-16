//! Text remains incremental; function fragments are bounded until a verified terminal.
//! Public translators must never invent an ID/name when the provider sends them in separate frames.
use crate::protocol::{
    openai,
    stream_decode::{BoundedSseDecoder, DecodeStep},
    stream_error::ProtocolStreamError,
};
use bytes::Bytes;
use futures::{Stream, StreamExt};
use serde_json::{json, Value};
use std::collections::{BTreeMap, VecDeque};
const LIMIT: usize = 16 * 1024 * 1024;

pub(crate) fn validated<S, E>(input: S) -> impl Stream<Item = Result<Bytes, E>> + Send
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: From<ProtocolStreamError> + Send + 'static,
{
    with_settings(input, LIMIT, None, false)
}
pub(crate) fn with_model<S, E>(
    input: S,
    model: String,
) -> impl Stream<Item = Result<Bytes, E>> + Send
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: From<ProtocolStreamError> + Send + 'static,
{
    with_settings(input, LIMIT, Some(model), false)
}
/// Only for our strictly admitted Messages JSON/converter output. Its separate cache counters
/// are not an ordinary Chat wire dialect; native validation already owns terminal/error admission.
pub(crate) fn from_messages<S, E>(
    input: S,
    model: Option<String>,
) -> impl Stream<Item = Result<Bytes, E>> + Send
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: From<ProtocolStreamError> + Send + 'static,
{
    with_settings(input, LIMIT, model, true)
}
#[cfg(test)]
fn with_limit<S, E>(input: S, limit: usize) -> impl Stream<Item = Result<Bytes, E>> + Send
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: From<ProtocolStreamError> + Send + 'static,
{
    with_settings(input, limit, None, false)
}
fn with_settings<S, E>(
    input: S,
    limit: usize,
    model: Option<String>,
    messages_admitted: bool,
) -> impl Stream<Item = Result<Bytes, E>> + Send
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: From<ProtocolStreamError> + Send + 'static,
{
    let guarded: std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, E>> + Send>> =
        if messages_admitted {
            Box::pin(input)
        } else {
            Box::pin(openai::chat_stream::require_terminal(
                input,
                super::stream_error,
            ))
        };
    futures::stream::unfold(
        (
            Some(guarded),
            Tools {
                limit,
                model,
                messages_admitted,
                ..Tools::default()
            },
            VecDeque::new(),
        ),
        |(mut stream, mut tools, mut ready)| async move {
            if stream.is_none() {
                return None;
            }
            loop {
                if let Some(bytes) = ready.pop_front() {
                    return Some((Ok(bytes), (stream, tools, ready)));
                }
                let bytes = match stream.as_mut().unwrap().next().await {
                    Some(Ok(bytes)) => bytes,
                    Some(Err(error)) => {
                        return Some((Err(error), (None, Tools::default(), VecDeque::new())))
                    }
                    None => return None,
                };
                if tools.accept(bytes, &mut ready).is_err() {
                    return Some((
                        Err(super::stream_error().into()),
                        (None, Tools::default(), VecDeque::new()),
                    ));
                }
            }
        },
    )
}
#[derive(Default)]
struct Tools {
    values: BTreeMap<u64, Value>,
    bytes: usize,
    limit: usize,
    usage: Option<Value>,
    model: Option<String>,
    messages_admitted: bool,
}
impl Tools {
    fn accept(&mut self, bytes: Bytes, ready: &mut VecDeque<Bytes>) -> Result<(), ()> {
        // The upstream guard emits complete sanitized frames, occasionally finish + DONE together.
        let mut decoder = BoundedSseDecoder::new_strict(64 * 1024 * 1024);
        decoder.push_chunk(bytes);
        loop {
            let frame = match decoder.decode_next() {
                DecodeStep::Frame(f) => f,
                DecodeStep::NeedInput => break,
                DecodeStep::Error(_) => return Err(()),
            };
            if frame.data == "[DONE]" {
                if let Some(usage) = self.usage.take() {
                    ready.push_back(encode(&json!({"choices":[],"usage":usage})));
                }
                ready.push_back(Bytes::from_static(b"data: [DONE]\n\n"));
                continue;
            }
            let mut value: Value = serde_json::from_str(&frame.data).map_err(|_| ())?;
            if let Some(usage) = value.as_object_mut().ok_or(())?.remove("usage") {
                self.usage = Some(usage);
            }
            if let Some(model) = &self.model {
                value["model"] = model.clone().into();
            }
            for choice in value["choices"].as_array_mut().ok_or(())? {
                if choice["index"] != 0 {
                    return Err(());
                }
                if choice["delta"]
                    .get("function_call")
                    .is_some_and(|v| !v.is_null())
                {
                    return Err(());
                }
                if let Some(calls) = choice["delta"]
                    .as_object_mut()
                    .ok_or(())?
                    .remove("tool_calls")
                {
                    if !calls.is_null() {
                        self.extend(calls.as_array().ok_or(())?)?;
                    }
                }
                if self.messages_admitted && choice["finish_reason"].is_string() {
                    if choice["finish_reason"] == "tool_calls" && self.values.is_empty() {
                        return Err(());
                    }
                    for tool in self.values.values() {
                        if tool["id"].as_str().is_none_or(|s| s.is_empty())
                            || tool["function"]["name"]
                                .as_str()
                                .is_none_or(|s| s.trim().is_empty())
                        {
                            return Err(());
                        }
                        let args = tool["function"]["arguments"].as_str().ok_or(())?;
                        if !crate::protocol::unique_json::parse(args.as_bytes())
                            .map_err(|_| ())?
                            .is_object()
                        {
                            return Err(());
                        }
                    }
                }
                if choice["finish_reason"].is_string() && !self.values.is_empty() {
                    let calls: Vec<_> = std::mem::take(&mut self.values).into_values().collect();
                    let mut delta = choice.clone();
                    delta["finish_reason"] = Value::Null;
                    delta["delta"] = json!({"tool_calls":calls});
                    let mut frame = json!({"choices":[delta]});
                    if let Some(model) = &self.model {
                        frame["model"] = model.clone().into();
                    }
                    ready.push_back(encode(&frame));
                    self.bytes = 0;
                }
            }
            if value["choices"].as_array().is_some_and(|v| !v.is_empty()) {
                ready.push_back(encode(&value));
            }
        }
        decoder.finish_strict().map_err(|_| ())
    }
    fn extend(&mut self, calls: &[Value]) -> Result<(), ()> {
        for call in calls {
            let index = call["index"].as_u64().ok_or(())?;
            let value = self.values.entry(index).or_insert_with(|| json!({"index":index,"id":"","type":"function","function":{"name":"","arguments":""}}));
            for path in ["/id", "/function/name", "/function/arguments"] {
                if let Some(text) = call.pointer(path).and_then(Value::as_str) {
                    self.bytes = self
                        .bytes
                        .checked_add(text.len())
                        .filter(|n| *n <= self.limit)
                        .ok_or(())?;
                    let Some(Value::String(target)) = value.pointer_mut(path) else {
                        return Err(());
                    };
                    target.push_str(text);
                }
            }
        }
        Ok(())
    }
}
fn encode(value: &Value) -> Bytes {
    Bytes::from(format!("data: {value}\n\n"))
}

#[cfg(test)]
#[path = "stream_tests.rs"]
mod tests;
