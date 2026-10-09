//! SQLite account ownership survives key rotation and in-flight key deletion.
use super::{storage_error, LocalRuntime};
use crate::access_balance::{policy, Change, Mutation};
use crate::db::{GatewayAccessKeyBalanceView, GatewayAccessKeyView};
use crate::error::GatewayError;
use sqlx::{Sqlite, Transaction};

pub(super) async fn initialize(tx: &mut Transaction<'_, Sqlite>) -> Result<(), GatewayError> {
    sqlx::raw_sql("CREATE TABLE IF NOT EXISTS local_balance_accounts (id TEXT PRIMARY KEY, payload TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS local_balance_keys (key_id TEXT PRIMARY KEY, account_id TEXT NOT NULL
            REFERENCES local_balance_accounts(id));
        CREATE INDEX IF NOT EXISTS local_balance_keys_account ON local_balance_keys(account_id);
        CREATE TABLE IF NOT EXISTS local_balance_inflight (account_id TEXT NOT NULL REFERENCES local_balance_accounts(id),
            owner TEXT NOT NULL, pending INTEGER NOT NULL CHECK(pending >= 0), PRIMARY KEY(account_id,owner));")
        .execute(&mut **tx).await.map_err(storage_error)?;
    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM local_access_keys WHERE id NOT IN (SELECT key_id FROM local_balance_keys)",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(storage_error)?;
    // Older local keys had no billing limit. Preserve that as an explicit policy record.
    for id in ids {
        create(tx, &id, None).await?;
    }
    Ok(())
}

pub(super) async fn create(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    rotated_from: Option<&str>,
) -> Result<(), GatewayError> {
    let account = if let Some(old) = rotated_from {
        sqlx::query_scalar::<_, String>(
            "SELECT account_id FROM local_balance_keys WHERE key_id = ?",
        )
        .bind(old)
        .fetch_one(&mut **tx)
        .await
        .map_err(storage_error)?
    } else {
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM local_balance_accounts")
            .fetch_one(&mut **tx)
            .await
            .map_err(storage_error)?;
        if count >= 10_000 {
            return Err(GatewayError::conflict(
                "Local balance account limit reached",
            ));
        }
        let payload = encode(&policy::unlimited(id))?;
        sqlx::query("INSERT INTO local_balance_accounts(id,payload) VALUES (?,?)")
            .bind(id)
            .bind(payload)
            .execute(&mut **tx)
            .await
            .map_err(storage_error)?;
        id.to_owned()
    };
    sqlx::query("INSERT INTO local_balance_keys(key_id,account_id) VALUES (?,?)")
        .bind(id)
        .bind(account)
        .execute(&mut **tx)
        .await
        .map_err(storage_error)?;
    if let Some(old) = rotated_from {
        crate::cash_billing::store::rotate(
            &mut crate::cash_billing::sql::CashConnection::Sqlite(tx),
            old,
            id,
        )
        .await?;
    }
    Ok(())
}

pub(super) async fn cleanup(tx: &mut Transaction<'_, Sqlite>) -> Result<(), GatewayError> {
    // Dead-instance reservations retain their debit, but no longer keep deleted keys alive.
    sqlx::query("DELETE FROM local_balance_inflight WHERE pending = 0 OR owner NOT IN (SELECT id FROM runtime_instances)")
        .execute(&mut **tx).await.map_err(storage_error)?;
    sqlx::query(
        "DELETE FROM local_balance_keys WHERE key_id NOT IN (SELECT id FROM local_access_keys)
        AND account_id NOT IN (SELECT account_id FROM local_balance_inflight)",
    )
    .execute(&mut **tx)
    .await
    .map_err(storage_error)?;
    sqlx::query("DELETE FROM local_balance_accounts WHERE id NOT IN (SELECT account_id FROM local_balance_keys)")
        .execute(&mut **tx).await.map_err(storage_error)?;
    Ok(())
}

fn encode(value: &GatewayAccessKeyBalanceView) -> Result<String, GatewayError> {
    serde_json::to_string(value)
        .map_err(|_| GatewayError::server_error("Cannot encode access balance"))
}

fn decode(id: &str, payload: &str) -> Result<GatewayAccessKeyBalanceView, GatewayError> {
    let mut balance: GatewayAccessKeyBalanceView = serde_json::from_str(payload)
        .map_err(|_| GatewayError::server_error("Invalid local access balance"))?;
    balance.access_key_id = id.into();
    Ok(balance)
}

async fn ensure_mode_change(
    tx: &mut Transaction<'_, Sqlite>,
    account: &str,
    old: &str,
    new: &str,
) -> Result<(), GatewayError> {
    if old != new {
        let pending: i64 = sqlx::query_scalar(
            "SELECT coalesce(sum(pending),0) FROM local_balance_inflight WHERE account_id = ?",
        )
        .bind(account)
        .fetch_one(&mut **tx)
        .await
        .map_err(storage_error)?;
        if pending > 0 {
            return Err(GatewayError::conflict(
                "Cannot change balance mode while requests are in flight",
            ));
        }
    }
    Ok(())
}

