//! Local operator-issued keys. Authentication uses hashes; managed copies use sealed secrets.
use super::{storage_error, LocalRuntime};
use crate::db::{
    DeleteAccessKeyResult, GatewayAccessCatalogView, GatewayAccessKeyView, UpsertAccessKeyInput,
};
use crate::error::GatewayError;
use sha2::{Digest, Sha256};
use sqlx::{Sqlite, Transaction};

pub(super) mod policy;
pub(super) mod secrets;
pub use policy::local_key_id;
#[cfg(test)]
mod editing_tests;
#[cfg(test)]
mod lifecycle_tests;
#[cfg(test)]
mod tests;

pub(super) const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS local_access_keys (
    id TEXT PRIMARY KEY, token_hash TEXT NOT NULL UNIQUE, payload TEXT NOT NULL);
    CREATE INDEX IF NOT EXISTS local_access_keys_order ON local_access_keys(id);
    CREATE TABLE IF NOT EXISTS local_access_key_secrets (key_id TEXT PRIMARY KEY REFERENCES local_access_keys(id) ON DELETE CASCADE, sealed TEXT NOT NULL);";
pub const ID_PREFIX: &str = "local-ak-";
const MAX_KEYS: i64 = 10_000;

fn digest(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

fn now() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .expect("UTC timestamp")
}

pub fn denied(message: &str) -> GatewayError {
    let mut error = GatewayError::unauthorized(message);
    error.http_status = Some(403);
    error
}

fn decode(payload: &str) -> Result<GatewayAccessKeyView, GatewayError> {
    serde_json::from_str(payload)
        .map_err(|_| GatewayError::server_error("Invalid local access key record"))
}

