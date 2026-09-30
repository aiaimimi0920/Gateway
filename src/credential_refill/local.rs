//! SQLite refill persistence. Atomic SQL predicates arbitrate claims across processes.
use super::*;
use crate::{
    error::GatewayError,
    local_runtime::{storage_error, LocalRuntime},
};
use sqlx::{Sqlite, Transaction};
use time::OffsetDateTime;

fn now() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}
fn expiry(ttl: u64) -> i64 {
    now().saturating_add(i64::try_from(ttl).unwrap_or(i64::MAX))
}
fn decode(payload: &str) -> Result<CredentialRefillTaskRecord, GatewayError> {
    serde_json::from_str(payload).map_err(|_| {
        GatewayError::server_error("Local refill task is corrupt")
            .with_code("credential_refill_task_corrupt")
    })
}

pub(super) async fn create(
    db: &LocalRuntime,
    task: CredentialRefillTaskRecord,
    ttl: u64,
) -> Result<CreateCredentialRefillTaskResult, GatewayError> {
    db.prune_refill_tasks().await?;
    let mut tx = db
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(storage_error)?;
    let existing: Option<String> = sqlx::query_scalar(
        "SELECT payload FROM refill_tasks WHERE provider_id = ?
         AND (terminal = 0 OR idempotency_key = ?) ORDER BY terminal LIMIT 1",
    )
    .bind(&task.provider_id)
    .bind(&task.idempotency_key)
    .fetch_optional(&mut *tx)
    .await
    .map_err(storage_error)?;
    let (resolved, created) = if let Some(payload) = existing {
        (decode(&payload)?, false)
    } else {
        sqlx::query("INSERT INTO refill_tasks
            (id, provider_id, payload, created, expires, idempotency_key) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(&task.id).bind(&task.provider_id).bind(serialize_task(&task)?)
            .bind(now()).bind(expiry(ttl)).bind(&task.idempotency_key)
            .execute(&mut *tx).await.map_err(storage_error)?;
        (task, true)
    };
    tx.commit().await.map_err(storage_error)?;
    Ok(CreateCredentialRefillTaskResult {
        task: (&resolved).into(),
        created,
    })
}

pub(super) async fn load(
    db: &LocalRuntime,
    id: &str,
) -> Result<Option<CredentialRefillTaskRecord>, GatewayError> {
    let payload: Option<String> =
        sqlx::query_scalar("SELECT payload FROM refill_tasks WHERE id = ? AND expires > ?")
            .bind(id)
            .bind(now())
            .fetch_optional(&db.pool)
            .await
            .map_err(storage_error)?;
    payload.as_deref().map(decode).transpose()
}

pub(super) async fn outstanding(
    db: &LocalRuntime,
    provider: &str,
) -> Result<Option<CredentialRefillTaskRecord>, GatewayError> {
    let payload: Option<String> = sqlx::query_scalar(
        "SELECT payload FROM refill_tasks WHERE provider_id = ? AND terminal = 0 AND expires > ?",
    )
    .bind(provider)
    .bind(now())
    .fetch_optional(&db.pool)
    .await
    .map_err(storage_error)?;
    payload.as_deref().map(decode).transpose()
}

pub(super) async fn pending(db: &LocalRuntime, limit: usize) -> Result<Vec<String>, GatewayError> {
    sqlx::query_scalar(
        "SELECT id FROM refill_tasks WHERE terminal = 0 AND expires > ?
        AND lease_until <= ? ORDER BY created, id LIMIT ?",
    )
    .bind(now())
    .bind(now())
    .bind(limit as i64)
    .fetch_all(&db.pool)
    .await
    .map_err(storage_error)
}

pub(super) async fn list(
    db: &LocalRuntime,
    provider: Option<&str>,
    state: Option<CredentialRefillTaskState>,
    limit: usize,
) -> Result<Vec<CredentialRefillTaskView>, GatewayError> {
    let state = state.map(|state| serde_json::to_value(state).expect("enum serializes"));
    let payloads: Vec<String> = sqlx::query_scalar(
        "SELECT payload FROM refill_tasks WHERE expires > ?
        AND (? IS NULL OR provider_id = ?) AND (? IS NULL OR json_extract(payload, '$.state') = ?)
        ORDER BY created DESC, id DESC LIMIT ?",
    )
    .bind(now())
    .bind(provider)
    .bind(provider)
    .bind(state.as_ref().and_then(serde_json::Value::as_str))
    .bind(state.as_ref().and_then(serde_json::Value::as_str))
    .bind(limit as i64)
    .fetch_all(&db.pool)
    .await
    .map_err(storage_error)?;
    payloads
        .iter()
        .map(|payload| decode(payload).map(|task| (&task).into()))
        .collect()
}

pub(super) async fn claim(
    db: &LocalRuntime,
    expected: &str,
    task: &CredentialRefillTaskRecord,
    lease: u64,
    ttl: u64,
) -> Result<bool, GatewayError> {
    let Some(_guard) = db.try_refill_lock(&task.id).await? else {
        return Ok(false);
    };
    let result = sqlx::query(
        "UPDATE refill_tasks SET payload = ?, lease_token = ?, lease_until = ?, expires = ?
        WHERE id = ? AND payload = ? AND terminal = 0 AND expires > ? AND lease_until <= ?",
    )
    .bind(serialize_task(task)?)
    .bind(&task.claim_token)
    .bind(expiry(lease))
    .bind(expiry(ttl.max(lease.saturating_add(1))))
    .bind(&task.id)
    .bind(expected)
    .bind(now())
    .bind(now())
    .execute(&db.pool)
    .await
    .map_err(storage_error)?;
    Ok(result.rows_affected() == 1)
}

pub(super) async fn require(db: &LocalRuntime, id: &str, token: &str) -> Result<(), GatewayError> {
    let valid: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM refill_tasks WHERE id = ?
        AND terminal = 0 AND lease_token = ? AND lease_until > ? AND expires > ?",
    )
    .bind(id)
    .bind(token)
    .bind(now())
    .bind(now())
    .fetch_one(&db.pool)
    .await
    .map_err(storage_error)?;
    if valid == 1 {
        Ok(())
    } else {
        Err(claim_conflict())
    }
}

