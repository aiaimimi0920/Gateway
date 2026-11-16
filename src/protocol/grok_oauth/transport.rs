//! Exactly one bounded HTTP attempt per grant; session owner supplies cancellation by dropping.
use super::{
    valid_atom, Authorization, DeviceChallenge, DevicePoll, OAuthClient, OAuthError,
    UnverifiedTokens,
};
use futures::StreamExt;
use serde_json::Value;
use std::time::Duration;
use tokio::time::Instant;

const MAX_RESPONSE: usize = 65_536;
const DEADLINE: Duration = Duration::from_secs(15);
impl OAuthClient {
    pub(crate) fn validate_refresh_token(refresh_token: &str) -> Result<(), OAuthError> {
        if !valid_atom(refresh_token, 32_768) {
            return Err(OAuthError::InvalidInput);
        }
        Ok(())
    }
    pub async fn exchange_code(
        &self,
        http: &rquest::Client,
        auth: &Authorization,
        code: &str,
    ) -> Result<UnverifiedTokens, OAuthError> {
        if !valid_atom(code, 8192) || auth.client != *self {
            return Err(OAuthError::InvalidInput);
        }
        let form = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", auth.redirect_uri()),
            ("client_id", &self.client_id),
            ("code_verifier", &auth.verifier),
        ];
        let (status, value) = self.post(http, "/oauth2/token", &form, DEADLINE).await?;
        token_result(status, value)
    }
    pub async fn refresh(
        &self,
        http: &rquest::Client,
        refresh_token: &str,
    ) -> Result<UnverifiedTokens, OAuthError> {
        Self::validate_refresh_token(refresh_token)?;
        let mut form = vec![
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", &self.client_id),
        ];
        if let Some(principal) = &self.principal {
            form.extend([
                ("principal_type", principal.kind()),
                ("principal_id", principal.id()),
            ]);
        }
        let (status, value) = self.post(http, "/oauth2/token", &form, DEADLINE).await?;
        token_result(status, value)
    }
    pub async fn begin_device(&self, http: &rquest::Client) -> Result<DeviceChallenge, OAuthError> {
        let form = [
            ("client_id", self.client_id.as_str()),
            ("scope", &self.scope),
            ("referrer", "neuro-gateway"),
        ];
        let (status, value) = self
            .post(http, "/oauth2/device/code", &form, DEADLINE)
            .await?;
        if !status.is_success() {
            return Err(rejection(status, &value));
        }
        if value.get("error").is_some() {
            return Err(OAuthError::InvalidResponse);
        }
        DeviceChallenge::parse(value, self)
    }
    /// One paced step, not an unbounded polling task. A mutable challenge prevents concurrent polls.
    pub async fn poll_device(
        &self,
        http: &rquest::Client,
        challenge: &mut DeviceChallenge,
    ) -> Result<DevicePoll, OAuthError> {
        if challenge.client != *self {
            return Err(OAuthError::InvalidInput);
        }
        if challenge.terminal || challenge.remaining().is_zero() {
            challenge.terminal = true;
            return Err(OAuthError::Expired);
        }
        tokio::time::sleep_until(challenge.next_poll.min(challenge.deadline)).await;
        if challenge.remaining().is_zero() {
            challenge.terminal = true;
            return Err(OAuthError::Expired);
        }
        challenge.next_poll = Instant::now() + challenge.interval.min(challenge.remaining());
        let form = [
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("device_code", challenge.device_code.as_str()),
            ("client_id", &self.client_id),
        ];
        let result = self
            .post(
                http,
                "/oauth2/token",
                &form,
                DEADLINE.min(challenge.remaining()),
            )
            .await;
        let (status, value) = result?;
        if challenge.remaining().is_zero() {
            challenge.terminal = true;
            return Err(OAuthError::Expired);
        }
        let result = if status.is_success() {
            UnverifiedTokens::parse(value).map(DevicePoll::Authorized)
        } else if status.as_u16() == 400 {
            match value.get("error").and_then(Value::as_str) {
                Some("authorization_pending") => Ok(DevicePoll::Pending),
                Some("slow_down") => {
                    challenge.interval = challenge.interval.saturating_add(Duration::from_secs(5));
                    Ok(DevicePoll::SlowDown)
                }
                _ => Err(rejection(status, &value)),
            }
        } else {
            Err(rejection(status, &value))
        };
        challenge.next_poll = Instant::now() + challenge.interval.min(challenge.remaining());
        if !matches!(result, Ok(DevicePoll::Pending | DevicePoll::SlowDown)) {
            challenge.terminal = true;
        }
        result
    }
    async fn post(
        &self,
        http: &rquest::Client,
        path: &str,
        form: &[(&str, &str)],
        deadline: Duration,
    ) -> Result<(rquest::StatusCode, Value), OAuthError> {
        tokio::time::timeout(deadline, async {
            let response = http
                .post(format!("{}{path}", self.issuer))
                .default_headers(false)
                .redirect(rquest::redirect::Policy::none())
                .timeout(deadline)
                .form(form)
                .send()
                .await
                .map_err(|_| OAuthError::Transport)?;
            let status = response.status();
            if status.is_redirection() {
                return Err(OAuthError::RedirectRejected);
            }
            if response
                .content_length()
                .is_some_and(|n| n > MAX_RESPONSE as u64)
            {
                return Err(OAuthError::ResponseTooLarge);
            }
            let mut bytes = Vec::new();
            let mut stream = response.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|_| OAuthError::Transport)?;
                if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE {
                    return Err(OAuthError::ResponseTooLarge);
                }
                bytes
                    .try_reserve(chunk.len())
                    .map_err(|_| OAuthError::ResponseTooLarge)?;
                bytes.extend_from_slice(&chunk);
            }
            let value = super::material::parse_reply(&bytes)?;
            Ok((status, value))
        })
        .await
        .map_err(|_| OAuthError::Timeout)?
    }
}
fn token_result(status: rquest::StatusCode, value: Value) -> Result<UnverifiedTokens, OAuthError> {
    if !status.is_success() {
        return Err(rejection(status, &value));
    }
    UnverifiedTokens::parse(value)
}
fn rejection(status: rquest::StatusCode, value: &Value) -> OAuthError {
    if !matches!(status.as_u16(), 400 | 401) {
        return OAuthError::Rejected;
    }
    match value.get("error").and_then(Value::as_str) {
        Some("invalid_grant" | "invalid_client" | "unauthorized_client") => {
            OAuthError::ReauthorizationRequired
        }
        Some("access_denied") => OAuthError::AccessDenied,
        Some("expired_token") => OAuthError::Expired,
        _ => OAuthError::Rejected,
    }
}
