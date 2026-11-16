//! Grok's observed ES256 JWS profile: no token-provided keys, URLs or algorithm negotiation.
use super::super::{material::parse_reply, valid_atom, OAuthError};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ring::signature::{UnparsedPublicKey, ECDSA_P256_SHA256_FIXED};
use serde_json::Value;

pub(super) struct Token<'a> {
    signed: &'a str,
    payload: Vec<u8>,
    signature: Vec<u8>,
    kid: String,
}
impl<'a> Token<'a> {
    pub(super) fn parse(token: &'a str) -> Result<Self, OAuthError> {
        if token.is_empty() || token.len() > 32_768 {
            return Err(OAuthError::InvalidIdentity);
        }
        let mut parts = token.split('.');
        let header = parts.next().ok_or(OAuthError::InvalidIdentity)?;
        let payload = parts.next().ok_or(OAuthError::InvalidIdentity)?;
        let signature = parts.next().ok_or(OAuthError::InvalidIdentity)?;
        if parts.next().is_some() {
            return Err(OAuthError::InvalidIdentity);
        }
        let header = URL_SAFE_NO_PAD
            .decode(header)
            .map_err(|_| OAuthError::InvalidIdentity)?;
        let header = parse_reply(&header).map_err(|_| OAuthError::InvalidIdentity)?;
        let kid = header
            .get("kid")
            .and_then(Value::as_str)
            .filter(|s| valid_atom(s, 256))
            .ok_or(OAuthError::InvalidIdentity)?
            .to_owned();
        if header.get("alg").and_then(Value::as_str) != Some("ES256")
            || ["crit", "b64", "jku", "jwk", "x5u", "x5c"]
                .iter()
                .any(|key| header.get(key).is_some())
        {
            return Err(OAuthError::InvalidIdentity);
        }
        let payload = URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|_| OAuthError::InvalidIdentity)?;
        let signature = URL_SAFE_NO_PAD
            .decode(signature)
            .map_err(|_| OAuthError::InvalidIdentity)?;
        if signature.len() != 64 {
            return Err(OAuthError::InvalidIdentity);
        }
        Ok(Self {
            signed: token.rsplit_once('.').ok_or(OAuthError::InvalidIdentity)?.0,
            payload,
            signature,
            kid,
        })
    }
    pub(super) fn verify(self, keys: &[u8]) -> Result<Value, OAuthError> {
        let key = super::jwks::public_key(keys, &self.kid)?;
        UnparsedPublicKey::new(&ECDSA_P256_SHA256_FIXED, key)
            .verify(self.signed.as_bytes(), &self.signature)
            .map_err(|_| OAuthError::InvalidIdentity)?;
        parse_reply(&self.payload).map_err(|_| OAuthError::InvalidIdentity)
    }
}
