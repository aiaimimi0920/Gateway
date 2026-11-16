//! Owned one-step approval capability. Safe GET recovery is paced even when its future is cancelled.
use super::{
    atom,
    material::UserWire,
    text,
    transport::{exchange, Request},
    Credentials, LoginError, LoginTransport, Reply,
};
use serde::Deserialize;
use std::time::Duration;
use tokio::time::Instant;

#[derive(Debug)]
pub enum LoginPoll {
    Pending,
    Authorized(Credentials),
}
pub struct LoginChallenge {
    pub(super) fingerprint: String,
    pub(super) hash: String,
    pub(super) expires_at: String,
    login_url: String,
    pub(super) deadline: Instant,
    pub(super) next_poll: Instant,
    terminal: Option<LoginError>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodeWire {
    login_url: String,
    fingerprint_hash: String,
    expires_at: String,
    expires_in_ms: Option<u64>,
    error: Option<serde_json::Value>,
}
#[derive(Deserialize)]
struct StatusWire {
    user: Option<UserWire>,
    error: Option<serde_json::Value>,
}

impl LoginChallenge {
    pub async fn start(http: &LoginTransport, fingerprint: &str) -> Result<Self, LoginError> {
        if !cfg!(feature = "line-freebuff-web-reverse-api") {
            return Err(LoginError::FeatureDisabled);
        }
        if !atom(fingerprint, 512) {
            return Err(LoginError::InvalidInput);
        }
        let started = Instant::now();
        let reply = exchange(http, Request::Code { fingerprint }, Duration::from_secs(15)).await?;
        Self::parse(fingerprint, reply, started)
    }
    pub(super) fn parse(
        fingerprint: &str,
        reply: Reply,
        started: Instant,
    ) -> Result<Self, LoginError> {
        if !(200..300).contains(&reply.status) {
            return Err(LoginError::Rejected);
        }
        let wire: CodeWire = reply.decode()?;
        if !browser_url(&wire.login_url)
            || !atom(&wire.fingerprint_hash, 4096)
            || !text(&wire.expires_at, 1024)
            || wire.error.is_some()
            || wire.expires_in_ms == Some(0)
        {
            return Err(LoginError::InvalidResponse);
        }
        // The fixed CLI polls for one hour when the relative hint is absent. Never parse HMAC expiry.
        let deadline =
            started + Duration::from_millis(wire.expires_in_ms.unwrap_or(3_600_000).min(3_600_000));
        if deadline <= Instant::now() {
            return Err(LoginError::Expired);
        }
        Ok(Self {
            fingerprint: fingerprint.into(),
            hash: wire.fingerprint_hash,
            expires_at: wire.expires_at,
            login_url: wire.login_url,
            deadline,
            next_poll: Instant::now(),
            terminal: None,
        })
    }
    pub fn login_url(&self) -> &str {
        &self.login_url
    }
    pub fn interval(&self) -> Duration {
        Duration::from_secs(5)
    }
    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }
    pub fn cancel(&mut self) {
        self.terminal = Some(LoginError::Cancelled);
    }
    fn active(&mut self) -> Result<(), LoginError> {
        if let Some(error) = self.terminal {
            return Err(error);
        }
        if self.remaining().is_zero() {
            self.terminal = Some(LoginError::Expired);
            return Err(LoginError::Expired);
        }
        Ok(())
    }
    pub async fn poll(&mut self, http: &LoginTransport) -> Result<LoginPoll, LoginError> {
        self.active()?;
        tokio::time::sleep_until(self.next_poll.min(self.deadline)).await;
        self.active()?;
        let budget = Duration::from_secs(15).min(self.remaining());
        let pace = Pace {
            next: &mut self.next_poll,
        };
        let reply = exchange(
            http,
            Request::Status {
                fingerprint: &self.fingerprint,
                hash: &self.hash,
                expiry: &self.expires_at,
            },
            budget,
        )
        .await;
        drop(pace);
        self.active()?;
        let reply = reply?;
        // The official CLI treats status 401 as waiting; no body text is used as an auth verdict.
        if reply.status == 401 {
            return Ok(LoginPoll::Pending);
        }
        if !(200..300).contains(&reply.status) {
            return Err(LoginError::Rejected);
        }
        let wire: StatusWire = reply
            .decode()
            .inspect_err(|_| self.terminal = Some(LoginError::InvalidResponse))?;
        if wire.error.is_some() {
            self.terminal = Some(LoginError::InvalidResponse);
            return Err(LoginError::InvalidResponse);
        }
        let Some(user) = wire.user else {
            return Ok(LoginPoll::Pending);
        };
        // Only this successful response can yield material; no second claim, even if it was malformed.
        self.terminal = Some(LoginError::Consumed);
        Credentials::accept(user, &self.fingerprint, &self.hash).map(LoginPoll::Authorized)
    }
}
struct Pace<'a> {
    next: &'a mut Instant,
}
impl Drop for Pace<'_> {
    fn drop(&mut self) {
        *self.next = Instant::now() + Duration::from_secs(5);
    }
}
fn browser_url(value: &str) -> bool {
    if !atom(value, 8192) {
        return false;
    }
    url::Url::parse(value).is_ok_and(|url| {
        url.scheme() == "https"
            && url.host_str() == Some("freebuff.com")
            && url.port_or_known_default() == Some(443)
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none()
    })
}
impl std::fmt::Debug for LoginChallenge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FreeBuffLoginChallenge { material: <redacted> }")
    }
}
