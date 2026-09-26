//! Lazily decodes one bounded frame at a time, independent of transport chunking.
use std::collections::HashMap;

use bytes::{Buf, Bytes};
use crc::{Crc, CRC_32_ISO_HDLC};
use serde_json::{json, Value};

use crate::error::GatewayError;

const EVENT_STREAM_CRC: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);
const MAX_KIRO_EVENT_STREAM_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone)]
pub(super) enum KiroEvent {
    AssistantResponse {
        content: String,
    },
    ToolUse {
        name: String,
        tool_use_id: String,
        input: String,
        stop: bool,
    },
    ContextUsage {
        context_usage_percentage: f64,
    },
    Error {
        error_code: String,
        error_message: String,
    },
    Exception {
        exception_type: String,
        message: String,
    },
    Unknown,
}

#[derive(Debug)]
pub(super) struct EventStreamParser {
    input: Bytes,
    buffer: Vec<u8>,
    max_buffer_bytes: usize,
}

impl Default for EventStreamParser {
    fn default() -> Self {
        Self::with_max_buffer_bytes(MAX_KIRO_EVENT_STREAM_BYTES)
    }
}

impl EventStreamParser {
    fn with_max_buffer_bytes(max_buffer_bytes: usize) -> Self {
        Self {
            input: Bytes::new(),
            buffer: Vec::new(),
            max_buffer_bytes,
        }
    }

    pub(super) fn push(&mut self, chunk: Bytes) -> Result<(), GatewayError> {
        // Callers must drain next_event to None before polling another chunk.
        // Retain only the transport-owned Bytes, never copy an entire chunk.
        if !self.input.is_empty() {
            self.reset();
            return Err(event_stream_limit_error(
                "buffer",
                "kiro_event_stream_buffer_too_large",
                self.max_buffer_bytes,
            ));
        }
        self.input = chunk;
        Ok(())
    }

    pub(super) fn next_event(&mut self) -> Result<Option<KiroEvent>, GatewayError> {
        let result = self.decode_next();
        if result.is_err() {
            self.reset();
        }
        result
    }

    fn decode_next(&mut self) -> Result<Option<KiroEvent>, GatewayError> {
        loop {
            if let Some((_, frame)) =
                parse_event_stream_frame_with_limit(&self.buffer, self.max_buffer_bytes)?
            {
                let event = parse_kiro_event(frame);
                if self.buffer.capacity() > 64 * 1024 {
                    self.buffer = Vec::new();
                } else {
                    self.buffer.clear();
                }
                return Ok(Some(event));
            }
            if self.input.is_empty() {
                return Ok(None);
            }

            // Read the prelude first so an oversized declaration fails before
            // reserving payload space. CRC/header error ordering stays in the codec.
            let target = if self.buffer.len() < 12 {
                12
            } else {
                u32::from_be_bytes(self.buffer[..4].try_into().expect("complete prelude")) as usize
            };
            let count = target
                .saturating_sub(self.buffer.len())
                .min(self.input.len());
            self.append_input(count)?;
        }
    }

    fn append_input(&mut self, count: usize) -> Result<(), GatewayError> {
        let required = self
            .buffer
            .len()
            .checked_add(count)
            .filter(|len| *len <= self.max_buffer_bytes)
            .ok_or_else(|| {
                event_stream_limit_error(
                    "buffer",
                    "kiro_event_stream_buffer_too_large",
                    self.max_buffer_bytes,
                )
            })?;
        if required > self.buffer.capacity() {
            // Geometric, fallible growth keeps byte-at-a-time input amortized linear.
            let capacity = self
                .buffer
                .capacity()
                .saturating_mul(2)
                .max(1024)
                .max(required)
                .min(self.max_buffer_bytes);
            self.buffer
                .try_reserve_exact(capacity - self.buffer.len())
                .map_err(|error| {
                    GatewayError::server_error(format!(
                        "Unable to allocate Kiro event-stream buffer: {error}"
                    ))
                    .with_code("kiro_event_stream_allocation_failed")
                })?;
        }
        self.buffer.extend_from_slice(&self.input[..count]);
        self.input.advance(count);
        if self.input.is_empty() {
            self.input = Bytes::new();
        }
        Ok(())
    }

