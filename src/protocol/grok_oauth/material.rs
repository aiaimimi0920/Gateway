//! Bounded wire material, deliberately not an authenticated identity or an import receipt.
use super::{valid_atom, OAuthClient, OAuthError};
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;
use tokio::time::Instant;

/// Duplicate top-level fields must not silently select another token or OAuth outcome.
pub(super) fn parse_reply(bytes: &[u8]) -> Result<Value, OAuthError> {
    struct Reply(Value);
    impl<'de> Deserialize<'de> for Reply {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            struct Object;
            impl<'de> serde::de::Visitor<'de> for Object {
                type Value = Reply;
                fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.write_str("a unique-field OAuth object")
                }
                fn visit_map<M: serde::de::MapAccess<'de>>(
                    self,
                    mut access: M,
                ) -> Result<Reply, M::Error> {
                    let mut map = serde_json::Map::new();
                    while let Some((key, value)) = access.next_entry::<String, Value>()? {
                        if map.insert(key, value).is_some() {
                            return Err(serde::de::Error::custom("duplicate OAuth field"));
                        }
                    }
                    Ok(Reply(Value::Object(map)))
                }
            }
            d.deserialize_map(Object)
        }
    }
    serde_json::from_slice::<Reply>(bytes)
        .map(|v| v.0)
        .map_err(|_| OAuthError::InvalidResponse)
}

#[derive(Clone, Deserialize, PartialEq, Eq)]
pub struct UnverifiedTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub expires_in: Option<u64>,
    pub scope: Option<String>,
}
impl UnverifiedTokens {
    pub(super) fn parse(value: Value) -> Result<Self, OAuthError> {
        if value.get("error").is_some()
            || value.get("token_type").is_some_and(|kind| {
                !kind
                    .as_str()
                    .is_some_and(|s| s.eq_ignore_ascii_case("Bearer"))
            })
        {
            return Err(OAuthError::InvalidResponse);
        }
        let tokens: Self =
            serde_json::from_value(value).map_err(|_| OAuthError::InvalidResponse)?;
        if !valid_atom(&tokens.access_token, 32_768)
            || tokens
                .refresh_token
                .as_deref()
                .is_some_and(|s| !valid_atom(s, 32_768))
            || tokens
                .id_token
                .as_deref()
                .is_some_and(|s| !valid_atom(s, 32_768))
            || tokens
                .expires_in
                .is_some_and(|s| s == 0 || s > 100 * 365 * 86400)
            || tokens
                .scope
                .as_ref()
                .is_some_and(|s| s.len() > 4096 || s.bytes().any(|b| !(0x20..=0x7e).contains(&b)))
        {
            return Err(OAuthError::InvalidResponse);
        }
        Ok(tokens)
    }
}
impl std::fmt::Debug for UnverifiedTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("UnverifiedGrokTokens { material: <redacted> }")
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum DevicePoll {
    Pending,
    SlowDown,
    /// Grant transport succeeded; identity/principal validation is still required before import.
    Authorized(UnverifiedTokens),
}

pub struct DeviceChallenge {
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    pub user_code: String,
    pub(super) device_code: String,
    pub(super) interval: Duration,
    pub(super) deadline: Instant,
    pub(super) next_poll: Instant,
    pub(super) client: OAuthClient,
    pub(super) terminal: bool,
}
#[derive(Deserialize)]
struct DeviceWire {
    verification_uri: String,
    verification_uri_complete: Option<String>,
    user_code: String,
    device_code: String,
    expires_in: u64,
    interval: Option<u64>,
}
impl DeviceChallenge {
    pub(super) fn parse(value: Value, client: &OAuthClient) -> Result<Self, OAuthError> {
        let wire: DeviceWire =
            serde_json::from_value(value).map_err(|_| OAuthError::InvalidResponse)?;
        if !valid_atom(&wire.device_code, 8192)
            || wire.user_code.is_empty()
            || wire.user_code.len() > 128
            || !wire
                .user_code
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || !verification_uri(&wire.verification_uri)
            || wire
                .verification_uri_complete
                .as_deref()
                .is_some_and(|s| !verification_uri(s))
            || wire.expires_in == 0
            || wire.interval == Some(0)
        {
            return Err(OAuthError::InvalidResponse);
        }
        // Gateway's local session ceiling never extends a shorter server expiry.
        let now = Instant::now();
        let ttl = Duration::from_secs(wire.expires_in.min(600));
        let interval = Duration::from_secs(wire.interval.unwrap_or(5));
        Ok(Self {
            verification_uri: wire.verification_uri,
            verification_uri_complete: wire.verification_uri_complete,
            user_code: wire.user_code,
            device_code: wire.device_code,
            interval,
            deadline: now + ttl,
            next_poll: now + interval.min(ttl),
            client: client.clone(),
            terminal: false,
        })
    }
    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }
    pub fn interval(&self) -> Duration {
        self.interval
    }
    /// Admission may return the current challenge instead of holding an HTTP action during pacing.
    pub fn poll_after(&self) -> Duration {
        self.next_poll.saturating_duration_since(Instant::now())
    }
}
fn verification_uri(value: &str) -> bool {
    if value.len() > 4096 || value.chars().any(char::is_control) {
        return false;
    }
    url::Url::parse(value).is_ok_and(|url| {
        url.scheme() == "https"
            && matches!(url.host_str(), Some("accounts.x.ai" | "auth.x.ai"))
            && url.port().is_none()
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none()
    })
}
impl std::fmt::Debug for DeviceChallenge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GrokDeviceChallenge { material: <redacted> }")
    }
}