pub(super) async fn renew(
    db: &LocalRuntime,
    task: &CredentialRefillTaskRecord,
    token: &str,
    lease: u64,
    ttl: u64,
) -> Result<(), GatewayError> {
    let _guard = db
        .try_refill_lock(&task.id)
        .await?
        .ok_or_else(claim_conflict)?;
    let result = sqlx::query(
        "UPDATE refill_tasks SET payload = ?, lease_until = ?, expires = ?
        WHERE id = ? AND terminal = 0 AND lease_token = ? AND lease_until > ? AND expires > ?",
    )
    .bind(serialize_task(task)?)
    .bind(expiry(lease))
    .bind(expiry(ttl.max(lease.saturating_add(1))))
    .bind(&task.id)
    .bind(token)
    .bind(now())
    .bind(now())
    .execute(&db.pool)
    .await
    .map_err(storage_error)?;
    if result.rows_affected() == 1 {
        Ok(())
    } else {
        Err(claim_conflict())
    }
}

pub(super) async fn finish(
    db: &LocalRuntime,
    task: &CredentialRefillTaskRecord,
    token: &str,
    ttl: u64,
) -> Result<(), GatewayError> {
    let _guard = db
        .try_refill_lock(&task.id)
        .await?
        .ok_or_else(claim_conflict)?;
    let mut tx = lock_claim(db, &task.id, token).await?;
    finish_locked(&mut tx, task, ttl).await?;
    tx.commit().await.map_err(storage_error)
}

// Validate the lease under a short transaction. The OS task lock fences network
// delivery without blocking audit writes or other SQLite readers/writers.
pub(super) async fn lock_claim<'a>(
    db: &'a LocalRuntime,
    id: &str,
    token: &str,
) -> Result<Transaction<'a, Sqlite>, GatewayError> {
    let mut tx = db
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(storage_error)?;
    let valid: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM refill_tasks WHERE id = ?
        AND terminal = 0 AND lease_token = ? AND lease_until > ? AND expires > ?",
    )
    .bind(id)
    .bind(token)
    .bind(now())
    .bind(now())
    .fetch_one(&mut *tx)
    .await
    .map_err(storage_error)?;
    if valid != 1 {
        return Err(claim_conflict());
    }
    Ok(tx)
}

pub(super) async fn finish_locked(
    tx: &mut Transaction<'_, Sqlite>,
    task: &CredentialRefillTaskRecord,
    ttl: u64,
) -> Result<(), GatewayError> {
    sqlx::query(
        "UPDATE refill_tasks SET payload = ?, terminal = 1, lease_token = NULL, lease_until = 0,
        expires = ? WHERE id = ?",
    )
    .bind(serialize_task(task)?)
    .bind(expiry(ttl))
    .bind(&task.id)
    .execute(&mut **tx)
    .await
    .map_err(storage_error)?;
    sqlx::query("DELETE FROM refill_deliveries WHERE task_id = ?")
        .bind(&task.id)
        .execute(&mut **tx)
        .await
        .map_err(storage_error)?;
    Ok(())
}
