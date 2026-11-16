//! Best-effort session termination, independent of catalog success and without retries.
use super::{transport::Connection, VERSION};
use rquest::header::{HeaderValue, AUTHORIZATION};
use std::time::Duration;
use time::OffsetDateTime;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Cleanup {
    NotNeeded,
    Closed,
    AlreadyClosed,
    Unsupported,
    Unconfirmed,
}

impl Connection<'_> {
    pub(super) async fn close(&mut self) -> Cleanup {
        let Some(session) = self.session.take() else {
            return Cleanup::NotNeeded;
        };
        if self
            .expires_at
            .is_some_and(|at| at <= OffsetDateTime::now_utc())
        {
            return Cleanup::Unconfirmed;
        }
        let Ok(mut bearer) = HeaderValue::from_str(&format!("Bearer {}", self.token)) else {
            return Cleanup::Unconfirmed;
        };
        bearer.set_sensitive(true);
        // Cleanup gets its own two-second budget, including after a catalog timeout.
        // No response body is needed: dropping it bounds memory and releases the stream.
        let budget = Duration::from_secs(2);
        let request = self
            .http
            .delete(self.endpoint)
            .default_headers(false)
            .redirect(rquest::redirect::Policy::none())
            .timeout(budget)
            .header(AUTHORIZATION, bearer)
            .header("Mcp-Session-Id", session)
            .header("MCP-Protocol-Version", VERSION);
        match tokio::time::timeout(budget, request.send()).await {
            Ok(Ok(response)) => match response.status().as_u16() {
                200 | 204 => Cleanup::Closed,
                404 => Cleanup::AlreadyClosed,
                405 => Cleanup::Unsupported,
                _ => Cleanup::Unconfirmed,
            },
            _ => Cleanup::Unconfirmed,
        }
    }
}
