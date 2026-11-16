//! PKCE/state/nonce preparation only; registry and callback listener own cancellation and one-shot use.
use super::{site::atom, Metadata, OAuthClient, OAuthError};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ring::rand::{SecureRandom, SystemRandom};
use sha2::{Digest, Sha256};
use std::time::Duration;
use subtle::ConstantTimeEq;
use tokio::time::Instant;

pub struct Authorization {
    pub(super) metadata: Metadata,
    pub(super) redirect: String,
    pub(super) verifier: String,
    pub(super) state: String,
    pub(super) nonce: String,
    pub(super) deadline: Instant,
}
impl Authorization {
    pub fn new(metadata: Metadata, redirect: &str) -> Result<Self, OAuthError> {
        Self::callback_port(redirect)?;
        Ok(Self {
            metadata,
            redirect: redirect.into(),
            verifier: entropy()?,
            state: entropy()?,
            nonce: entropy()?,
            deadline: Instant::now() + Duration::from_secs(300),
        })
    }
    /// Validate registration before discovery or binding; never substitute an ephemeral URI.
    pub fn callback_port(redirect: &str) -> Result<u16, OAuthError> {
        if redirect.len() > 512 || redirect.chars().any(char::is_control) {
            return Err(OAuthError::InvalidInput);
        }
        let url = url::Url::parse(redirect).map_err(|_| OAuthError::InvalidInput)?;
        if url.scheme() != "http"
            || url.host_str() != Some("127.0.0.1")
            || !url.port().is_some_and(|p| p != 0)
            || url.path() != "/oauth/callback"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.as_str() != redirect
        {
            return Err(OAuthError::InvalidInput);
        }
        Ok(url.port().expect("validated explicit loopback port"))
    }
    pub fn client(&self) -> &OAuthClient {
        self.metadata.client()
    }
    pub fn redirect_uri(&self) -> &str {
        &self.redirect
    }
    pub fn nonce(&self) -> &str {
        &self.nonce
    }
    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }
    pub fn authorize_url(&self) -> String {
        let mut url =
            url::Url::parse(&self.metadata.authorization_endpoint).expect("validated metadata");
        let scope = self.client().scopes().join(" ");
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(self.verifier.as_bytes()));
        url.query_pairs_mut().extend_pairs([
            ("response_type", "code"),
            ("client_id", self.client().client_id()),
            ("redirect_uri", &self.redirect),
            ("scope", &scope),
            ("state", &self.state),
            ("nonce", &self.nonce),
            ("code_challenge", &challenge),
            ("code_challenge_method", "S256"),
        ]);
        url.into()
    }
    /// Validation is non-consuming so a wrong browser callback cannot destroy the pending attempt.
    pub fn callback_code(&self, callback: &str) -> Result<String, OAuthError> {
        if self.remaining().is_zero() {
            return Err(OAuthError::Expired);
        }
        if callback.len() > 16_384 || callback.chars().any(char::is_control) {
            return Err(OAuthError::InvalidCallback);
        }
        let mut url = url::Url::parse(callback).map_err(|_| OAuthError::InvalidCallback)?;
        if url.fragment().is_some() {
            return Err(OAuthError::InvalidCallback);
        }
        let mut params = std::collections::HashMap::new();
        for (key, value) in url.query_pairs() {
            if params.len() >= 16
                || params
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
        let state = params.get("state").ok_or(OAuthError::StateMismatch)?;
        if !bool::from(state.as_bytes().ct_eq(self.state.as_bytes())) {
            return Err(OAuthError::StateMismatch);
        }
        if params.contains_key("code") && params.contains_key("error") {
            return Err(OAuthError::InvalidCallback);
        }
        if params
            .get("iss")
            .is_some_and(|issuer| issuer != self.client().site().issuer())
        {
            return Err(OAuthError::InvalidCallback);
        }
        if let Some(error) = params.get("error") {
            return Err(if error == "access_denied" {
                OAuthError::AccessDenied
            } else {
                OAuthError::Rejected
            });
        }
        params
            .remove("code")
            .filter(|code| atom(code, 8192))
            .ok_or(OAuthError::InvalidCallback)
    }
    pub fn into_code_grant(self, callback: &str) -> Result<CodeGrant, OAuthError> {
        let code = self.callback_code(callback)?;
        Ok(CodeGrant {
            authorization: self,
            code,
        })
    }
}
/// A caller must transfer this owned grant to a single exchange, never re-create it after I/O.
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
        f.write_str("DimAuthorization { material: <redacted> }")
    }
}
impl std::fmt::Debug for CodeGrant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DimCodeGrant { material: <redacted> }")
    }
}
