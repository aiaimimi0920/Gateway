//! One correlated JSON-RPC result completes a POST; never wait for socket EOF on SSE.
use super::McpError;
use crate::protocol::{
    stream_decode::{BoundedSseDecoder, DecodeStep},
    unique_json,
};
use bytes::Bytes;
use serde_json::Value;

pub(super) fn json(bytes: &[u8], id: u64) -> Result<Value, McpError> {
    result(
        unique_json::parse(bytes).map_err(|_| McpError::InvalidResponse)?,
        id,
    )?
    .ok_or(McpError::InvalidResponse)
}

fn result(value: Value, id: u64) -> Result<Option<Value>, McpError> {
    let object = value.as_object().ok_or(McpError::InvalidResponse)?;
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(McpError::InvalidResponse);
    }
    if object.contains_key("error") {
        return Err(McpError::Rejected);
    }
    if object.contains_key("method") {
        if object.contains_key("id")
            || object.contains_key("result")
            || !matches!(
                value["method"].as_str(),
                Some("notifications/progress" | "notifications/message")
            )
            || !value["params"].is_object()
        {
            return Err(McpError::InvalidResponse);
        }
        // Log/progress text is untrusted and can contain private data; never expose or execute it.
        return Ok(None);
    }
    if value["id"].as_u64() != Some(id) || !value["result"].is_object() {
        return Err(McpError::InvalidResponse);
    }
    let Value::Object(mut object) = value else {
        unreachable!("checked object")
    };
    Ok(object.remove("result"))
}

pub(super) struct Decoder {
    wire: BoundedSseDecoder,
    id: u64,
    events: usize,
}
impl Decoder {
    pub(super) fn new(id: u64) -> Self {
        Self {
            wire: BoundedSseDecoder::new_strict(1024 * 1024),
            id,
            events: 0,
        }
    }
    pub(super) fn push(&mut self, bytes: Bytes) -> Result<Option<Value>, McpError> {
        self.wire.push_chunk(bytes);
        loop {
            match self.wire.decode_next() {
                DecodeStep::NeedInput => return Ok(None),
                DecodeStep::Error(_) => return Err(McpError::InvalidResponse),
                DecodeStep::Frame(frame) => {
                    self.events += 1;
                    if self.events > 256 {
                        return Err(McpError::Limit);
                    }
                    if frame
                        .event_name
                        .as_deref()
                        .is_some_and(|name| name != "message")
                    {
                        return Err(McpError::InvalidResponse);
                    }
                    if frame.data.is_empty() {
                        continue;
                    }
                    let value = unique_json::parse(frame.data.as_bytes())
                        .map_err(|_| McpError::InvalidResponse)?;
                    if let Some(value) = result(value, self.id)? {
                        return Ok(Some(value));
                    }
                }
            }
        }
    }
    pub(super) fn finish(&self) -> Result<(), McpError> {
        self.wire
            .finish_strict()
            .map_err(|_| McpError::InvalidResponse)?;
        Err(McpError::InvalidResponse)
    }
}
