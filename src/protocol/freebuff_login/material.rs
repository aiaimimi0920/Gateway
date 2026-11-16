//! Server-returned session material bound to this attempt; no serialization, inferred expiry or identity proof.
use super::{atom, text, LoginError};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct UserWire {
    id: String,
    email: String,
    auth_token: String,
    fingerprint_id: String,
    fingerprint_hash: String,
}
pub struct Credentials {
    user_id: String,
    email: String,
    auth_token: String,
    fingerprint_id: String,
    fingerprint_hash: String,
}
impl Credentials {
    pub(super) fn accept(
        wire: UserWire,
        fingerprint: &str,
        hash: &str,
    ) -> Result<Self, LoginError> {
        if !text(&wire.id, 512)
            || !text(&wire.email, 512)
            || !atom(&wire.auth_token, 32_768)
            || wire.fingerprint_id != fingerprint
            || wire.fingerprint_hash != hash
        {
            return Err(LoginError::InvalidResponse);
        }
        Ok(Self {
            user_id: wire.id,
            email: wire.email,
            auth_token: wire.auth_token,
            fingerprint_id: wire.fingerprint_id,
            fingerprint_hash: wire.fingerprint_hash,
        })
    }
    pub fn auth_token(&self) -> &str {
        &self.auth_token
    }
    pub fn fingerprint_id(&self) -> &str {
        &self.fingerprint_id
    }
    pub fn fingerprint_hash(&self) -> &str {
        &self.fingerprint_hash
    }
    pub fn user_id(&self) -> &str {
        &self.user_id
    }
    pub fn email(&self) -> &str {
        &self.email
    }
}
impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FreeBuffCredentials { material: <redacted> }")
    }
}
