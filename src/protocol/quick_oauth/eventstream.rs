//! Clean-room AWS EventStream binary frame codec (`CRC_32_ISO_HDLC`) and AIDP `rawInputEvent`
//! outbound JSON envelope encoder for Amazon Quick WebSocket conversations.

use crate::error::GatewayError;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use crc::{Crc, CRC_32_ISO_HDLC};
use serde_json::{json, Value};
use std::collections::HashMap;

const CRC32: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);
const MIN_FRAME_BYTES: usize = 16;
const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

pub fn encode_outbound_event(event_type: &str, payload: Value) -> Result<String, GatewayError> {
    let inner = serde_json::to_vec(&json!({
        "type": event_type,
        "payload": payload,
    }))
    .map_err(|_| frame_error("quick_oauth_encode_failed"))?;
    let encoded = STANDARD.encode(inner);
    serde_json::to_string(&json!({
        "eventType": "rawInputEvent",
        "payload": {
            "payload": encoded,
        }
    }))
    .map_err(|_| frame_error("quick_oauth_encode_failed"))
}

pub fn decode_inbound_binary(raw: &[u8]) -> Result<Vec<Value>, GatewayError> {
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    let mut offset = 0usize;
    let mut events = Vec::new();
    while offset < raw.len() {
        let rem = &raw[offset..];
        if rem.len() < MIN_FRAME_BYTES {
            return Err(frame_error("quick_oauth_invalid_eventstream"));
        }
        let total_len = u32::from_be_bytes([rem[0], rem[1], rem[2], rem[3]]) as usize;
        let headers_len = u32::from_be_bytes([rem[4], rem[5], rem[6], rem[7]]) as usize;
        let prelude_crc = u32::from_be_bytes([rem[8], rem[9], rem[10], rem[11]]);
        if !(MIN_FRAME_BYTES..=MAX_FRAME_BYTES).contains(&total_len)
            || total_len > rem.len()
            || 12 + headers_len + 4 > total_len
        {
            return Err(frame_error("quick_oauth_invalid_eventstream"));
        }
        if CRC32.checksum(&rem[..8]) != prelude_crc {
            return Err(frame_error("quick_oauth_eventstream_crc_mismatch"));
        }
        let frame = &rem[..total_len];
        let msg_crc_offset = total_len - 4;
        let expected_msg_crc = u32::from_be_bytes([
            frame[msg_crc_offset],
            frame[msg_crc_offset + 1],
            frame[msg_crc_offset + 2],
            frame[msg_crc_offset + 3],
        ]);
        if CRC32.checksum(&frame[..msg_crc_offset]) != expected_msg_crc {
            return Err(frame_error("quick_oauth_eventstream_crc_mismatch"));
        }
        let headers = parse_headers(&frame[12..12 + headers_len])?;
        let payload_bytes = &frame[12 + headers_len..msg_crc_offset];
        if let Some(kind) = headers.get(":message-type").map(String::as_str) {
            if matches!(kind, "error" | "exception") {
                return Err(classify_header_error(&headers));
            }
        }
        if let Some(event) = decode_payload_json(payload_bytes)? {
            events.push(event);
        }
        offset += total_len;
    }
    Ok(events)
}

pub fn decode_inbound_text(raw: &str) -> Result<Option<Value>, GatewayError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    decode_payload_json(trimmed.as_bytes())
}

fn decode_payload_json(payload_bytes: &[u8]) -> Result<Option<Value>, GatewayError> {
    if payload_bytes.is_empty() {
        return Ok(None);
    }
    let outer: Value = serde_json::from_slice(payload_bytes)
        .map_err(|_| frame_error("quick_oauth_invalid_event_payload"))?;
    if let Some(encoded) = outer.get("payload").and_then(Value::as_str) {
        let decoded = STANDARD
            .decode(encoded.trim())
            .map_err(|_| frame_error("quick_oauth_invalid_event_payload"))?;
        let inner: Value = serde_json::from_slice(&decoded)
            .map_err(|_| frame_error("quick_oauth_invalid_event_payload"))?;
        return Ok(Some(inner));
    }
    Ok(Some(outer))
}