    pub(super) fn finish(&mut self) -> Result<(), GatewayError> {
        if self.buffer.is_empty() && self.input.is_empty() {
            self.reset();
            return Ok(());
        }
        self.reset();
        Err(
            GatewayError::server_error("Truncated Kiro event-stream frame")
                .with_code("kiro_truncated_event_stream"),
        )
    }

    pub(super) fn reset(&mut self) {
        self.input = Bytes::new();
        self.buffer = Vec::new();
    }

    #[cfg(test)]
    pub(super) fn test_with_max_buffer_bytes(max_buffer_bytes: usize) -> Self {
        Self::with_max_buffer_bytes(max_buffer_bytes)
    }

    #[cfg(test)]
    pub(super) fn test_buffered_len(&self) -> usize {
        self.buffer.len().saturating_add(self.input.len())
    }
}

fn event_stream_limit_error(scope: &str, code: &str, limit: usize) -> GatewayError {
    GatewayError::server_error(format!(
        "Kiro event-stream {scope} exceeds {limit}-byte limit"
    ))
    .with_code(code.to_string())
}

#[derive(Debug)]
pub(super) struct EventStreamFrame<'a> {
    headers: HashMap<String, HeaderValue>,
    payload: &'a [u8],
}

#[derive(Debug, Clone)]
pub(super) enum HeaderValue {
    Bool,
    String(String),
    Int,
    Bytes,
    Timestamp,
    Uuid,
}

impl HeaderValue {
    fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }
}

#[cfg(test)]
pub(super) fn parse_event_stream_frame(
    buffer: &[u8],
) -> Result<Option<(usize, EventStreamFrame<'_>)>, GatewayError> {
    parse_event_stream_frame_with_limit(buffer, MAX_KIRO_EVENT_STREAM_BYTES)
}

fn parse_event_stream_frame_with_limit(
    buffer: &[u8],
    max_frame_bytes: usize,
) -> Result<Option<(usize, EventStreamFrame<'_>)>, GatewayError> {
    if buffer.len() < 12 {
        return Ok(None);
    }

    let total_len = u32::from_be_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as usize;
    let header_len = u32::from_be_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]) as usize;
    let prelude_crc = u32::from_be_bytes([buffer[8], buffer[9], buffer[10], buffer[11]]);

    if total_len > max_frame_bytes {
        return Err(event_stream_limit_error(
            "frame",
            "kiro_event_stream_frame_too_large",
            max_frame_bytes,
        ));
    }
    if total_len < 16 || buffer.len() < total_len {
        return if buffer.len() < total_len {
            Ok(None)
        } else {
            Err(GatewayError::server_error("Invalid Kiro frame length")
                .with_code("kiro_invalid_frame_length"))
        };
    }

    if EVENT_STREAM_CRC.checksum(&buffer[..8]) != prelude_crc {
        return Err(
            GatewayError::server_error("Invalid Kiro event-stream prelude CRC")
                .with_code("kiro_invalid_prelude_crc"),
        );
    }

    let payload_end = total_len.checked_sub(4).ok_or_else(|| {
        GatewayError::server_error("Invalid Kiro frame length")
            .with_code("kiro_invalid_frame_length")
    })?;
    let message_crc = u32::from_be_bytes([
        buffer[payload_end],
        buffer[payload_end + 1],
        buffer[payload_end + 2],
        buffer[payload_end + 3],
    ]);
    if EVENT_STREAM_CRC.checksum(&buffer[..payload_end]) != message_crc {
        return Err(
            GatewayError::server_error("Invalid Kiro event-stream message CRC")
                .with_code("kiro_invalid_message_crc"),
        );
    }

    let headers_start = 12usize;
    let headers_end = headers_start.checked_add(header_len).ok_or_else(|| {
        GatewayError::server_error("Invalid Kiro event-stream header length")
            .with_code("kiro_invalid_header_length")
    })?;
    if headers_end > payload_end {
        return Err(
            GatewayError::server_error("Invalid Kiro event-stream header length")
                .with_code("kiro_invalid_header_length"),
        );
    }

    Ok(Some((
        total_len,
        EventStreamFrame {
            headers: parse_headers(&buffer[headers_start..headers_end])?,
            payload: &buffer[headers_end..payload_end],
        },
    )))
}

