//! A paced, mutable, one-step device capability. No per-account polling task is spawned.
use super::{atom, registration::ClientRegistration, transport::post, OAuthError, TokenMaterial};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use time::OffsetDateTime;
use tokio::time::Instant;

#[derive(Debug)]
pub enum DevicePoll {
    Pending,
    SlowDown,
    Authorized(TokenMaterial),
}
pub struct DeviceChallenge {
    registration: ClientRegistration,
    verification_uri: String,
    verification_uri_complete: Option<String>,
    user_code: String,
    device_code: String,
    interval: Duration,
    deadline: Instant,
    next_poll: Instant,
    terminal: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DeviceWire {
    device_code: String,
    user_code: String,
    verification_uri: String,
    verification_uri_complete: Option<String>,
    expires_in: u64,
    interval: Option<u64>,
    error: Option<String>,
}
impl DeviceChallenge {
    pub(super) fn parse(
        wire: DeviceWire,
        registration: ClientRegistration,
        started: Instant,
    ) -> Result<Self, OAuthError> {
        if !atom(&wire.device_code, 8192)
            || !atom(&wire.user_code, 128)
            || wire.error.is_some()
            || !registration
                .directory
                .verification_url(&wire.verification_uri)
            || wire
                .verification_uri_complete
                .as_deref()
                .is_some_and(|value| {
                    !registration.directory.verification_url(value)
                        || !same_origin(value, &wire.verification_uri)
                })
            || wire.expires_in == 0
            || wire.interval == Some(0)
        {
            return Err(OAuthError::InvalidResponse);
        }
        // RFC 8628 defaults an omitted interval to 5s; explicit zero is not omission.
        let interval = Duration::from_secs(wire.interval.unwrap_or(5));
        let deadline = (started + Duration::from_secs(wire.expires_in.min(600)))
            .min(Instant::now() + registration.remaining()?);
        if deadline <= Instant::now() {
            return Err(OAuthError::Expired);
        }
        Ok(Self {
            registration,
            verification_uri: wire.verification_uri,
            verification_uri_complete: wire.verification_uri_complete,
            user_code: wire.user_code,
            device_code: wire.device_code,
            interval,
            deadline,
            next_poll: Instant::now()
                + interval.min(deadline.saturating_duration_since(Instant::now())),
            terminal: false,
        })
    }
    pub fn verification_uri(&self) -> &str {
        &self.verification_uri
    }
    pub fn verification_uri_complete(&self) -> Option<&str> {
        self.verification_uri_complete.as_deref()
    }
    pub fn user_code(&self) -> &str {
        &self.user_code
    }
    pub fn interval(&self) -> Duration {
        self.interval
    }
    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }
    pub fn cancel(&mut self) {
        self.terminal = true;
    }
    pub async fn poll(&mut self, http: &rquest::Client) -> Result<DevicePoll, OAuthError> {
        if self.terminal || self.remaining().is_zero() {
            self.terminal = true;
            return Err(OAuthError::Expired);
        }
        self.registration.remaining()?;
        tokio::time::sleep_until(self.next_poll.min(self.deadline)).await;
        if self.remaining().is_zero() {
            self.terminal = true;
            return Err(OAuthError::Expired);
        }
        let budget = Duration::from_secs(15)
            .min(self.remaining())
            .min(self.registration.remaining()?);
        // Cancellation after send is ambiguous: this capability cannot poll the same code again.
        self.terminal = true;
        let started = OffsetDateTime::now_utc();
        let reply = post(http, &self.registration.directory, "/token", json!({"clientId":self.registration.client_id,
            "clientSecret":self.registration.secret,"grantType":"urn:ietf:params:oauth:grant-type:device_code",
            "deviceCode":self.device_code}), budget).await?;
        if self.remaining().is_zero() {
            return Err(OAuthError::Expired);
        }
        self.registration.remaining()?;
        let pending = if reply.status == 400 {
            reply.error_code()
        } else {
            None
        };
        let result = match pending.as_deref() {
            Some("authorization_pending") => DevicePoll::Pending,
            Some("slow_down") => {
                self.interval = self.interval.saturating_add(Duration::from_secs(5));
                DevicePoll::SlowDown
            }
            _ => return TokenMaterial::parse(&reply, started).map(DevicePoll::Authorized),
        };
        self.next_poll = Instant::now() + self.interval.min(self.remaining());
        self.terminal = false;
        Ok(result)
    }
}
fn same_origin(a: &str, b: &str) -> bool {
    match (url::Url::parse(a), url::Url::parse(b)) {
        (Ok(a), Ok(b)) => a.origin() == b.origin(),
        _ => false,
    }
}
impl std::fmt::Debug for DeviceChallenge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("KiroDeviceChallenge { material: <redacted> }")
    }
}
#[cfg(all(test, feature = "line-kiro-official-vendor-api"))]
#[path = "device_tests.rs"]
mod tests;
