//! Explicit Ed25519 import. No generation, implicit key replacement, disk writes or login fingerprint reuse.
use super::{account::Binding, CatalogError};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ring::signature::{Ed25519KeyPair, KeyPair};
use rquest::header::{HeaderMap, HeaderValue};
use std::sync::Arc;

pub struct DeviceKey(Ed25519KeyPair);
impl DeviceKey {
    pub fn from_pkcs8(private: &[u8], public: &[u8]) -> Result<Self, CatalogError> {
        if private.len() > 4096 || public.len() != 32 {
            return Err(CatalogError::InvalidKey);
        }
        // WebCrypto may export PKCS#8 v1 without a public field. ring derives it from the seed;
        // v2 also verifies the embedded public key. Both must match this separately supplied raw key.
        let pair = Ed25519KeyPair::from_pkcs8_maybe_unchecked(private)
            .map_err(|_| CatalogError::InvalidKey)?;
        if pair.public_key().as_ref() != public {
            return Err(CatalogError::InvalidKey);
        }
        Ok(Self(pair))
    }
    pub fn public_key(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.0.public_key().as_ref())
    }
    pub(super) fn sign(
        &self,
        method: &str,
        path: &str,
        body: &[u8],
        fetch_id: Option<&str>,
        timestamp: u64,
    ) -> String {
        // Only internal, fixed-path owners call this, after final serialization; origin is bound separately.
        let hash = hex::encode(ring::digest::digest(&ring::digest::SHA256, body));
        let payload = format!(
            "freebuff-device-v1\n{}\n{path}\n{timestamp}\n{hash}\n{}",
            method.to_ascii_uppercase(),
            fetch_id.unwrap_or("")
        );
        URL_SAFE_NO_PAD.encode(self.0.sign(payload.as_bytes()).as_ref())
    }
}
impl std::fmt::Debug for DeviceKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FreeBuffDeviceKey { material: <redacted> }")
    }
}

/// An in-memory registration capability. It cannot be deserialized or rebound by changing token/URL fields.
pub struct DeviceRegistration {
    pub(super) scope: [u8; 32],
    pub(super) key: Arc<DeviceKey>,
    pub(super) key_id: String,
}
impl DeviceRegistration {
    pub(super) fn new(binding: &Binding, key: Arc<DeviceKey>, key_id: String) -> Self {
        Self {
            scope: binding.scope,
            key,
            key_id,
        }
    }
    pub(super) fn check_binding(&self, binding: &Binding) -> Result<(), CatalogError> {
        if self.scope == binding.scope {
            Ok(())
        } else {
            Err(CatalogError::ScopeMismatch)
        }
    }
    pub(super) fn catalog_headers(
        &self,
        binding: &Binding,
        timestamp: u64,
    ) -> Result<HeaderMap, CatalogError> {
        // A catalog GET has neither body nor fetch id, even after a previous catalog was held.
        self.request_headers(
            binding,
            "GET",
            super::transport::CATALOG_PATH,
            b"",
            None,
            timestamp,
        )
    }
    pub(super) fn request_headers(
        &self,
        binding: &Binding,
        method: &str,
        path: &str,
        body: &[u8],
        fetch_id: Option<&str>,
        timestamp: u64,
    ) -> Result<HeaderMap, CatalogError> {
        self.check_binding(binding)?;
        let signature = self.key.sign(method, path, body, fetch_id, timestamp);
        let mut headers = HeaderMap::new();
        for (name, value) in [
            ("x-freebuff-device-key", self.key_id.clone()),
            ("x-freebuff-device-ts", timestamp.to_string()),
            ("x-freebuff-device-sig", signature),
        ] {
            let mut value =
                HeaderValue::from_str(&value).expect("validated or locally encoded header");
            value.set_sensitive(true);
            headers.insert(name, value);
        }
        Ok(headers)
    }
}
impl std::fmt::Debug for DeviceRegistration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FreeBuffDeviceRegistration { material: <redacted> }")
    }
}
