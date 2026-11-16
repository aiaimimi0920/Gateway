//! A bounded queue of genuine header-tagged AWS frames; no SDK union wrapper on the wire.
use super::invalid;
use crate::protocol::stream_error::ProtocolStreamError;
use bytes::Bytes;
use crc::{Crc, CRC_32_ISO_HDLC};
use serde_json::{json, Value};
use std::collections::VecDeque;

const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;
const MAX_OUTPUT_EVENTS: usize = 2 * super::tools::MAX_TOOLS + 4;
const MAX_SERVICE_PAYLOAD: usize = 24 * 1024 * 1024;
const MAX_SERVICE_HEADERS: usize = 128 * 1024;
// JSON can escape each raw byte into six bytes. Leave room for the delta envelope.
const MAX_DELTA_BYTES: usize = MAX_SERVICE_PAYLOAD / 6 - 1024;
static CRC32: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

#[derive(Default)]
pub(super) struct Output {
    frames: VecDeque<Bytes>,
    bytes: usize,
}

impl Output {
    /// Keep genuine native fragments intact unless the downstream service-size rule requires splitting.
    pub fn native(&mut self, kind: &str, mut body: Value) -> Result<(), ProtocolStreamError> {
        let path = if kind == "contentBlockDelta"
            && body.pointer("/delta/text").is_some_and(Value::is_string)
        {
            "/delta/text"
        } else if kind == "contentBlockDelta"
            && body
                .pointer("/delta/toolUse/input")
                .is_some_and(Value::is_string)
        {
            "/delta/toolUse/input"
        } else {
            return self.emit(kind, body);
        };
        if body.pointer(path).and_then(Value::as_str).unwrap().len() <= MAX_DELTA_BYTES {
            return self.emit(kind, body);
        }
        // Measure actual escaping before changing a legal original fragment's boundaries.
        let oversized = serde_json::to_vec(&body)
            .map_err(ProtocolStreamError::from)?
            .len()
            > MAX_SERVICE_PAYLOAD;
        if !oversized {
            return self.emit(kind, body);
        }
        let Value::String(input) = body.pointer_mut(path).unwrap().take() else {
            unreachable!("validated native string delta")
        };
        let mut input = input.as_str();
        loop {
            let mut end = input.len().min(MAX_DELTA_BYTES);
            while !input.is_char_boundary(end) {
                end -= 1;
            }
            let (part, tail) = input.split_at(end);
            let mut fragment = body.clone();
            *fragment.pointer_mut(path).unwrap() = Value::String(part.to_owned());
            self.emit(kind, fragment)?;
            if tail.is_empty() {
                return Ok(());
            }
            input = tail;
        }
    }

    pub fn delta(
        &mut self,
        index: u32,
        tool: bool,
        mut input: &str,
    ) -> Result<(), ProtocolStreamError> {
        loop {
            let mut end = input.len().min(MAX_DELTA_BYTES);
            while !input.is_char_boundary(end) {
                end -= 1;
            }
            let (part, tail) = input.split_at(end);
            let delta = if tool {
                json!({"toolUse":{"input":part}})
            } else {
                json!({"text":part})
            };
            self.emit(
                "contentBlockDelta",
                json!({"contentBlockIndex":index,"delta":delta}),
            )?;
            if tail.is_empty() {
                return Ok(());
            }
            input = tail;
        }
    }

    pub fn pop(&mut self) -> Option<Bytes> {
        let frame = self.frames.pop_front()?;
        self.bytes -= frame.len();
        Some(frame)
    }

    pub fn emit(&mut self, kind: &str, body: Value) -> Result<(), ProtocolStreamError> {
        if self.frames.len() >= MAX_OUTPUT_EVENTS {
            return Err(invalid("Bedrock output event limit exceeded"));
        }
        let frame = encode(kind, &body)?;
        self.bytes = self
            .bytes
            .checked_add(frame.len())
            .filter(|n| *n <= MAX_OUTPUT_BYTES)
            .ok_or_else(|| invalid("Bedrock output bytes exceeded the queue limit"))?;
        self.frames.push_back(frame);
        Ok(())
    }
}

fn encode(kind: &str, body: &Value) -> Result<Bytes, ProtocolStreamError> {
    encode_headers(
        &[
            (":message-type", "event"),
            (":event-type", kind),
            (":content-type", "application/json"),
        ],
        body,
    )
}

pub(super) fn failure(
    error: &crate::error::GatewayError,
) -> Result<Option<Bytes>, ProtocolStreamError> {
    let Some(exception) = super::super::exception::Exception::from_error(error) else {
        return Ok(None);
    };
    // Only static identity and the owner's safe message are serialized; never raw AWS details.
    encode_headers(
        &[
            (":message-type", "exception"),
            (":exception-type", exception.name),
            (":content-type", "application/json"),
        ],
        &json!({"message":error.message}),
    )
    .map(Some)
}

fn encode_headers(headers: &[(&str, &str)], body: &Value) -> Result<Bytes, ProtocolStreamError> {
    let payload = serde_json::to_vec(body).map_err(ProtocolStreamError::from)?;
    if payload.len() > MAX_SERVICE_PAYLOAD {
        return Err(invalid("Bedrock output payload exceeded the service limit"));
    }
    let mut encoded = Vec::new();
    for &(name, value) in headers {
        let value_len = u16::try_from(value.len())
            .map_err(|_| invalid("Bedrock output header value exceeded the service limit"))?;
        encoded.push(name.len() as u8);
        encoded.extend_from_slice(name.as_bytes());
        encoded.push(7); // AWS string header value.
        encoded.extend_from_slice(&value_len.to_be_bytes());
        encoded.extend_from_slice(value.as_bytes());
    }
    if encoded.len() > MAX_SERVICE_HEADERS {
        return Err(invalid("Bedrock output headers exceeded the service limit"));
    }
    let length = payload
        .len()
        .checked_add(encoded.len())
        .and_then(|n| n.checked_add(16))
        .filter(|n| *n <= MAX_OUTPUT_BYTES)
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| invalid("Bedrock output bytes exceeded the frame limit"))?;
    let mut frame = Vec::new();
    frame
        .try_reserve_exact(length as usize)
        .map_err(|_| invalid("failed to reserve Bedrock output bytes"))?;
    frame.extend_from_slice(&length.to_be_bytes());
    frame.extend_from_slice(&(encoded.len() as u32).to_be_bytes());
    frame.extend_from_slice(&CRC32.checksum(&frame).to_be_bytes());
    frame.extend_from_slice(&encoded);
    frame.extend_from_slice(&payload);
    frame.extend_from_slice(&CRC32.checksum(&frame).to_be_bytes());
    Ok(Bytes::from(frame))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bedrock_service_payload_limit_measures_serialized_bytes_including_escapes() {
        // A JSON string includes two quote bytes; the exact service limit is inclusive.
        assert!(encode(
            "contentBlockDelta",
            &json!("x".repeat(MAX_SERVICE_PAYLOAD - 2))
        )
        .is_ok());
        assert!(encode(
            "contentBlockDelta",
            &json!("x".repeat(MAX_SERVICE_PAYLOAD - 1))
        )
        .is_err());
        assert!(encode(
            "contentBlockDelta",
            &json!("\0".repeat(MAX_SERVICE_PAYLOAD / 6))
        )
        .is_err());
    }
}
