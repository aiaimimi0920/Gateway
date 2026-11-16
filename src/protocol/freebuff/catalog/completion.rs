//! Freeze the FINAL JSON once, sign those bytes and transfer those same owned bytes to HTTP.
use super::{business_wire, BusinessError, BusinessRequests};
use serde_json::Value;
use std::{
    io::{self, Write},
    time::Duration,
};

pub struct PreparedCompletion<'a> {
    requests: &'a BusinessRequests<'a>,
    bytes: Vec<u8>,
    stream: bool,
}
impl BusinessRequests<'_> {
    /// This is a wire primitive, not run/session admission. The caller supplies its final envelope.
    /// No model substitution, metadata mutation or second serialization is allowed after this point.
    pub fn prepare_completion(
        &self,
        body: &Value,
        limit: usize,
    ) -> Result<PreparedCompletion<'_>, BusinessError> {
        if !body.is_object()
            || body.get("model").and_then(Value::as_str) != Some(self.selected.handle)
            || limit == 0
            || limit > 50 * 1024 * 1024
        {
            return Err(BusinessError::InvalidInput);
        }
        Ok(PreparedCompletion {
            requests: self,
            bytes: freeze(body, limit)?,
            stream: body.get("stream") == Some(&Value::Bool(true)),
        })
    }
}
pub(super) fn freeze(body: &Value, limit: usize) -> Result<Vec<u8>, BusinessError> {
    if limit == 0 || limit > 50 * 1024 * 1024 {
        return Err(BusinessError::InvalidInput);
    }
    let mut writer = LimitedBytes {
        bytes: Vec::new(),
        limit,
    };
    serde_json::to_writer(&mut writer, body).map_err(|_| BusinessError::RequestTooLarge)?;
    Ok(writer.bytes)
}
impl PreparedCompletion<'_> {
    /// The caller owns reading/decoding/status handling and must keep its stream/lease lifecycle.
    /// This never retries, follows redirects, starts a run, buys a session or falls back unsigned.
    pub async fn send(self, budget: Duration) -> Result<rquest::Response, BusinessError> {
        business_wire::completion(self.requests, self.bytes, self.stream, budget).await
    }
}
impl std::fmt::Debug for PreparedCompletion<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FreeBuffPreparedCompletion { body: <redacted> }")
    }
}
struct LimitedBytes {
    bytes: Vec<u8>,
    limit: usize,
}
impl Write for LimitedBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(io::Error::other("FreeBuff JSON limit reached"));
        }
        self.bytes
            .try_reserve(bytes.len())
            .map_err(io::Error::other)?;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
