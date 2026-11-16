//! Single-use PKCE capability with a fixed loopback destination and monotonic login deadline.
//! A future management session must additionally bind operator, provider, Secret Grant and revision.
use super::{valid_atom, OAuthClient, OAuthError};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
use subtle::ConstantTimeEq;
use url::Url;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginSite {
    China,
    Global,
}
impl LoginSite {
    fn login_url(self) -> &'static str {
        match self {
            Self::China => "https://www.accio-ai.com/login",
            Self::Global => "https://zh.accio.com/login",
        }
    }
}

// Neither capability is Clone or serializable; consuming even an invalid callback ends this attempt.
pub struct AuthorizationAttempt {
    client: OAuthClient,
    site: LoginSite,
    redirect: String,
    verifier: String,
    state: String,
    ttid: String,
    deadline: Instant,
}
pub struct CodeGrant {
    pub(super) client: OAuthClient,
    pub(super) site: LoginSite,
    pub(super) redirect: String,
    pub(super) verifier: String,
    pub(super) code: String,
    pub(super) deadline: Instant,
}

impl AuthorizationAttempt {
    /// The listener owner binds 127.0.0.1:0 first, then supplies its actual nonzero port.
    pub fn new(client: OAuthClient, site: LoginSite, port: u16) -> Result<Self, OAuthError> {
        if port == 0 {
            return Err(OAuthError::InvalidInput);
        }
        Ok(Self {
            client,
            site,
            redirect: format!("http://127.0.0.1:{port}/auth/callback"),
            verifier: URL_SAFE_NO_PAD.encode(random::<48>()?),
            state: hex::encode(random::<32>()?),
            ttid: format!("ttid_{}", hex::encode(random::<16>()?)),
            deadline: Instant::now() + Duration::from_secs(600),
        })
    }
    pub fn redirect_uri(&self) -> &str {
        &self.redirect
    }
    pub fn authorize_url(&self) -> String {
        let mut url = Url::parse(self.site.login_url()).expect("fixed Accio login URL");
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(self.verifier.as_bytes()));
        url.query_pairs_mut().extend_pairs([
            ("return_url", self.redirect.as_str()),
            ("state", &self.state),
            ("code_challenge", &challenge),
            ("code_challenge_method", "S256"),
            ("client_id", self.client.client_id()),
            ("ttid", &self.ttid),
            ("language", "zh"),
        ]);
        url.into()
    }
    /// No callback or authorization code is included in errors. No HTTP is sent here.
    pub fn finish(self, callback: &str) -> Result<CodeGrant, OAuthError> {
        if Instant::now() >= self.deadline {
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
            if !matches!(
                key.as_ref(),
                "code" | "state" | "error" | "error_description"
            ) || values
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
        if values.contains_key("code") && values.contains_key("error") {
            return Err(OAuthError::InvalidCallback);
        }
        if let Some(error) = values.get("error") {
            return Err(if error == "access_denied" {
                OAuthError::AccessDenied
            } else {
                OAuthError::Rejected
            });
        }
        if values.contains_key("error_description") {
            return Err(OAuthError::InvalidCallback);
        }
        let code = values.remove("code").ok_or(OAuthError::InvalidCallback)?;
        if !valid_atom(&code, 8192) {
            return Err(OAuthError::InvalidCallback);
        }
        Ok(CodeGrant {
            client: self.client,
            site: self.site,
            redirect: self.redirect,
            verifier: self.verifier,
            code,
            deadline: self.deadline,
        })
    }
}
impl CodeGrant {
    pub fn login_site(&self) -> LoginSite {
        self.site
    }
    pub(super) fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }
}
fn random<const N: usize>() -> Result<[u8; N], OAuthError> {
    let mut bytes = [0; N];
    rand::rngs::OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(|_| OAuthError::Entropy)?;
    Ok(bytes)
}
impl std::fmt::Debug for AuthorizationAttempt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AccioAuthorizationAttempt { material: <redacted> }")
    }
}
impl std::fmt::Debug for CodeGrant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AccioCodeGrant { material: <redacted> }")
    }
}

#[cfg(test)]
impl AuthorizationAttempt {
    pub(super) fn expire(&mut self) {
        self.deadline = Instant::now();
    }
}
