//! Bounded, lazy SSE conversion. Native DashScope events preserve vendor content.
use crate::protocol::stream_decode::{
    BoundedSseDecoder, DecodeStep, MAX_TRANSLATED_SSE_FRAME_BYTES,
};
use crate::protocol::stream_error::ProtocolStreamError;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use serde_json::Value;

pub fn translate<E>(
    source: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    model: String,
    to_dashscope: bool,
    multimodal: bool,
    message_format: bool,
    incremental: bool,
) -> impl Stream<Item = Result<Bytes, E>> + Send + 'static
where
    E: From<ProtocolStreamError> + Send + 'static,
{
    let state = State {
        decoder: BoundedSseDecoder::new(MAX_TRANSLATED_SSE_FRAME_BYTES),
        model,
        to_dashscope,
        multimodal,
        message_format,
        incremental,
        ended: false,
        eof: false,
        finished: false,
        accumulator: super::stream_accumulator::Accumulator::default(),
        last: None,
    };
    futures::stream::unfold(
        (Box::pin(source), state),
        |(mut source, mut state)| async move {
            loop {
                if state.ended {
                    return None;
                }
                if state.eof {
                    state.ended = true;
                    if !state.finished {
                        return Some((
                            Err(ProtocolStreamError::invalid_data(
                                "DashScope bridge stream ended before completion",
                            )
                            .into()),
                            (source, state),
                        ));
                    }
                    if state.to_dashscope {
                        return None;
                    }
                    return Some((Ok(Bytes::from_static(b"data: [DONE]\n\n")), (source, state)));
                }
                match state.decoder.decode_next() {
                    DecodeStep::Frame(frame) => {
                        if frame.data == "[DONE]" {
                            state.ended = true;
                            if state.to_dashscope {
                                return None;
                            }
                            return Some((
                                Ok(Bytes::from_static(b"data: [DONE]\n\n")),
                                (source, state),
                            ));
                        }
                        let result = state.convert(&frame.data).map_err(E::from);
                        if result.is_err() {
                            state.ended = true;
                        }
                        return Some((result, (source, state)));
                    }
                    DecodeStep::Error(error) => {
                        state.ended = true;
                        return Some((Err(error.into()), (source, state)));
                    }
                    DecodeStep::NeedInput => match source.next().await {
                        Some(Ok(chunk)) => state.decoder.push_chunk(chunk),
                        Some(Err(error)) => {
                            state.ended = true;
                            return Some((Err(error), (source, state)));
                        }
                        None => {
                            state.eof = true;
                            if let Some(frame) = state.decoder.flush_pending_frame() {
                                return Some((
                                    state.convert(&frame.data).map_err(E::from),
                                    (source, state),
                                ));
                            }
                        }
                    },
                }
            }
        },
    )
}

struct State {
    decoder: BoundedSseDecoder,
    model: String,
    to_dashscope: bool,
    multimodal: bool,
    message_format: bool,
    incremental: bool,
    ended: bool,
    eof: bool,
    finished: bool,
    accumulator: super::stream_accumulator::Accumulator,
    last: Option<Value>,
}

impl State {
    fn convert(&mut self, data: &str) -> Result<Bytes, ProtocolStreamError> {
        let mut body: Value = serde_json::from_str(data)?;
        if self.to_dashscope {
            self.finished |= body
                .pointer("/output/choices/0/finish_reason")
                .or_else(|| body.pointer("/output/finish_reason"))
                .and_then(Value::as_str)
                .is_some_and(|v| v != "null");
            if body.get("output").is_none() {
                if body.get("error").is_some() {
                    return Err(ProtocolStreamError::invalid_data(
                        "Upstream generation failed",
                    ));
                }
                self.finished |= body
                    .pointer("/choices/0/finish_reason")
                    .and_then(Value::as_str)
                    .is_some();
                if !self.incremental {
                    self.accumulator.apply(&mut body)?;
                } else if body["choices"].as_array().is_some_and(Vec::is_empty) {
                    if let Some(last) = &self.last {
                        let usage = body.get("usage").cloned();
                        body = last.clone();
                        if let Some(usage) = usage {
                            body["usage"] = usage;
                        }
                        if let Some(choices) = body["choices"].as_array_mut() {
                            for choice in choices {
                                choice["delta"] = serde_json::json!({});
                            }
                        }
                    }
                }
                self.last = Some(body.clone());
                body = super::response::from_openai(&body, self.multimodal, self.message_format)
                    .map_err(|e| ProtocolStreamError::invalid_data(e.message))?;
            }
            Ok(Bytes::from(format!("event: result\ndata: {body}\n\n")))
        } else {
            let body = super::response::to_openai(&body, &self.model, true)
                .map_err(|e| ProtocolStreamError::invalid_data(e.message))?;
            self.finished |= body
                .pointer("/choices/0/finish_reason")
                .and_then(Value::as_str)
                .is_some();
            Ok(Bytes::from(format!("data: {body}\n\n")))
        }
    }
}
