//! A mutable, paced device challenge owns its deadline; no background polling task survives drop.
use super::{atom, transport, OAuthClient, OAuthError, ReceivedTokens, INFERENCE_SCOPE, PORTAL};
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;
use tokio::time::Instant;

pub struct DeviceChallenge {
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    pub user_code: String,
    client: OAuthClient,
    device_code: String,
    interval: Duration,
    deadline: Instant,
    next_poll: Instant,
    terminal: bool,
}
#[derive(Debug)]
pub enum DevicePoll {
    Pending,
    SlowDown,
    /// Retain delivery even at the deadline; an expired challenge cannot authorize an import.
    Received {
        tokens: ReceivedTokens,
        challenge_expired: bool,
    },
}
#[derive(Deserialize)]
struct DeviceWire {
    device_code: String,
    user_code: String,
    verification_uri: String,
    verification_uri_complete: Option<String>,
    expires_in: u64,
    interval: Option<u64>,
}
impl DeviceChallenge {
    fn parse(value: Value, client: &OAuthClient) -> Result<Self, OAuthError> {
        if value.get("error").is_some() {
            return Err(OAuthError::InvalidResponse);
        }
        let wire: DeviceWire =
            serde_json::from_value(value).map_err(|_| OAuthError::InvalidResponse)?;
        if !atom(&wire.device_code, 8192)
            || !atom(&wire.user_code, 128)
            || !verification_uri(&wire.verification_uri)
            || wire
                .verification_uri_complete
                .as_deref()
                .is_some_and(|v| !verification_uri(v))
            || wire.expires_in == 0
            || wire.interval == Some(0)
        {
            return Err(OAuthError::InvalidResponse);
        }
        let now = Instant::now();
        let ttl = Duration::from_secs(wire.expires_in.min(600));
        let interval = Duration::from_secs(wire.interval.unwrap_or(5));
        Ok(Self {
            verification_uri: wire.verification_uri,
            verification_uri_complete: wire.verification_uri_complete,
            user_code: wire.user_code,
            device_code: wire.device_code,
            client: client.clone(),
            interval,
            deadline: now + ttl,
            next_poll: now + interval.min(ttl),
            terminal: false,
        })
    }
    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }
    pub fn poll_after(&self) -> Duration {
        self.next_poll.saturating_duration_since(Instant::now())
    }
    pub fn interval(&self) -> Duration {
        self.interval
    }
    fn finish_poll(&mut self, reply: transport::Reply) -> Result<DevicePoll, OAuthError> {
        if reply.status.is_success() {
            let tokens = reply.tokens()?;
            return Ok(DevicePoll::Received {
                tokens,
                challenge_expired: self.remaining().is_zero(),
            });
        }
        if self.remaining().is_zero() {
            return Err(OAuthError::Expired);
        }
        let kind = reply
            .value
            .as_ref()
            .and_then(|v| v.get("error"))
            .and_then(Value::as_str);
        let result = match (reply.status.as_u16(), kind) {
            (400, Some("authorization_pending")) => DevicePoll::Pending,
            (400, Some("slow_down")) => {
                self.interval = self.interval.saturating_add(Duration::from_secs(5));
                DevicePoll::SlowDown
            }
            _ => return Err(reply.rejection()),
        };
        self.next_poll = Instant::now() + self.interval.min(self.remaining());
        self.terminal = false;
        Ok(result)
    }
}
impl OAuthClient {
    pub async fn begin_device(&self, http: &rquest::Client) -> Result<DeviceChallenge, OAuthError> {
        let reply = transport::post(
            http,
            "/api/oauth/device/code",
            &[("client_id", self.client_id()), ("scope", INFERENCE_SCOPE)],
            None,
            transport::BUDGET,
        )
        .await?;
        if !reply.status.is_success() {
            return Err(reply.rejection());
        }
        DeviceChallenge::parse(reply.value.ok_or(OAuthError::InvalidResponse)?, self)
    }
    pub async fn poll_device(
        &self,
        http: &rquest::Client,
        challenge: &mut DeviceChallenge,
    ) -> Result<DevicePoll, OAuthError> {
        if challenge.client != *self {
            return Err(OAuthError::InvalidInput);
        }
        if challenge.terminal || challenge.remaining().is_zero() {
            return Err(OAuthError::Expired);
        }
        tokio::time::sleep_until(challenge.next_poll.min(challenge.deadline)).await;
        if challenge.remaining().is_zero() {
            challenge.terminal = true;
            return Err(OAuthError::Expired);
        }
        // Once sent, losing the future cannot prove that authorization did not complete remotely.
        challenge.terminal = true;
        let reply = transport::post(
            http,
            "/api/oauth/token",
            &[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", self.client_id()),
                ("device_code", &challenge.device_code),
            ],
            None,
            transport::BUDGET.min(challenge.remaining()),
        )
        .await?;
        challenge.finish_poll(reply)
    }
}
fn verification_uri(value: &str) -> bool {
    value.len() <= 4096
        && !value.chars().any(char::is_control)
        && url::Url::parse(value).is_ok_and(|url| {
            url.origin().ascii_serialization() == PORTAL
                && url.username().is_empty()
                && url.password().is_none()
                && url.fragment().is_none()
                && url.port().is_none()
        })
}
impl std::fmt::Debug for DeviceChallenge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NousDeviceChallenge { material: <redacted> }")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn nous_device_deadline_never_discards_an_already_delivered_successor() {
        let client = OAuthClient::new("permitted-gateway").unwrap();
        let mut challenge = DeviceChallenge::parse(serde_json::json!({"device_code":"device",
            "user_code":"ABCD","verification_uri":"https://portal.nousresearch.com/device","expires_in":1}),&client).unwrap();
        challenge.deadline = Instant::now();
        challenge.terminal = true;
        let reply = transport::Reply {
            status: rquest::StatusCode::OK,
            value: Some(
                serde_json::json!({"refresh_token":"delivered-successor","access_token":"unusable-jwt"}),
            ),
            blocked: false,
        };
        let DevicePoll::Received {
            tokens,
            challenge_expired,
        } = challenge.finish_poll(reply).unwrap()
        else {
            panic!("lost delivered receipt")
        };
        assert!(challenge_expired);
        assert!(challenge.terminal);
        assert_eq!(tokens.refresh_token().unwrap(), "delivered-successor");
    }
}
