//! PKCE and callback parsing only. Session owner must enforce TTL, one-shot use and management auth.
use super::{valid_atom, OAuthClient, OAuthError};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use url::Url;

pub struct Authorization {
    pub(super) client: OAuthClient,
    pub(super) redirect_uri: String,
    pub(super) verifier: String,
    pub(super) state: String,
    pub(super) nonce: String,
}
impl Authorization {
    pub fn new(client: &OAuthClient, redirect_uri: &str) -> Result<Self, OAuthError> {
        if redirect_uri.len() > 256 || redirect_uri.chars().any(char::is_control) {
            return Err(OAuthError::InvalidInput);
        }
        let url = Url::parse(redirect_uri).map_err(|_| OAuthError::InvalidInput)?;
        if url.scheme() != "http"
            || !matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))
            || url.port().is_none()
            || url.path() != "/callback"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(OAuthError::InvalidInput);
        }
        Ok(Self {
            client: client.clone(),
            redirect_uri: url.into(),
            verifier: random_value(),
            state: random_value(),
            nonce: random_value(),
        })
    }
    pub fn nonce(&self) -> &str {
        &self.nonce
    }
    pub fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }
    pub fn authorize_url(&self) -> String {
        let client = &self.client;
        let mut url =
            Url::parse(&format!("{}/oauth2/authorize", client.issuer)).expect("fixed issuer");
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(self.verifier.as_bytes()));
        {
            let mut query = url.query_pairs_mut();
            query.extend_pairs([
                ("response_type", "code"),
                ("client_id", &client.client_id),
                ("redirect_uri", &self.redirect_uri),
                ("scope", &client.scope),
                ("code_challenge", &challenge),
                ("code_challenge_method", "S256"),
                ("state", &self.state),
                ("nonce", &self.nonce),
                ("referrer", "neuro-gateway"),
            ]);
            if let Some(principal) = &client.principal {
                query.extend_pairs([
                    ("principal_type", principal.kind()),
                    ("principal_id", principal.id()),
                ]);
            }
        }
        url.into()
    }
    pub fn callback_code(&self, callback: &str) -> Result<String, OAuthError> {
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
        if url.as_str() != self.redirect_uri {
            return Err(OAuthError::InvalidCallback);
        }
        let state = values.get("state").ok_or(OAuthError::StateMismatch)?;
        if !bool::from(self.state.as_bytes().ct_eq(state.as_bytes())) {
            return Err(OAuthError::StateMismatch);
        }
        if values.contains_key("code") && values.contains_key("error") {
            return Err(OAuthError::InvalidCallback);
        }
        if let Some(issuer) = values.get("iss") {
            if issuer != &self.client.issuer {
                return Err(OAuthError::InvalidCallback);
            }
        }
        if let Some(error) = values.get("error") {
            return Err(if error == "access_denied" {
                OAuthError::AccessDenied
            } else {
                OAuthError::Rejected
            });
        }
        let code = values.remove("code").ok_or(OAuthError::InvalidCallback)?;
        if !valid_atom(&code, 8192) {
            return Err(OAuthError::InvalidCallback);
        }
        Ok(code)
    }
}
fn random_value() -> String {
    URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>())
}
impl std::fmt::Debug for Authorization {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GrokAuthorization { material: <redacted> }")
    }
}
