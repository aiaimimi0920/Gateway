//! One bounded inspection owns HTTP bodies and opaque session headers; no retry or resume.
use super::{decode, McpError, VERSION};
use futures::StreamExt;
use rquest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde_json::Value;
use time::OffsetDateTime;
use tokio::time::Instant;

pub(super) struct Connection<'a> {
    pub http: &'a rquest::Client,
    pub endpoint: &'a str,
    pub token: &'a str,
    pub expires_at: Option<OffsetDateTime>,
    pub deadline: Instant,
    pub session: Option<HeaderValue>,
    pub total: usize,
}

impl Connection<'_> {
    pub(super) async fn post(
        &mut self,
        body: Value,
        id: Option<u64>,
        initialized: bool,
    ) -> Result<Option<Value>, McpError> {
        let budget = self.deadline.saturating_duration_since(Instant::now());
        if budget.is_zero() {
            return Err(McpError::Timeout);
        }
        if self
            .expires_at
            .is_some_and(|at| at <= OffsetDateTime::now_utc())
        {
            return Err(McpError::Expired);
        }
        let mut bearer = HeaderValue::from_str(&format!("Bearer {}", self.token))
            .map_err(|_| McpError::InvalidResponse)?;
        bearer.set_sensitive(true);
        let mut request = self
            .http
            .post(self.endpoint)
            .default_headers(false)
            .redirect(rquest::redirect::Policy::none())
            .timeout(budget)
            .header(AUTHORIZATION, bearer)
            .header("accept", "application/json, text/event-stream")
            .json(&body);
        if initialized {
            request = request.header("MCP-Protocol-Version", VERSION);
            if let Some(session) = &self.session {
                request = request.header("Mcp-Session-Id", session.clone());
            }
        }
        let response = request.send().await.map_err(|_| McpError::Transport)?;
        if response.status().is_redirection() {
            return Err(McpError::Redirect);
        }
        if !response.status().is_success() {
            return Err(McpError::Rejected);
        }
        let session = session(response.headers())?;
        if initialized {
            if session
                .as_ref()
                .is_some_and(|value| Some(value) != self.session.as_ref())
            {
                return Err(McpError::InvalidResponse);
            }
        } else {
            self.session = session;
        }
        if response
            .content_length()
            .is_some_and(|size| size > 2 * 1024 * 1024)
        {
            return Err(McpError::Limit);
        }
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        let mut decoder = match id {
            Some(id) if content_type == "text/event-stream" => Some(decode::Decoder::new(id)),
            Some(_) if content_type == "application/json" => None,
            Some(_) => return Err(McpError::InvalidResponse),
            None if response.status().as_u16() == 202 => None,
            None => return Err(McpError::InvalidResponse),
        };
        let mut bytes = Vec::new();
        let mut size = 0usize;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| McpError::Transport)?;
            size = size.saturating_add(chunk.len());
            self.total = self.total.saturating_add(chunk.len());
            if size > 2 * 1024 * 1024 || self.total > 4 * 1024 * 1024 {
                return Err(McpError::Limit);
            }
            if id.is_none() && !chunk.is_empty() {
                return Err(McpError::InvalidResponse);
            }
            if let Some(decoder) = decoder.as_mut() {
                if let Some(value) = decoder.push(chunk)? {
                    return Ok(Some(value));
                }
            } else {
                bytes
                    .try_reserve(chunk.len())
                    .map_err(|_| McpError::Limit)?;
                bytes.extend_from_slice(&chunk);
            }
        }
        if let Some(decoder) = decoder {
            decoder.finish()?;
        }
        id.map(|id| decode::json(&bytes, id)).transpose()
    }
}

fn session(headers: &HeaderMap) -> Result<Option<HeaderValue>, McpError> {
    let mut values = headers.get_all("mcp-session-id").iter();
    let Some(value) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some()
        || value.is_empty()
        || value.as_bytes().len() > 512
        || !value.as_bytes().iter().all(|b| (0x21..=0x7e).contains(b))
    {
        return Err(McpError::InvalidResponse);
    }
    let mut value = value.clone();
    value.set_sensitive(true);
    Ok(Some(value))
}
