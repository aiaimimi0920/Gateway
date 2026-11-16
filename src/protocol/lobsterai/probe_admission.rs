//! Probe-only checks run before canonical decoding discards choice count and refusal metadata.
use super::response_error;
use crate::{
    error::GatewayError,
    protocol::stream_decode::{BoundedSseDecoder, DecodeStep},
};
use serde_json::Value;
pub(super) const LIMIT: usize = 65_536;

pub(super) fn chat(bytes: &[u8], sse: bool) -> Result<(), GatewayError> {
    if !sse {
        let body = crate::protocol::unique_json::parse(bytes).map_err(|_| response_error())?;
        return choices(&body, false);
    }
    // The existing Lobster stream owner still validates EOF, errors and terminal semantics.
    // This bounded pre-pass only checks metadata that that projection intentionally omits.
    let mut decoder = BoundedSseDecoder::new_strict(LIMIT);
    decoder.push_chunk(bytes::Bytes::copy_from_slice(bytes));
    loop {
        match decoder.decode_next() {
            DecodeStep::Frame(frame) if frame.data == "[DONE]" => {}
            DecodeStep::Frame(frame) => {
                let value = crate::protocol::unique_json::parse(frame.data.as_bytes())
                    .map_err(|_| response_error())?;
                choices(&value, true)?;
            }
            DecodeStep::NeedInput => break,
            DecodeStep::Error(_) => return Err(response_error()),
        }
    }
    decoder.finish_strict().map_err(|_| response_error())
}
fn choices(value: &Value, sse: bool) -> Result<(), GatewayError> {
    if sse && value.get("choices").is_none() {
        return Ok(());
    }
    let choices = value["choices"].as_array().ok_or_else(response_error)?;
    if choices.len() > 1 || (!sse && choices.len() != 1) {
        return Err(response_error());
    }
    for choice in choices {
        for field in ["message", "delta"] {
            if choice[field].get("refusal").is_some_and(|v| !v.is_null()) {
                return Err(response_error());
            }
        }
    }
    Ok(())
}
