//! Durable Web-session overlay; OAuth tokens and public media results never own this material.
#[cfg(all(test, feature = "line-lumalabs-web-reverse-api"))]
mod browser_tests;
#[cfg(feature = "line-lumalabs-web-reverse-api")]
pub(crate) mod maintenance;
mod pending;
mod storage;
#[cfg(test)]
mod tests;

use crate::{error::GatewayError, routing::candidate::ProviderAccountPayload};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
pub(crate) use storage::SessionStore;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SessionState {
    version: u32,
    token: String,
    // Permanent until the exact owner returns a receipt. A crash is not proof of non-dispatch.
    attempt: Option<String>,
    reauthentication_required: bool,
    checked_at: i64,
}

pub(crate) struct SessionAttempt {
    key: String,
    state: SessionState,
    host: String,
    permit: std::sync::Arc<tokio::sync::OwnedSemaphorePermit>,
}
impl SessionAttempt {
    pub(crate) fn token(&self) -> &str {
        &self.state.token
    }
    pub(crate) fn binding(&self) -> &str {
        self.state.attempt.as_deref().expect("reserved attempt")
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Receipt {
    version: u32,
    binding: String,
    cookie: Option<BrowserCookie>,
    reauthentication_required: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowserCookie {
    name: String,
    value: String,
    domain: String,
    path: String,
    expires: f64,
    http_only: bool,
    secure: bool,
    same_site: String,
}

pub(crate) fn error(code: &'static str) -> GatewayError {
    let mut error = GatewayError::service_unavailable(match code {
        "lumalabs_session_unconfirmed" => "Luma Web session has an unfinished request. Restore its receipt or import a fresh browser session; generation was not replayed.",
        "lumalabs_session_login_required" => "Luma Web session is no longer authenticated. Sign in with the local browser and update this account.",
        _ => "Luma Web session could not be safely loaded or saved; generation must not be replayed.",
    }).with_code(code).with_provider("lumalabs_compatible");
    error.retryable = false;
    error.fallback_hint = crate::error::FallbackHint::Abort {
        reason: "Luma Web session ownership is required.".into(),
    };
    error
}
fn valid_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32768
        && value
            .bytes()
            .all(|c| (0x21..=0x7e).contains(&c) && !b";,\"\\".contains(&c))
}
fn now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

fn identity(
    provider: &str,
    payload: &ProviderAccountPayload,
) -> Result<(String, String), GatewayError> {
    let url = url::Url::parse(&payload.base_url)
        .map_err(|_| error("lumalabs_session_binding_invalid"))?;
    if payload.adapter != "lumalabs_compatible"
        || !valid_token(&payload.api_key)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
        || !(url.scheme() == "https" && url.host_str() == Some("app.lumalabs.ai")
            || url.scheme() == "http"
                && matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]")))
    {
        return Err(error("lumalabs_session_binding_invalid"));
    }
    // Bind the complete imported authorization, including realm aliases, UA and session policy.
    // Stable object ordering makes HashMap iteration irrelevant. Only a digest becomes a filename.
    let realms: std::collections::BTreeMap<_, _> = payload
        .extra_body
        .iter()
        .flatten()
        .filter(|(key, _)| {
            matches!(
                key.as_str(),
                "realmId" | "realm_id" | "boardId" | "board_id"
            )
        })
        .collect();
    let mut value = serde_json::json!([
        "luma-web-session-v1",
        provider,
        payload.credential_id,
        payload.adapter,
        payload.base_url,
        payload.api_key,
        payload.headers,
        realms,
        payload.auth_mode,
        payload.auth_header_name,
        payload.auth_token,
        payload.session_auth,
        payload.expires_at
    ]);
    value.sort_all_objects();
    Ok((
        hex::encode(Sha256::digest(value.to_string())),
        url.host_str().unwrap().into(),
    ))
}

impl SessionState {
    pub(crate) fn decode(raw: &str) -> Result<Self, GatewayError> {
        if raw.len() > 65536 {
            return Err(error("lumalabs_session_store_invalid"));
        }
        let state: Self =
            serde_json::from_str(raw).map_err(|_| error("lumalabs_session_store_invalid"))?;
        if state.version != 1
            || !valid_token(&state.token)
            || state.checked_at < 0
            || state
                .attempt
                .as_ref()
                .is_some_and(|s| uuid::Uuid::parse_str(s).is_err())
        {
            return Err(error("lumalabs_session_store_invalid"));
        }
        Ok(state)
    }
    fn encode(&self) -> String {
        serde_json::to_string(self).expect("session state is serializable")
    }
}

impl SessionStore {
    pub(crate) async fn begin(
        &self,
        provider: &str,
        payload: &ProviderAccountPayload,
        only_if_due: bool,
    ) -> Result<Option<SessionAttempt>, GatewayError> {
        let (key, host) = identity(provider, payload)?;
        self.flush_key(&key).await?;
        let permit = std::sync::Arc::new(
            self.slots
                .clone()
                .try_acquire_owned()
                .map_err(|_| error("lumalabs_session_capacity"))?,
        );
        let raw = self.read(&key).await?;
        let mut state = match &raw {
            Some(raw) => SessionState::decode(raw)?,
            None => SessionState {
                version: 1,
                token: payload.api_key.clone(),
                attempt: None,
                reauthentication_required: false,
                checked_at: 0,
            },
        };
        if state.reauthentication_required {
            return Err(error("lumalabs_session_login_required"));
        }
        if state.attempt.is_some() {
            return Err(error("lumalabs_session_unconfirmed"));
        }
        if only_if_due && now().saturating_sub(state.checked_at) < 300 {
            return Ok(None);
        }
        state.attempt = Some(uuid::Uuid::new_v4().to_string());
        self.cas(&key, raw.as_deref(), &state.encode()).await?;
        Ok(Some(SessionAttempt {
            key,
            state,
            host,
            permit,
        }))
    }

    pub(crate) async fn finish(
        &self,
        attempt: &SessionAttempt,
        receipt: serde_json::Value,
    ) -> Result<(), GatewayError> {
        let receipt: Receipt = serde_json::from_value(receipt)
            .map_err(|_| error("lumalabs_session_receipt_invalid"))?;
        if receipt.version != 1 || receipt.binding != attempt.binding() {
            return Err(error("lumalabs_session_receipt_invalid"));
        }
        let mut next = attempt.state.clone();
        if let Some(cookie) = receipt.cookie {
            if cookie.name != "wos-session"
                || cookie.domain.trim_start_matches('.') != attempt.host
                || cookie.path != "/"
                || !cookie.secure
                || !cookie.http_only
                || !matches!(cookie.same_site.as_str(), "Lax" | "Strict")
                || !valid_token(&cookie.value)
                || !(cookie.expires == -1.0 || cookie.expires > now() as f64)
            {
                return Err(error("lumalabs_session_receipt_invalid"));
            }
            next.token = cookie.value;
        }
        next.attempt = None;
        next.reauthentication_required = receipt.reauthentication_required;
        next.checked_at = now();
        self.retain_and_save(attempt, next).await
    }
}
