//! Signed end-user identity only, not grant acceptance, management authority or quota principal proof.
mod claims;
mod fetch;
mod jwks;
mod jwt;
use super::{Authorization, OAuthClient, OAuthError, UnverifiedTokens};

/// Constructed only after signature and authorization-context validation. Never stores a raw token.
pub struct VerifiedIdentity {
    issuer: String,
    subject: String,
    client_id: String,
    expires_at: u64,
}
impl VerifiedIdentity {
    pub fn issuer(&self) -> &str {
        &self.issuer
    }
    pub fn subject(&self) -> &str {
        &self.subject
    }
    pub fn client_id(&self) -> &str {
        &self.client_id
    }
    pub fn expires_at(&self) -> u64 {
        self.expires_at
    }
}
impl std::fmt::Debug for VerifiedIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VerifiedGrokIdentity { identity: <redacted> }")
    }
}
impl OAuthClient {
    /// Verifies an ID token for this code-flow nonce. Missing-ID Team/device grants need their own
    /// explicit acceptance policy; this method does not reject or approve those grants as a whole.
    pub async fn verify_code_identity(
        &self,
        http: &rquest::Client,
        auth: &Authorization,
        tokens: &UnverifiedTokens,
    ) -> Result<VerifiedIdentity, OAuthError> {
        if auth.client != *self {
            return Err(OAuthError::InvalidInput);
        }
        let token = tokens
            .id_token
            .as_deref()
            .ok_or(OAuthError::InvalidIdentity)?;
        let parsed = jwt::Token::parse(token)?;
        let keys = fetch::key_set(self, http).await?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| OAuthError::InvalidIdentity)?
            .as_secs();
        let body = parsed.verify(&keys)?;
        claims::validate(self, body, auth.nonce(), &tokens.access_token, now)
    }
}

#[cfg(test)]
pub(super) fn verify_token(
    client: &OAuthClient,
    token: &str,
    nonce: &str,
    access_token: &str,
    jwks: &[u8],
    now: u64,
) -> Result<VerifiedIdentity, OAuthError> {
    let body = jwt::Token::parse(token)?.verify(jwks)?;
    claims::validate(client, body, nonce, access_token, now)
}