pub(super) fn parse_headers(buffer: &[u8]) -> Result<HashMap<String, HeaderValue>, GatewayError> {
    let mut headers = HashMap::new();
    let mut offset = 0usize;

    while offset < buffer.len() {
        let name_len = *buffer.get(offset).ok_or_else(|| {
            GatewayError::server_error("Invalid Kiro header name length")
                .with_code("kiro_invalid_header_name")
        })? as usize;
        offset = checked_header_end(offset, 1, buffer.len())?;
        let name_end = offset
            .checked_add(name_len)
            .ok_or_else(header_overflow_error)?;
        if name_end > buffer.len() {
            return Err(
                GatewayError::server_error("Invalid Kiro header name bounds")
                    .with_code("kiro_invalid_header_bounds"),
            );
        }
        let name = String::from_utf8_lossy(&buffer[offset..name_end]).to_string();
        offset = name_end;

        let value_type = *buffer.get(offset).ok_or_else(|| {
            GatewayError::server_error("Invalid Kiro header type")
                .with_code("kiro_invalid_header_type")
        })?;
        offset = checked_header_end(offset, 1, buffer.len())?;

        let value = match value_type {
            0 | 1 => HeaderValue::Bool,
            2 => {
                offset = checked_header_end(offset, 1, buffer.len())?;
                HeaderValue::Int
            }
            3 => {
                offset = checked_header_end(offset, 2, buffer.len())?;
                HeaderValue::Int
            }
            4 => {
                offset = checked_header_end(offset, 4, buffer.len())?;
                HeaderValue::Int
            }
            5 => {
                offset = checked_header_end(offset, 8, buffer.len())?;
                HeaderValue::Int
            }
            6 => {
                let (len, start) = read_header_value_len(buffer, offset)?;
                offset = checked_header_end(start, len, buffer.len())?;
                HeaderValue::Bytes
            }
            7 => {
                let (len, start) = read_header_value_len(buffer, offset)?;
                let end = start.checked_add(len).ok_or_else(header_overflow_error)?;
                if end > buffer.len() {
                    return Err(
                        GatewayError::server_error("Invalid Kiro string header bounds")
                            .with_code("kiro_invalid_header_string_bounds"),
                    );
                }
                offset = end;
                HeaderValue::String(String::from_utf8_lossy(&buffer[start..end]).to_string())
            }
            8 => {
                offset = checked_header_end(offset, 8, buffer.len())?;
                HeaderValue::Timestamp
            }
            9 => {
                offset = checked_header_end(offset, 16, buffer.len())?;
                HeaderValue::Uuid
            }
            _ => {
                return Err(GatewayError::server_error("Unsupported Kiro header type")
                    .with_code("kiro_unsupported_header_type"))
            }
        };
        headers.insert(name, value);
    }
    Ok(headers)
}

fn checked_header_end(
    offset: usize,
    width: usize,
    buffer_len: usize,
) -> Result<usize, GatewayError> {
    let end = offset
        .checked_add(width)
        .ok_or_else(header_overflow_error)?;
    if end > buffer_len {
        return Err(header_overflow_error());
    }
    Ok(end)
}

