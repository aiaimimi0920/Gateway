//! Recoverable display uses authenticated encryption; authentication remains hash based.
use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use ring::{
    aead,
    rand::{SecureRandom, SystemRandom},
};

fn unavailable() -> GatewayError {
    GatewayError::service_unavailable("Management key display storage is unavailable")
        .with_code("console_key_display_unavailable")
}

impl ConsoleAuthRuntime {
    fn display_key(
        &self,
        guard: Option<&WriterLockGuard>,
    ) -> Result<aead::LessSafeKey, GatewayError> {
        let path = self
            .persistence
            .state_root()
            .join("management-key-seal.json");
        self.persistence
            .validate_optional_managed_file(&path)
            .map_err(persistence_to_gateway_error)?;
        let encoded: String = match fs::read(&path) {
            Ok(bytes) if bytes.len() <= 256 => {
                serde_json::from_slice(&bytes).map_err(|_| unavailable())?
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let guard = guard.ok_or_else(unavailable)?;
                // Never replace a missing key for an existing encrypted ring.
                if self
                    .load_key_ring()?
                    .is_some_and(|ring| ring.keys.iter().any(|key| key.sealed_token.is_some()))
                {
                    return Err(unavailable());
                }
                let mut bytes = [0u8; 32];
                SystemRandom::new()
                    .fill(&mut bytes)
                    .map_err(|_| unavailable())?;
                let encoded = STANDARD.encode(bytes);
                self.persistence
                    .atomic_write_json_locked(guard, &path, &encoded)
                    .map_err(persistence_to_gateway_error)?;
                encoded
            }
            _ => return Err(unavailable()),
        };
        let bytes = STANDARD.decode(encoded).map_err(|_| unavailable())?;
        aead::UnboundKey::new(&aead::AES_256_GCM, &bytes)
            .map(aead::LessSafeKey::new)
            .map_err(|_| unavailable())
    }

    pub(super) fn seal_management_key(
        &self,
        guard: &WriterLockGuard,
        record: &StoredKey,
        token: &str,
    ) -> Result<String, GatewayError> {
        let key = self.display_key(Some(guard))?;
        let mut nonce = [0u8; 12];
        SystemRandom::new()
            .fill(&mut nonce)
            .map_err(|_| unavailable())?;
        let mut data = token.as_bytes().to_vec();
        let binding = format!(
            "gateway-management-key:v1:{}:{}",
            record.id, record.token_hash
        );
        key.seal_in_place_append_tag(
            aead::Nonce::assume_unique_for_key(nonce),
            aead::Aad::from(binding.as_bytes()),
            &mut data,
        )
        .map_err(|_| unavailable())?;
        let mut payload = nonce.to_vec();
        payload.extend(data);
        Ok(STANDARD.encode(payload))
    }

    pub(super) fn open_management_key(
        &self,
        record: &StoredKey,
        sealed: &str,
    ) -> Result<String, GatewayError> {
        let key = self.display_key(None)?;
        let mut payload = STANDARD.decode(sealed).map_err(|_| unavailable())?;
        if payload.len() < 28 || payload.len() > 4096 + 28 {
            return Err(unavailable());
        }
        let (nonce, ciphertext) = payload.split_at_mut(12);
        let nonce = aead::Nonce::try_assume_unique_for_key(nonce).map_err(|_| unavailable())?;
        let binding = format!(
            "gateway-management-key:v1:{}:{}",
            record.id, record.token_hash
        );
        let plaintext = key
            .open_in_place(nonce, aead::Aad::from(binding.as_bytes()), ciphertext)
            .map_err(|_| unavailable())?;
        String::from_utf8(plaintext.to_vec()).map_err(|_| unavailable())
    }
}
