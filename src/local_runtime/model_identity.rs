//! Model-health identity compatibility. No management activation endpoint is exposed.
//!
//! The alias registry also grants access-key identity equivalence. A future rename
//! coordinator must prove both IDs own the same provider, reserve the old ID and
//! cover routing, audits and in-memory health. Never run it via live SQL.
use super::{storage_error, LocalRuntime};
use crate::db::provider_credential_model_states::{
    provider_credential_model_state_id, GatewayProviderCredentialModelStateView,
};
use crate::error::GatewayError;
use sqlx::SqliteConnection;

pub(super) const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS model_provider_aliases (
    alias TEXT PRIMARY KEY, canonical TEXT NOT NULL CHECK(alias <> canonical));
CREATE TABLE IF NOT EXISTS model_state_aliases (
    alias TEXT PRIMARY KEY, canonical TEXT NOT NULL CHECK(alias <> canonical));";

pub(super) async fn resolve(
    connection: &mut SqliteConnection,
    provider: &str,
) -> Result<String, GatewayError> {
    let target: Option<String> =
        sqlx::query_scalar("SELECT canonical FROM model_provider_aliases WHERE alias=?")
            .bind(provider)
            .fetch_optional(connection)
            .await
            .map_err(storage_error)?;
    Ok(target.unwrap_or_else(|| provider.to_owned()))
}

impl LocalRuntime {
    pub(crate) async fn canonical_model_provider(&self, id: &str) -> Result<String, GatewayError> {
        resolve(&mut *self.pool.acquire().await.map_err(storage_error)?, id).await
    }
}

fn invalid() -> GatewayError {
    GatewayError::bad_request("Model identity rename conflicts with existing state")
        .with_code("model_identity_rename_conflict")
}

/// Internal migration primitive, not exposed to HTTP/config commits. Publication
/// also changes key authorization, so callers must not treat this as health-only.
/// One short transaction serializes old/new request completions. Refuse large
/// inventories rather than holding SQLite's only writer for an unbounded scan.
#[allow(dead_code)]
pub(super) async fn rename(
    local: &LocalRuntime,
    old: &str,
    new: &str,
) -> Result<u64, GatewayError> {
    if old.is_empty()
        || new.is_empty()
        || old.trim() != old
        || new.trim() != new
        || old == new
        || old.len() > 256
        || new.len() > 256
        || old.chars().any(char::is_control)
        || new.chars().any(char::is_control)
    {
        return Err(invalid());
    }
    let mut tx = local
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(storage_error)?;
    let existing = resolve(&mut tx, old).await?;
    if existing == new {
        return Ok(0);
    }
    let occupied: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM model_provider_aliases
         WHERE alias IN (?,?) OR canonical IN (?,?)",
    )
    .bind(old)
    .bind(new)
    .bind(old)
    .bind(new)
    .fetch_one(&mut *tx)
    .await
    .map_err(storage_error)?;
    if occupied != 0 {
        return Err(invalid());
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM model_provider_aliases")
        .fetch_one(&mut *tx)
        .await
        .map_err(storage_error)?;
    if count >= 256 {
        return Err(invalid());
    }
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT id,payload FROM credential_model_states LIMIT 257")
            .fetch_all(&mut *tx)
            .await
            .map_err(storage_error)?;
    if rows.len() > 256 {
        return Err(invalid());
    }
    let mut changed = 0;
    for (id, payload) in rows {
        let mut row: GatewayProviderCredentialModelStateView =
            serde_json::from_str(&payload).map_err(|_| invalid())?;
        if row.provider_account_id == new {
            return Err(invalid());
        }
        if row.provider_account_id != old {
            continue;
        }
        if row.id != id || state_id(&row) != id {
            return Err(invalid());
        }
        row.provider_account_id = new.to_owned();
        row.id = state_id(&row);
        let encoded = serde_json::to_string(&row).map_err(|_| invalid())?;
        // Payload changes invalidate pre-rename CAS checkpoints, preserving all
        // health fields and their original timestamps rather than resetting them.
        sqlx::query("UPDATE credential_model_states SET id=?,payload=? WHERE id=?")
            .bind(&row.id)
            .bind(encoded)
            .bind(&id)
            .execute(&mut *tx)
            .await
            .map_err(storage_error)?;
        sqlx::query("INSERT INTO model_state_aliases(alias,canonical) VALUES (?,?)")
            .bind(id)
            .bind(&row.id)
            .execute(&mut *tx)
            .await
            .map_err(storage_error)?;
        changed += 1;
    }
    sqlx::query("INSERT INTO model_provider_aliases(alias,canonical) VALUES (?,?)")
        .bind(old)
        .bind(new)
        .execute(&mut *tx)
        .await
        .map_err(storage_error)?;
    tx.commit().await.map_err(storage_error)?;
    Ok(changed)
}

fn state_id(row: &GatewayProviderCredentialModelStateView) -> String {
    provider_credential_model_state_id(
        &row.provider_account_id,
        row.provider_credential_id.as_deref(),
        row.provider_credential_ref.as_deref(),
        row.protocol_profile.as_deref(),
        &row.model,
    )
}

#[cfg(test)]
mod guards_tests;
#[cfg(test)]
mod tests;
