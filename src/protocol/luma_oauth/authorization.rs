//! PKCE capability only; management authentication, listeners and publication belong to the caller.
use super::{atom, OAuthError, AUTHORIZE_ENDPOINT, ISSUER, RESOURCE};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ring::rand::{SecureRandom, SystemRandom};
use sha2::{Digest, Sha256};
use std::time::Duration;
use subtle::ConstantTimeEq;
use tokio::time::Instant;
use url::Url;

pub struct Authorization {
    pub(super) client_id: String,
    pub(super) redirect: String,
    pub(super) verifier: String,
    pub(super) offline_access: bool,
    state: String,
    deadline: Instant,
}

impl Authorization {
    /// Use an explicitly permitted, registered public client. Never borrow a first-party identity.
    pub fn new(client_id: &str, redirect: &str) -> Result<Self, OAuthError> {
        if !atom(client_id, 512) {
            return Err(OAuthError::InvalidInput);
        }
        Self::callback_port(redirect)?;
        Ok(Self {
            client_id: client_id.into(),
            redirect: redirect.into(),
            verifier: entropy()?,
            offline_access: false,
            state: entropy()?,
            deadline: Instant::now() + Duration::from_secs(1200),
        })
    }

    /// Only an explicit caller choice may request persistent access and fresh consent.
    pub fn with_offline_access(mut self) -> Self {
        self.offline_access = true;
        self
    }

    pub fn callback_port(redirect: &str) -> Result<u16, OAuthError> {
        if redirect.len() > 512 || redirect.chars().any(char::is_control) {
            return Err(OAuthError::InvalidInput);
        }
        let url = Url::parse(redirect).map_err(|_| OAuthError::InvalidInput)?;
        if url.scheme() != "http"
            || url.host_str() != Some("127.0.0.1")
            || !url.port().is_some_and(|port| port != 0)
            || url.path() != "/callback"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.as_str() != redirect
        {
            return Err(OAuthError::InvalidInput);
        }
        Ok(url.port().expect("validated explicit port"))
    }

    pub fn client_id(&self) -> &str {
        &self.client_id
    }
    pub fn redirect_uri(&self) -> &str {
        &self.redirect
    }
    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    pub fn authorize_url(&self) -> String {
        let mut url = Url::parse(AUTHORIZE_ENDPOINT).expect("fixed endpoint");
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(self.verifier.as_bytes()));
        url.query_pairs_mut().extend_pairs([
            ("response_type", "code"),
            ("client_id", &self.client_id),
            ("redirect_uri", &self.redirect),
            (
                "scope",
                if self.offline_access {
                    "openid offline_access"
                } else {
                    "openid"
                },
            ),
            ("resource", RESOURCE),
            ("state", &self.state),
            ("code_challenge", &challenge),
            ("code_challenge_method", "S256"),
        ]);
        if self.offline_access {
            url.query_pairs_mut().append_pair("prompt", "consent");
        }
        url.into()
    }

    /// Invalid callbacks must not consume the pending owner or stop its listener.
    pub fn validate_callback(&self, callback: &str) -> Result<(), OAuthError> {
        self.code(callback).map(|_| ())
    }

    pub fn into_code_grant(self, callback: &str) -> Result<CodeGrant, OAuthError> {
        let code = self.code(callback)?;
        Ok(CodeGrant {
            authorization: self,
            code,
        })
    }

    fn code(&self, callback: &str) -> Result<String, OAuthError> {
        if self.remaining().is_zero() {
            return Err(OAuthError::Expired);
        }
        if callback.len() > 16_384 || callback.chars().any(char::is_control) {
            return Err(OAuthError::InvalidCallback);
        }
        let mut url = Url::parse(callback).map_err(|_| OAuthError::InvalidCallback)?;
        if url.fragment().is_some() {
            return Err(OAuthError::InvalidCallback);
        }
        let mut values = std::collections::HashMap::new();
        for (key, value) in url.query_pairs() {
            if values.len() >= 16
                || values
                    .insert(key.into_owned(), value.into_owned())
                    .is_some()
            {
                return Err(OAuthError::InvalidCallback);
            }
        }
        url.set_query(None);
        if url.as_str() != self.redirect {
            return Err(OAuthError::InvalidCallback);
        }
        let state = values.get("state").ok_or(OAuthError::StateMismatch)?;
        if !bool::from(self.state.as_bytes().ct_eq(state.as_bytes())) {
            return Err(OAuthError::StateMismatch);
        }
        if values.contains_key("code") && values.contains_key("error")
            || values.get("iss").is_some_and(|issuer| issuer != ISSUER)
        {
            return Err(OAuthError::InvalidCallback);
        }
        if let Some(error) = values.get("error") {
            return Err(if error == "access_denied" {
                OAuthError::AccessDenied
            } else {
                OAuthError::Rejected
            });
        }
        values
            .remove("code")
            .filter(|code| atom(code, 8192))
            .ok_or(OAuthError::InvalidCallback)
    }
}

/// Owned and non-Clone: an ambiguously consumed code cannot be exchanged again by this owner.
pub struct CodeGrant {
    pub(super) authorization: Authorization,
    pub(super) code: String,
}

fn entropy() -> Result<String, OAuthError> {
    let mut bytes = [0u8; 32];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| OAuthError::Entropy)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

impl std::fmt::Debug for Authorization {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LumaAuthorization { material: <redacted> }")
    }
}
impl std::fmt::Debug for CodeGrant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LumaCodeGrant { material: <redacted> }")
    }
}
