//! Single-use local capability; management identity and the listener belong to the session owner.
use super::{
    valid_atom, OAuthClient, OAuthError, AUTHORIZE_ENDPOINT, CLOUD_PLATFORM_SCOPE, ISSUER,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
use subtle::ConstantTimeEq;
use url::Url;

pub struct AuthorizationAttempt {
    pub(super) client: OAuthClient,
    pub(super) redirect: String,
    pub(super) verifier: String,
    pub(super) state: String,
    pub(super) deadline: Instant,
}
pub struct CodeGrant {
    pub(super) client: OAuthClient,
    pub(super) redirect: String,
    pub(super) verifier: String,
    pub(super) code: String,
    pub(super) deadline: Instant,
}
impl AuthorizationAttempt {
    /// Bind the loopback listener first, then pass its actual port. This does not create a listener.
    pub fn new(client: OAuthClient, port: u16) -> Result<Self, OAuthError> {
        if port == 0 {
            return Err(OAuthError::InvalidInput);
        }
        Ok(Self {
            client,
            redirect: Url::parse(&format!("http://127.0.0.1:{port}/"))
                .expect("fixed loopback redirect with nonzero port")
                .into(),
            verifier: random::<48>()?,
            state: random::<32>()?,
            deadline: Instant::now() + Duration::from_secs(600),
        })
    }
    pub fn redirect_uri(&self) -> &str {
        &self.redirect
    }
    pub fn authorize_url(&self) -> String {
        let mut url = Url::parse(AUTHORIZE_ENDPOINT).expect("fixed Google authorization endpoint");
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(self.verifier.as_bytes()));
        url.query_pairs_mut().extend_pairs([
            ("client_id", self.client.client_id()),
            ("redirect_uri", &self.redirect),
            ("response_type", "code"),
            ("scope", CLOUD_PLATFORM_SCOPE),
            ("access_type", "offline"),
            ("prompt", "consent"),
            ("state", &self.state),
            ("code_challenge", &challenge),
            ("code_challenge_method", "S256"),
        ]);
        url.into()
    }
    /// Consuming even a failed callback prevents retrying a capability outside its owning session.
    pub fn finish(self, callback: &str) -> Result<CodeGrant, OAuthError> {
        if self.deadline <= Instant::now() {
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
        if values.get("iss").is_some_and(|issuer| issuer != ISSUER)
            || (values.contains_key("code") && values.contains_key("error"))
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
        if values.contains_key("error_description") || values.contains_key("error_uri") {
            return Err(OAuthError::InvalidCallback);
        }
        let code = values.remove("code").ok_or(OAuthError::InvalidCallback)?;
        if !valid_atom(&code, 8192) {
            return Err(OAuthError::InvalidCallback);
        }
        Ok(CodeGrant {
            client: self.client,
            redirect: self.redirect,
            verifier: self.verifier,
            code,
            deadline: self.deadline,
        })
    }
}
impl CodeGrant {
    pub(super) fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }
}
fn random<const N: usize>() -> Result<String, OAuthError> {
    let mut bytes = [0; N];
    rand::rngs::OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(|_| OAuthError::Entropy)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
impl std::fmt::Debug for AuthorizationAttempt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VertexAuthorizationAttempt { material: <redacted> }")
    }
}
impl std::fmt::Debug for CodeGrant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VertexCodeGrant { material: <redacted> }")
    }
}
