//! Proprietary browser grant: state + authCode, with no invented client_id, PKCE or scopes.
use super::{atom, Client, LoginError};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ring::rand::{SecureRandom, SystemRandom};
use std::time::Duration;
use subtle::ConstantTimeEq;
use tokio::time::Instant;

pub struct Authorization {
    pub(super) client: Client,
    redirect: String,
    state: String,
    pub(super) deadline: Instant,
}
impl Authorization {
    pub fn new(client: Client, redirect: &str) -> Result<Self, LoginError> {
        let url = url::Url::parse(redirect).map_err(|_| LoginError::InvalidInput)?;
        if redirect.len() > 512
            || url.scheme() != "http"
            || url.host_str() != Some("127.0.0.1")
            || !url.port().is_some_and(|p| p != 0)
            || url.path() != "/auth/callback"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.as_str() != redirect
        {
            return Err(LoginError::InvalidInput);
        }
        let mut bytes = [0u8; 32];
        SystemRandom::new()
            .fill(&mut bytes)
            .map_err(|_| LoginError::Entropy)?;
        Ok(Self {
            client,
            redirect: redirect.into(),
            state: URL_SAFE_NO_PAD.encode(bytes),
            deadline: Instant::now() + Duration::from_secs(300),
        })
    }
    pub fn redirect_uri(&self) -> &str {
        &self.redirect
    }
    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }
    pub fn authorization_url(&self) -> String {
        let query = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs([
                ("source", "electron"),
                ("redirect_uri", &self.redirect),
                ("state", &self.state),
            ])
            .finish();
        format!("{}?{query}", super::LOGIN_URL)
    }
    /// Invalid unsolicited callbacks leave the pending grant available to its owner.
    pub fn callback_code(&self, callback: &str) -> Result<String, LoginError> {
        if self.remaining().is_zero() {
            return Err(LoginError::Expired);
        }
        if callback.len() > 16_384 || callback.chars().any(char::is_control) {
            return Err(LoginError::InvalidCallback);
        }
        let mut url = url::Url::parse(callback).map_err(|_| LoginError::InvalidCallback)?;
        if url.fragment().is_some() {
            return Err(LoginError::InvalidCallback);
        }
        let mut params = std::collections::HashMap::new();
        for (key, value) in url.query_pairs() {
            if params.len() >= 16
                || params
                    .insert(key.into_owned(), value.into_owned())
                    .is_some()
            {
                return Err(LoginError::InvalidCallback);
            }
        }
        url.set_query(None);
        if url.as_str() != self.redirect {
            return Err(LoginError::InvalidCallback);
        }
        let state = params.get("state").ok_or(LoginError::StateMismatch)?;
        if !bool::from(state.as_bytes().ct_eq(self.state.as_bytes())) {
            return Err(LoginError::StateMismatch);
        }
        if params.contains_key("error") {
            return Err(LoginError::InvalidCallback);
        }
        params
            .remove("code")
            .filter(|code| atom(code, 8192))
            .ok_or(LoginError::InvalidCallback)
    }
    pub fn into_code_grant(self, callback: &str) -> Result<CodeGrant, LoginError> {
        let code = self.callback_code(callback)?;
        Ok(CodeGrant {
            authorization: self,
            code,
        })
    }
}
/// An owned, non-cloneable grant is consumed by exactly one HTTP attempt.
pub struct CodeGrant {
    authorization: Authorization,
    code: String,
}
impl CodeGrant {
    pub async fn exchange(self) -> Result<super::ReceivedSession, LoginError> {
        let budget = self.authorization.remaining().min(super::transport::BUDGET);
        if budget.is_zero() {
            return Err(LoginError::Expired);
        }
        let client = self.authorization.client;
        let mut body = client.attribution();
        body["authCode"] = self.code.into();
        let response =
            super::transport::post(&client.http, "/api/auth/exchange", &body, budget).await?;
        super::ReceivedSession::capture(client, response, None)
    }
}
impl std::fmt::Debug for Authorization {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LobsterAuthorization { material: <redacted> }")
    }
}
impl std::fmt::Debug for CodeGrant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LobsterCodeGrant { material: <redacted> }")
    }
}