fn parse_headers(buf: &[u8]) -> Result<HashMap<String, String>, GatewayError> {
    let mut cursor = 0usize;
    let mut map = HashMap::new();
    while cursor < buf.len() {
        let name_len = *buf
            .get(cursor)
            .ok_or_else(|| frame_error("quick_oauth_invalid_eventstream"))?
            as usize;
        cursor += 1;
        if cursor + name_len >= buf.len() {
            return Err(frame_error("quick_oauth_invalid_eventstream"));
        }
        let name = std::str::from_utf8(&buf[cursor..cursor + name_len])
            .map_err(|_| frame_error("quick_oauth_invalid_eventstream"))?
            .to_owned();
        cursor += name_len;
        let tag = buf[cursor];
        cursor += 1;
        match tag {
            0 => {
                map.insert(name, "true".to_owned());
            }
            1 => {
                map.insert(name, "false".to_owned());
            }
            2 => cursor = skip(buf, cursor, 1)?,
            3 => cursor = skip(buf, cursor, 2)?,
            4 => cursor = skip(buf, cursor, 4)?,
            5 | 8 => cursor = skip(buf, cursor, 8)?,
            6 => {
                let len = read_u16(buf, cursor)?;
                cursor = skip(buf, cursor + 2, len)?;
            }
            7 => {
                let len = read_u16(buf, cursor)?;
                cursor += 2;
                if cursor + len > buf.len() {
                    return Err(frame_error("quick_oauth_invalid_eventstream"));
                }
                let val = std::str::from_utf8(&buf[cursor..cursor + len])
                    .map_err(|_| frame_error("quick_oauth_invalid_eventstream"))?
                    .to_owned();
                cursor += len;
                map.insert(name, val);
            }
            9 => cursor = skip(buf, cursor, 16)?,
            _ => return Err(frame_error("quick_oauth_invalid_eventstream")),
        }
    }
    Ok(map)
}

fn read_u16(buf: &[u8], pos: usize) -> Result<usize, GatewayError> {
    if pos + 2 > buf.len() {
        return Err(frame_error("quick_oauth_invalid_eventstream"));
    }
    Ok(u16::from_be_bytes([buf[pos], buf[pos + 1]]) as usize)
}

fn skip(buf: &[u8], pos: usize, len: usize) -> Result<usize, GatewayError> {
    if pos + len > buf.len() {
        return Err(frame_error("quick_oauth_invalid_eventstream"));
    }
    Ok(pos + len)
}

fn classify_header_error(headers: &HashMap<String, String>) -> GatewayError {
    let code = headers
        .get(":error-code")
        .or_else(|| headers.get(":exception-type"))
        .map(String::as_str)
        .unwrap_or("UpstreamStreamException");
    let lower = code.to_ascii_lowercase();
    let mut err = GatewayError::service_unavailable(format!("Amazon Quick stream error ({code})"))
        .with_code("quick_oauth_stream_exception");
    err.http_status = Some(502);
    if lower.contains("throttl") || lower.contains("too_many") || lower.contains("limit") {
        err.http_status = Some(429);
    } else if lower.contains("unauthoriz")
        || lower.contains("expired")
        || lower.contains("accessdenied")
    {
        err.http_status = Some(401);
    }
    err
}

fn frame_error(code: &'static str) -> GatewayError {
    let mut err =
        GatewayError::service_unavailable("Amazon Quick EventStream framing error").with_code(code);
    err.http_status = Some(502);
    err
}

#[cfg(test)]
pub(crate) fn encode_test_eventstream_frame(
    headers: &[(&str, &str)],
    payload_bytes: &[u8],
) -> Vec<u8> {
    let mut header_buf = Vec::new();
    for (name, val) in headers {
        header_buf.push(name.len() as u8);
        header_buf.extend_from_slice(name.as_bytes());
        header_buf.push(7u8);
        header_buf.extend_from_slice(&(val.len() as u16).to_be_bytes());
        header_buf.extend_from_slice(val.as_bytes());
    }
    let total_len = 12 + header_buf.len() + payload_bytes.len() + 4;
    let mut frame = Vec::with_capacity(total_len);
    frame.extend_from_slice(&(total_len as u32).to_be_bytes());
    frame.extend_from_slice(&(header_buf.len() as u32).to_be_bytes());
    let prelude_crc = CRC32.checksum(&frame[..8]);
    frame.extend_from_slice(&prelude_crc.to_be_bytes());
    frame.extend_from_slice(&header_buf);
    frame.extend_from_slice(payload_bytes);
    let msg_crc = CRC32.checksum(&frame);
    frame.extend_from_slice(&msg_crc.to_be_bytes());
    frame
}
