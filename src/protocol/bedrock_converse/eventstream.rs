//! Lazy OpenAI-to-native-AWS projection. Completion is validated before tracking commits EOF.
mod frame;
mod native;
mod state;
mod tools;
mod usage;

use crate::protocol::stream_decode::{
    BoundedSseDecoder, DecodeStep, MAX_TRANSLATED_SSE_FRAME_BYTES,
};
use crate::protocol::stream_error::{ProtocolStreamError, StreamError};
use crate::protocol::stream_error_legacy::with_transport_error;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

type UsageHandle = Arc<Mutex<Option<crate::protocol::canonical::TokenUsage>>>;

/// Project an already validated terminal text response through the shared AWS encoder.
pub(crate) fn encode_buffered_text(
    value: &serde_json::Value,
) -> Result<Vec<Bytes>, ProtocolStreamError> {
    use serde_json::json;
    let text = value
        .pointer("/output/message/content/0/text")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| invalid("missing terminal text"))?;
    if value.get("usage").is_none() || value.get("metrics").is_none() {
        return Err(invalid("missing terminal usage or metrics"));
    }
    let mut output = frame::Output::default();
    output.emit("messageStart", json!({"role":"assistant"}))?;
    output.delta(0, false, text)?;
    output.emit("contentBlockStop", json!({"contentBlockIndex":0}))?;
    output.emit("messageStop", json!({"stopReason":"end_turn"}))?;
    output.emit(
        "metadata",
        json!({"usage":value["usage"],"metrics":value["metrics"]}),
    )?;
    let mut frames = Vec::new();
    while let Some(frame) = output.pop() {
        frames.push(frame);
    }
    Ok(frames)
}

pub(crate) fn encode_failure(
    error: &ProtocolStreamError,
) -> Result<Option<Bytes>, ProtocolStreamError> {
    match error.gateway_error() {
        Some(error) => frame::failure(error),
        None => Ok(None),
    }
}

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
    translate_with_started_at(inner, model, Instant::now()).0
}

pub(crate) fn translate_with_started_at<E>(
    inner: impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    _model: String,
    started_at: Instant,
) -> (
    impl Stream<Item = Result<Bytes, E>> + Send + 'static,
    UsageHandle,
)
where
    E: From<ProtocolStreamError> + Send + 'static,
{
    let usage = Arc::new(Mutex::new(None));
    let captured = Arc::clone(&usage);
    let stream = futures::stream::unfold(
        Some((
            Some(Box::pin(inner)),
            BoundedSseDecoder::new_strict(MAX_TRANSLATED_SSE_FRAME_BYTES),
            state::State::new(started_at, captured),
        )),
        |owned| async move {
            let (mut inner, mut decoder, mut state) = owned?;
            loop {
                if let Some(output) = state.output.pop() {
                    return Some((Ok(output), Some((inner, decoder, state))));
                }
                if inner.is_none() {
                    return None;
                }
                match decoder.decode_next() {
                    DecodeStep::Frame(frame) => {
                        if let Err(error) = state.observe(frame) {
                            return Some((Err(error.into()), None));
                        }
                        continue;
                    }
                    DecodeStep::Error(error) => return Some((Err(error.into()), None)),
                    DecodeStep::NeedInput => {}
                }
                match inner.as_mut().unwrap().next().await {
                    Some(Ok(bytes)) => decoder.push_chunk(bytes),
                    Some(Err(error)) => return Some((Err(error), None)),
                    None => {
                        // Drop the source before delivering either terminal output or failure.
                        inner = None;
                        if let Err(error) = decoder.finish_strict().and_then(|()| state.finish()) {
                            return Some((Err(error.into()), None));
                        }
                    }
                }
            }
        },
    );
    (stream, usage)
}

fn invalid(message: &'static str) -> ProtocolStreamError {
    ProtocolStreamError::invalid_data(message)
}
