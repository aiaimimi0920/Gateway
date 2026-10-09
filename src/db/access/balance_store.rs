//! PostgreSQL is the authoritative ledger; Redis projections never authorize spending.
use super::*;
use crate::access_balance::{Change, Mutation};

pub(crate) async fn load(
    pool: &PgPool,
    id: &str,
) -> Result<Option<GatewayAccessKeyBalanceView>, GatewayError> {
    sqlx::query_as::<_, AccessKeyBalanceRow>(
        "SELECT * FROM gateway_access_key_balances WHERE access_key_id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)
    .map(|row| row.map(views::to_access_key_balance_view))
}

pub(crate) async fn mutate<T, F>(
    pool: &PgPool,
    id: &str,
    operation: Mutation,
    change: F,
) -> Result<T, GatewayError>
where
    F: FnOnce(Option<GatewayAccessKeyBalanceView>) -> Result<Change<T>, GatewayError>,
{
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    // Lock the parent as well, serializing initialization with rotation and deletion.
    let key: Option<(String, Option<OffsetDateTime>)> = sqlx::query_as(
        "SELECT status, expires_at FROM gateway_access_keys WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?;
    let (status, expiry) = key.ok_or_else(|| GatewayError::not_found("Access key not found"))?;
    if operation == Mutation::Reserve
        && (status != "active" || expiry.is_some_and(|expiry| expiry <= now_utc()))
    {
        return Err(GatewayError::unauthorized(
            "Access key is inactive or expired",
        ));
    }
    let current = sqlx::query_as::<_, AccessKeyBalanceRow>(
        "SELECT * FROM gateway_access_key_balances WHERE access_key_id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?;
    let (balance, result, _) = change(current.map(views::to_access_key_balance_view))?;
    write(&mut tx, id, &balance).await?;
    tx.commit().await.map_err(map_db_error)?;
    Ok(result)
}

pub(super) async fn set_key_quota(
    tx: &mut Transaction<'_, Postgres>,
    id: &str,
    quota: &crate::access_balance::quota::KeyQuotaInput,
) -> Result<(), GatewayError> {
    let current = sqlx::query_as::<_, AccessKeyBalanceRow>(
        "SELECT * FROM gateway_access_key_balances WHERE access_key_id=$1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(map_db_error)?;
    let current = current.map(views::to_access_key_balance_view);
    quota.validate_server_mode(current.as_ref())?;
    let balance = quota.apply(id, current)?;
    if quota.mode == "cash_prepaid" {
        crate::cash_billing::store::set_limit(
            &mut crate::cash_billing::sql::CashConnection::Postgres(tx),
            id,
            quota.limit.expect("validated cash limit"),
        )
        .await?;
    }
    write(tx, id, &balance).await
}

async fn write(
    tx: &mut Transaction<'_, Postgres>,
    id: &str,
    balance: &GatewayAccessKeyBalanceView,
) -> Result<(), GatewayError> {
    sqlx::query("INSERT INTO gateway_access_key_balances (
        access_key_id, balance_mode, status, unlimited_until, period_starts_at, period_ends_at,
        total_tokens, remaining_tokens, total_messages, remaining_messages, updated_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
        ON CONFLICT (access_key_id) DO UPDATE SET
        balance_mode=excluded.balance_mode, status=excluded.status, unlimited_until=excluded.unlimited_until,
        period_starts_at=excluded.period_starts_at, period_ends_at=excluded.period_ends_at,
        total_tokens=excluded.total_tokens, remaining_tokens=excluded.remaining_tokens,
        total_messages=excluded.total_messages, remaining_messages=excluded.remaining_messages,
        updated_at=excluded.updated_at")
        .bind(id).bind(&balance.balance_mode).bind(&balance.status)
        .bind(parse_optional_timestamp(balance.unlimited_until.as_deref()))
        .bind(parse_optional_timestamp(balance.period_starts_at.as_deref()))
        .bind(parse_optional_timestamp(balance.period_ends_at.as_deref()))
        .bind(balance.total_tokens).bind(balance.remaining_tokens)
        .bind(balance.total_messages).bind(balance.remaining_messages)
        .bind(parse_optional_timestamp(Some(&balance.updated_at)).unwrap_or_else(now_utc))
        .execute(&mut **tx).await.map_err(map_db_error)?;
    Ok(())
}
