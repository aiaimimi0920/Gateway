//! A native delta is a union; redacted fragments are bytes, not concatenated Base64 strings.
use super::invalid;
use crate::protocol::stream_error::ProtocolStreamError;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::Value;

pub(super) enum Delta {
    Text(String),
    Signature(String),
    Redacted(Vec<u8>),
}
pub(super) fn parse(value: &Value) -> Result<Delta, ProtocolStreamError> {
    let object = value
        .as_object()
        .filter(|o| o.len() == 1)
        .ok_or_else(|| invalid("invalid reasoning delta union"))?;
    let (key, value) = object.iter().next().expect("one union member");
    let text = value
        .as_str()
        .ok_or_else(|| invalid("invalid reasoning delta type"))?;
    if text.len() > crate::protocol::bedrock_turn::MAX_TURN_BYTES {
        return Err(invalid("reasoning delta exceeds retained-byte limit"));
    }
    match key.as_str() {
        "text" => Ok(Delta::Text(text.into())),
        "signature" => Ok(Delta::Signature(text.into())),
        "redactedContent" => STANDARD
            .decode(text)
            .map(Delta::Redacted)
            .map_err(|_| invalid("invalid redacted reasoning bytes")),
        _ => Err(invalid("unsupported reasoning delta member")),
    }
}
