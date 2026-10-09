//! Recoverable local key copies, separate from the authentication digest and public catalog.
use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use ring::{
    aead,
    rand::{SecureRandom, SystemRandom},
};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
    sync::Arc,
};

#[derive(Clone, Default)]
pub(crate) struct KeySecretStore(Option<Arc<aead::LessSafeKey>>);

fn unavailable() -> GatewayError {
    GatewayError::service_unavailable(
        "密钥加密存储不可用，请恢复原数据目录中的 access-key-seal.json。",
    )
    .with_code("access_key_secret_unavailable")
}

impl KeySecretStore {
    // Called under the startup SQLite writer transaction; concurrent instances cannot re-key it.
    pub(crate) fn open(directory: &Path, has_sealed: bool) -> Self {
        Self(Self::load(directory, has_sealed).ok().map(Arc::new))
    }

    fn load(directory: &Path, has_sealed: bool) -> Result<aead::LessSafeKey, GatewayError> {
        let path = directory.join("access-key-seal.json");
        match fs::symlink_metadata(&path) {
            Ok(metadata) => {
                if !metadata.is_file() || metadata.file_type().is_symlink() {
                    return Err(unavailable());
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if metadata.file_attributes() & 0x400 != 0 {
                        return Err(unavailable());
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && !has_sealed => {
                let mut key = [0u8; 32];
                SystemRandom::new()
                    .fill(&mut key)
                    .map_err(|_| unavailable())?;
                let encoded =
                    serde_json::to_vec(&STANDARD.encode(key)).map_err(|_| unavailable())?;
                let mut options = OpenOptions::new();
                options.write(true).create_new(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.mode(0o600);
                }
                let mut file = options.open(&path).map_err(|_| unavailable())?;
                file.write_all(&encoded)
                    .and_then(|_| file.sync_all())
                    .map_err(|_| unavailable())?;
            }
            Err(_) => return Err(unavailable()),
        }
        let mut bytes = Vec::new();
        fs::File::open(&path)
            .map_err(|_| unavailable())?
            .take(257)
            .read_to_end(&mut bytes)
            .map_err(|_| unavailable())?;
        if bytes.len() > 256 {
            return Err(unavailable());
        }
        let encoded: String = serde_json::from_slice(&bytes).map_err(|_| unavailable())?;
        let key = STANDARD.decode(encoded).map_err(|_| unavailable())?;
        aead::UnboundKey::new(&aead::AES_256_GCM, &key)
            .map(aead::LessSafeKey::new)
            .map_err(|_| unavailable())
    }

    fn seal(&self, id: &str, token: &str) -> Result<String, GatewayError> {
        let key = self.0.as_ref().ok_or_else(unavailable)?;
        let mut nonce = [0u8; 12];
        SystemRandom::new()
            .fill(&mut nonce)
            .map_err(|_| unavailable())?;
        let mut data = token.as_bytes().to_vec();
        let binding = format!("gateway-access-key:v1:{id}:{}", digest(token));
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

    fn open_token(&self, id: &str, token_hash: &str, sealed: &str) -> Result<String, GatewayError> {
        let key = self.0.as_ref().ok_or_else(unavailable)?;
        if sealed.len() > 1024 {
            return Err(unavailable());
        }
        let mut bytes = STANDARD.decode(sealed).map_err(|_| unavailable())?;
        if !(28..=540).contains(&bytes.len()) {
            return Err(unavailable());
        }
        let (nonce, data) = bytes.split_at_mut(12);
        let binding = format!("gateway-access-key:v1:{id}:{token_hash}");
        let plaintext = key
            .open_in_place(
                aead::Nonce::try_assume_unique_for_key(nonce).map_err(|_| unavailable())?,
                aead::Aad::from(binding.as_bytes()),
                data,
            )
            .map_err(|_| unavailable())?;
        let token = String::from_utf8(plaintext.to_vec()).map_err(|_| unavailable())?;
        if digest(&token) != token_hash {
            return Err(unavailable());
        }
        Ok(token)
    }
}

pub(super) async fn store(
    tx: &mut Transaction<'_, Sqlite>,
    sealer: &KeySecretStore,
    id: &str,
    token: &str,
) -> Result<(), GatewayError> {
    let sealed = sealer.seal(id, token)?;
    sqlx::query("INSERT INTO local_access_key_secrets(key_id,sealed) VALUES (?,?) ON CONFLICT(key_id) DO NOTHING")
        .bind(id).bind(sealed).execute(&mut **tx).await.map_err(storage_error)?;
    Ok(())
}

impl LocalRuntime {
    pub async fn access_key_secret(&self, id: &str) -> Result<String, GatewayError> {
        let mut tx = self.pool.begin().await.map_err(storage_error)?;
        let key = read(&mut tx, id).await?;
        policy::ensure_active(&key)?;
        let (token_hash, sealed): (String, Option<String>) = sqlx::query_as("SELECT k.token_hash,s.sealed FROM local_access_keys k LEFT JOIN local_access_key_secrets s ON s.key_id=k.id WHERE k.id=?")
            .bind(id).fetch_one(&mut *tx).await.map_err(storage_error)?;
        let sealed = sealed.ok_or_else(|| {
            GatewayError::conflict(
                "旧密钥尚未保存可复制原文；客户端正常使用一次后可复制，或主动轮换密钥。",
            )
            .with_code("access_key_secret_not_saved")
        })?;
        let token = self.key_secrets.open_token(id, &token_hash, &sealed)?;
        tx.commit().await.map_err(storage_error)?;
        Ok(token)
    }

    pub(super) async fn capture_access_key_secret(
        &self,
        id: &str,
        token: &str,
    ) -> Result<(), GatewayError> {
        let mut tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let key = read(&mut tx, id).await?;
        policy::ensure_active(&key)?;
        let token_hash: String =
            sqlx::query_scalar("SELECT token_hash FROM local_access_keys WHERE id=?")
                .bind(id)
                .fetch_one(&mut *tx)
                .await
                .map_err(storage_error)?;
        if token_hash != digest(token) {
            return Err(unavailable());
        }
        store(&mut tx, &self.key_secrets, id, token).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(())
    }
}
