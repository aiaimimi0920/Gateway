//! Recoverable local key copies, separate from the authentication digest and public catalog.
use super::*;
use crate::local_runtime::secret_seal::LocalSecretSeal;

#[derive(Clone, Default)]
pub(crate) struct KeySecretStore(pub(in crate::local_runtime) LocalSecretSeal);

fn unavailable() -> GatewayError {
    GatewayError::service_unavailable(
        "密钥加密存储不可用，请恢复原数据目录中的 access-key-seal.json。",
    )
    .with_code("access_key_secret_unavailable")
}

impl KeySecretStore {
    fn seal(&self, id: &str, token: &str) -> Result<String, GatewayError> {
        let binding = format!("gateway-access-key:v1:{id}:{}", digest(token));
        self.0
            .seal(binding.as_bytes(), token.as_bytes(), 512)
            .map_err(|_| unavailable())
    }

    fn open_token(&self, id: &str, token_hash: &str, sealed: &str) -> Result<String, GatewayError> {
        let binding = format!("gateway-access-key:v1:{id}:{token_hash}");
        let plaintext = self
            .0
            .open_bytes(binding.as_bytes(), sealed, 1024, 512)
            .map_err(|_| unavailable())?;
        let token = String::from_utf8(plaintext).map_err(|_| unavailable())?;
        if digest(&token) != token_hash {
            return Err(unavailable());
        }
        Ok(token)
    }
}

#[cfg(test)]
#[path = "secret_compatibility_tests.rs"]
mod compatibility;

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