async fn read(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
) -> Result<GatewayAccessKeyView, GatewayError> {
    let payload: String = sqlx::query_scalar("SELECT payload FROM local_access_keys WHERE id = ?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(storage_error)?
        .ok_or_else(|| GatewayError::not_found("Access key not found"))?;
    decode(&payload)
}

async fn write(
    tx: &mut Transaction<'_, Sqlite>,
    key: &GatewayAccessKeyView,
) -> Result<(), GatewayError> {
    let mut safe = key.clone();
    safe.token = None;
    safe.external_key = None;
    let payload = serde_json::to_string(&safe)
        .map_err(|_| GatewayError::server_error("Cannot encode access key"))?;
    sqlx::query("UPDATE local_access_keys SET payload = ? WHERE id = ?")
        .bind(payload)
        .bind(&key.id)
        .execute(&mut **tx)
        .await
        .map_err(storage_error)?;
    Ok(())
}

async fn insert(
    tx: &mut Transaction<'_, Sqlite>,
    key: &mut GatewayAccessKeyView,
    sealer: &secrets::KeySecretStore,
) -> Result<(), GatewayError> {
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM local_access_keys")
        .fetch_one(&mut **tx)
        .await
        .map_err(storage_error)?;
    if count >= MAX_KEYS {
        return Err(GatewayError::conflict(
            "Local access key limit reached; delete unused keys",
        ));
    }
    key.id = format!("{ID_PREFIX}{}", uuid::Uuid::new_v4());
    // Two independent UUIDs provide 244 random bits, independent of the display encryption key.
    let token = format!(
        "{}-local-{}{}",
        key.public_key_prefix,
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    sqlx::query("INSERT INTO local_access_keys(id,token_hash,payload) VALUES (?,?, '{}')")
        .bind(&key.id)
        .bind(digest(&token))
        .execute(&mut **tx)
        .await
        .map_err(storage_error)?;
    write(tx, key).await?;
    secrets::store(tx, sealer, &key.id, &token).await?;
    super::access_balances::create(tx, &key.id, key.rotated_from_access_key_id.as_deref()).await?;
    key.token = Some(token);
    Ok(())
}

impl LocalRuntime {
    pub async fn access_key_metadata(
        &self,
        id: &str,
    ) -> Result<Option<serde_json::Value>, GatewayError> {
        let mut tx = self.pool.begin().await.map_err(storage_error)?;
        let key = read(&mut tx, id).await?;
        policy::ensure_active(&key)?;
        tx.commit().await.map_err(storage_error)?;
        Ok(key.metadata)
    }

    pub async fn access_catalog(&self) -> Result<GatewayAccessCatalogView, GatewayError> {
        let rows: Vec<String> =
            sqlx::query_scalar("SELECT payload FROM local_access_keys ORDER BY id LIMIT 10000")
                .fetch_all(&self.pool)
                .await
                .map_err(storage_error)?;
        Ok(GatewayAccessCatalogView {
            provider_capabilities: vec![],
            platform_access_rows: vec![],
            bundles: vec![],
            bundle_items: vec![],
            access_keys: rows
                .iter()
                .map(|row| decode(row))
                .collect::<Result<_, _>>()?,
            key_bundle_bindings: vec![],
            balances: self.access_balances().await?,
            aggregate_memberships: vec![],
        })
    }

    pub async fn save_access_key(
        &self,
        id: Option<&str>,
        input: UpsertAccessKeyInput,
    ) -> Result<GatewayAccessKeyView, GatewayError> {
        self.save_access_key_with_quota(id, input, None).await
    }

    pub async fn save_access_key_with_quota(
        &self,
        id: Option<&str>,
        input: UpsertAccessKeyInput,
        quota: Option<&crate::access_balance::quota::KeyQuotaInput>,
    ) -> Result<GatewayAccessKeyView, GatewayError> {
        policy::validate_input(&input)?;
        if let Some(quota) = quota {
            quota.validate()?;
        }
        let mut tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let timestamp = now();
        let mut key = if let Some(id) = id {
            let existing = read(&mut tx, id).await?;
            policy::ensure_active(&existing)?;
            if existing.resolved_project_id != input.resolved_project_id.trim()
                || existing.resolved_tenant_id != input.resolved_tenant_id.trim()
                || existing.public_key_prefix != input.public_key_prefix.trim()
            {
                return Err(GatewayError::bad_request(
                    "Key identity and prefix are immutable; issue a new key",
                ));
            }
            existing
        } else {
            GatewayAccessKeyView {
                id: String::new(),
                owner_type: String::new(),
                owner_id: String::new(),
                resolved_project_id: input.resolved_project_id.trim().into(),
                resolved_tenant_id: input.resolved_tenant_id.trim().into(),
                key_kind: "normal".into(),
                status: "active".into(),
                public_key_prefix: input.public_key_prefix.trim().into(),
                display_name: String::new(),
                token: None,
                external_key: None,
                rotated_from_access_key_id: None,
                legacy_gateway_api_key_id: None,
                legacy_user_credential_id: None,
                expires_at: None,
                last_used_at: None,
                metadata: None,
                revoked_at: None,
                revoke_reason: None,
                created_at: timestamp.clone(),
                updated_at: timestamp.clone(),
            }
        };
        key.owner_type = input.owner_type.trim().into();
        key.owner_id = input.owner_id.trim().into();
        key.display_name = input.display_name.trim().into();
        key.expires_at = input.expires_at;
        key.metadata = input.metadata;
        key.updated_at = timestamp;
        if id.is_none() {
            insert(&mut tx, &mut key, &self.key_secrets).await?;
        } else {
            write(&mut tx, &key).await?;
        }
        if let Some(quota) = quota {
            super::access_balances::set_key_quota(&mut tx, &key.id, quota).await?;
        }
        tx.commit().await.map_err(storage_error)?;
        Ok(key)
    }

    pub async fn rotate_access_key(&self, id: &str) -> Result<GatewayAccessKeyView, GatewayError> {
        let mut tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let mut old = read(&mut tx, id).await?;
        policy::ensure_active(&old)?;
        let mut replacement = old.clone();
        replacement.rotated_from_access_key_id = Some(old.id.clone());
        replacement.created_at = now();
        replacement.updated_at = replacement.created_at.clone();
        replacement.last_used_at = None;
        old.status = "revoked".into();
        old.revoked_at = Some(now());
        old.revoke_reason = Some("rotated".into());
        old.updated_at = now();
        write(&mut tx, &old).await?;
        insert(&mut tx, &mut replacement, &self.key_secrets).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(replacement)
    }

    pub async fn revoke_access_key(
        &self,
        id: &str,
        reason: Option<&str>,
    ) -> Result<(), GatewayError> {
        if reason.is_some_and(|value| value.len() > 1024) {
            return Err(GatewayError::bad_request("Revoke reason is too long"));
        }
        let mut tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let mut key = read(&mut tx, id).await?;
        if key.status != "revoked" {
            key.status = "revoked".into();
            key.revoked_at = Some(now());
            key.revoke_reason = reason.map(str::to_owned);
            key.updated_at = now();
            write(&mut tx, &key).await?;
        }
        tx.commit().await.map_err(storage_error)?;
        Ok(())
    }

    pub async fn set_access_key_enabled(
        &self,
        id: &str,
        enabled: bool,
    ) -> Result<(), GatewayError> {
        let mut tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let mut key = read(&mut tx, id).await?;
        // Revocation (including rotation) is terminal; toggling must not resurrect old tokens.
        if key.revoked_at.is_some() || !matches!(key.status.as_str(), "active" | "disabled") {
            return Err(GatewayError::conflict(
                "Only active or disabled keys can be toggled",
            ));
        }
        key.status = if enabled { "active" } else { "disabled" }.into();
        key.updated_at = now();
        write(&mut tx, &key).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(())
    }

    pub async fn delete_access_key(&self, id: &str) -> Result<DeleteAccessKeyResult, GatewayError> {
        let mut tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let key = read(&mut tx, id).await?;
        sqlx::query("DELETE FROM local_access_keys WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(storage_error)?;
        super::access_balances::cleanup(&mut tx).await?;
        tx.commit().await.map_err(storage_error)?;
        Ok(DeleteAccessKeyResult {
            access_key_id: key.id,
            display_name: key.display_name,
        })
    }
}
