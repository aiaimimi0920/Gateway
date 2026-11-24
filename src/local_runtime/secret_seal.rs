//! Shared local AES seal. The legacy filename and ciphertext framing remain byte-compatible.
//! This is a same-data-root key, not an OS vault or protection against a reader of the whole root.
use base64::{engine::general_purpose::STANDARD, Engine};
use hmac::{Hmac, Mac};
use ring::{
    aead,
    rand::{SecureRandom, SystemRandom},
};
use sha2::Sha256;
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
    sync::Arc,
};

struct SealKeys {
    aes: aead::LessSafeKey,
    sap_response: [u8; 32],
}

#[derive(Clone, Default)]
pub(super) struct LocalSecretSeal(Option<Arc<SealKeys>>);

impl LocalSecretSeal {
    // Startup holds SQLite's writer transaction, including the all-owner sealed-row check.
    pub(super) fn open(directory: &Path, has_sealed: bool) -> Self {
        Self(Self::load(directory, has_sealed).ok().map(Arc::new))
    }
    fn load(directory: &Path, has_sealed: bool) -> Result<SealKeys, ()> {
        let path = directory.join("access-key-seal.json");
        match fs::symlink_metadata(&path) {
            Ok(metadata) => {
                if !metadata.is_file() || metadata.file_type().is_symlink() {
                    return Err(());
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if metadata.file_attributes() & 0x400 != 0 {
                        return Err(());
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && !has_sealed => {
                let mut key = [0u8; 32];
                SystemRandom::new().fill(&mut key).map_err(|_| ())?;
                let encoded = serde_json::to_vec(&STANDARD.encode(key)).map_err(|_| ())?;
                let mut options = OpenOptions::new();
                options.write(true).create_new(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.mode(0o600);
                }
                let mut file = options.open(&path).map_err(|_| ())?;
                file.write_all(&encoded)
                    .and_then(|_| file.sync_all())
                    .map_err(|_| ())?;
            }
            Err(_) => return Err(()),
        }
        let mut bytes = Vec::new();
        fs::File::open(&path)
            .map_err(|_| ())?
            .take(257)
            .read_to_end(&mut bytes)
            .map_err(|_| ())?;
        if bytes.len() > 256 {
            return Err(());
        }
        let encoded: String = serde_json::from_slice(&bytes).map_err(|_| ())?;
        let key = STANDARD.decode(encoded).map_err(|_| ())?;
        let aes = aead::UnboundKey::new(&aead::AES_256_GCM, &key)
            .map(aead::LessSafeKey::new)
            .map_err(|_| ())?;
        let mut derive = Hmac::<Sha256>::new_from_slice(&key).map_err(|_| ())?;
        derive.update(b"neuro-gateway/sap-response-local/v1\0");
        Ok(SealKeys {
            aes,
            sap_response: derive.finalize().into_bytes().into(),
        })
    }

    pub(super) fn sap_response_key(&self) -> Option<&[u8; 32]> {
        self.0.as_ref().map(|keys| &keys.sap_response)
    }
    pub(super) fn seal(&self, aad: &[u8], bytes: &[u8], max: usize) -> Result<String, ()> {
        if bytes.len() > max {
            return Err(());
        }
        let key = &self.0.as_ref().ok_or(())?.aes;
        let mut nonce = [0u8; 12];
        SystemRandom::new().fill(&mut nonce).map_err(|_| ())?;
        let mut data = bytes.to_vec();
        key.seal_in_place_append_tag(
            aead::Nonce::assume_unique_for_key(nonce),
            aead::Aad::from(aad),
            &mut data,
        )
        .map_err(|_| ())?;
        let mut payload = nonce.to_vec();
        payload.extend(data);
        Ok(STANDARD.encode(payload))
    }
    pub(super) fn open_bytes(
        &self,
        aad: &[u8],
        sealed: &str,
        encoded_max: usize,
        plain_max: usize,
    ) -> Result<Vec<u8>, ()> {
        let key = &self.0.as_ref().ok_or(())?.aes;
        if sealed.len() > encoded_max {
            return Err(());
        }
        let mut bytes = STANDARD.decode(sealed).map_err(|_| ())?;
        if bytes.len() < 28 || bytes.len() > plain_max.saturating_add(28) {
            return Err(());
        }
        let (nonce, data) = bytes.split_at_mut(12);
        let plaintext = key
            .open_in_place(
                aead::Nonce::try_assume_unique_for_key(nonce).map_err(|_| ())?,
                aead::Aad::from(aad),
                data,
            )
            .map_err(|_| ())?;
        Ok(plaintext.to_vec())
    }
}