pub(super) async fn set_key_quota(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    quota: &crate::access_balance::quota::KeyQuotaInput,
) -> Result<(), GatewayError> {
    let (account, payload): (String, String) = sqlx::query_as("SELECT a.id,a.payload FROM local_balance_accounts a JOIN local_balance_keys k ON k.account_id = a.id WHERE k.key_id = ?")
        .bind(id).fetch_one(&mut **tx).await.map_err(storage_error)?;
    let current = decode(id, &payload)?;
    let mode = current.balance_mode.clone();
    let balance = quota.apply(id, Some(current))?;
    ensure_mode_change(tx, &account, &mode, &balance.balance_mode).await?;
    if quota.mode == "cash_prepaid" {
        crate::cash_billing::store::set_limit(
            &mut crate::cash_billing::sql::CashConnection::Sqlite(tx),
            id,
            quota.limit.expect("validated cash limit"),
        )
        .await?;
    }
    sqlx::query("UPDATE local_balance_accounts SET payload = ? WHERE id = ?")
        .bind(encode(&balance)?)
        .bind(account)
        .execute(&mut **tx)
        .await
        .map_err(storage_error)?;
    Ok(())
}

impl LocalRuntime {
    pub async fn access_balance(
        &self,
        id: &str,
    ) -> Result<Option<GatewayAccessKeyBalanceView>, GatewayError> {
        let payload: Option<String> = sqlx::query_scalar("SELECT a.payload FROM local_balance_accounts a
            JOIN local_balance_keys k ON k.account_id = a.id JOIN local_access_keys key ON key.id = k.key_id WHERE k.key_id = ?")
            .bind(id).fetch_optional(&self.pool).await.map_err(storage_error)?;
        payload.map(|payload| decode(id, &payload)).transpose()
    }

    pub(super) async fn access_balances(
        &self,
    ) -> Result<Vec<GatewayAccessKeyBalanceView>, GatewayError> {
        let rows: Vec<(String, String)> = sqlx::query_as("SELECT k.key_id, a.payload FROM local_balance_accounts a
            JOIN local_balance_keys k ON k.account_id = a.id JOIN local_access_keys key ON key.id = k.key_id ORDER BY k.key_id LIMIT 10000")
            .fetch_all(&self.pool).await.map_err(storage_error)?;
        rows.into_iter()
            .map(|(id, payload)| decode(&id, &payload))
            .collect()
    }

    pub(crate) async fn mutate_access_balance<T, F>(
        &self,
        id: &str,
        operation: Mutation,
        change: F,
    ) -> Result<T, GatewayError>
    where
        F: FnOnce(Option<GatewayAccessKeyBalanceView>) -> Result<Change<T>, GatewayError>,
    {
        let mut tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let row: Option<(String, String, Option<String>)> = sqlx::query_as(
            "SELECT a.id, a.payload, key.payload
            FROM local_balance_accounts a JOIN local_balance_keys k ON k.account_id = a.id
            LEFT JOIN local_access_keys key ON key.id = k.key_id WHERE k.key_id = ?",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage_error)?;
        let (account, payload, key) =
            row.ok_or_else(|| GatewayError::not_found("Access key not found"))?;
        if operation != Mutation::Finalize {
            let key: GatewayAccessKeyView = serde_json::from_str(
                &key.ok_or_else(|| GatewayError::not_found("Access key not found"))?,
            )
            .map_err(|_| GatewayError::server_error("Invalid local access key"))?;
            if operation == Mutation::Reserve {
                super::access_keys::policy::ensure_active(&key)?;
            }
        }
        let current = decode(id, &payload)?;
        let mode = current.balance_mode.clone();
        let (balance, result, pending_delta) = change(Some(current))?;
        ensure_mode_change(&mut tx, &account, &mode, &balance.balance_mode).await?;
        if pending_delta != 0 {
            sqlx::query(
                "INSERT INTO local_balance_inflight(account_id,owner,pending) VALUES (?,?,max(0,?))
                ON CONFLICT(account_id,owner) DO UPDATE SET pending = max(0,pending + ?)",
            )
            .bind(&account)
            .bind(&self.owner)
            .bind(pending_delta)
            .bind(pending_delta)
            .execute(&mut *tx)
            .await
            .map_err(storage_error)?;
        }
        sqlx::query("UPDATE local_balance_accounts SET payload = ? WHERE id = ?")
            .bind(encode(&balance)?)
            .bind(account)
            .execute(&mut *tx)
            .await
            .map_err(storage_error)?;
        if operation == Mutation::Finalize {
            cleanup(&mut tx).await?;
        }
        tx.commit().await.map_err(storage_error)?;
        Ok(result)
    }
}