fn read_header_value_len(buffer: &[u8], offset: usize) -> Result<(usize, usize), GatewayError> {
    let start = checked_header_end(offset, 2, buffer.len())?;
    let len = u16::from_be_bytes([buffer[offset], buffer[offset + 1]]) as usize;
    Ok((len, start))
}

fn header_overflow_error() -> GatewayError {
    GatewayError::server_error("Invalid Kiro header overflow")
        .with_code("kiro_invalid_header_overflow")
}

fn parse_kiro_event(frame: EventStreamFrame<'_>) -> KiroEvent {
    let message_type = frame
        .headers
        .get(":message-type")
        .and_then(HeaderValue::as_str)
        .unwrap_or("event");
    match message_type {
        "event" => parse_kiro_event_payload(frame),
        "error" => KiroEvent::Error {
            error_code: frame
                .headers
                .get(":error-code")
                .and_then(HeaderValue::as_str)
                .unwrap_or("UnknownError")
                .to_string(),
            error_message: String::from_utf8_lossy(frame.payload).to_string(),
        },
        "exception" => KiroEvent::Exception {
            exception_type: frame
                .headers
                .get(":exception-type")
                .and_then(HeaderValue::as_str)
                .unwrap_or("UnknownException")
                .to_string(),
            message: String::from_utf8_lossy(frame.payload).to_string(),
        },
        _ => KiroEvent::Unknown,
    }
}

fn parse_kiro_event_payload(frame: EventStreamFrame<'_>) -> KiroEvent {
    let event_type = frame
        .headers
        .get(":event-type")
        .and_then(HeaderValue::as_str)
        .unwrap_or("");
    let payload: Value = serde_json::from_slice(frame.payload).unwrap_or_else(|_| json!({}));
    match event_type {
        "assistantResponseEvent" => KiroEvent::AssistantResponse {
            content: payload
                .get("content")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
        },
        "toolUseEvent" => KiroEvent::ToolUse {
            name: payload
                .get("name")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            tool_use_id: payload
                .get("toolUseId")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            input: payload
                .get("input")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
            stop: payload
                .get("stop")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
        },
        "contextUsageEvent" => KiroEvent::ContextUsage {
            context_usage_percentage: payload
                .get("contextUsagePercentage")
                .and_then(|value| value.as_f64())
                .unwrap_or(0.0),
        },
        _ => KiroEvent::Unknown,
    }
}

#[cfg(test)]
mod bounds_tests {
    use super::super::event_stream_tests::event_frame;
    use super::*;

    #[test]
    fn checked_header_arithmetic_rejects_overflow() {
        assert_eq!(
            checked_header_end(usize::MAX, 1, usize::MAX)
                .unwrap_err()
                .code
                .as_deref(),
            Some("kiro_invalid_header_overflow")
        );
        assert_eq!(checked_header_end(5, 3, 8).unwrap(), 8);
        assert!(checked_header_end(5, 4, 8).is_err());
    }

    #[test]
    fn bytewise_input_uses_bounded_geometric_growth_and_releases_on_finish() {
        let input = event_frame(
            "assistantResponseEvent",
            json!({"content":"x".repeat(8192)}),
        );
        let mut parser = EventStreamParser::with_max_buffer_bytes(input.len());
        let mut growths = 0;
        let mut capacity = 0;
        let mut events = 0;
        for byte in &input {
            parser.push(Bytes::copy_from_slice(&[*byte])).unwrap();
            events += usize::from(parser.next_event().unwrap().is_some());
            if parser.buffer.capacity() != capacity {
                growths += 1;
                capacity = parser.buffer.capacity();
            }
            assert!(capacity <= input.len());
        }
        assert_eq!(events, 1);
        assert!(growths <= 6, "per-byte exact reallocations: {growths}");
        parser.finish().unwrap();
        assert_eq!(parser.buffer.capacity(), 0);
        assert!(parser.input.is_empty());
    }
}
