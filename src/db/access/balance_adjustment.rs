use super::balance::{apply_balance_delta, cache_balance, map_balance_row_to_cache};
use super::*;

pub async fn adjust_access_key_balance(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: &str,
    input: AccessKeyBalanceAdjustInput,
) -> Result<GatewayAccessKeyBalanceView, GatewayError> {
    let now = now_utc();
    let current = sqlx::query_as::<_, AccessKeyBalanceRow>(
        r#"
        select
          access_key_id, balance_mode, status, unlimited_until, period_starts_at, period_ends_at,
          total_tokens, remaining_tokens, total_messages, remaining_messages, updated_at
        from gateway_access_key_balances
        where access_key_id = $1
        limit 1
        "#,
    )
    .bind(access_key_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;
    let mut cached =
        current
            .as_ref()
            .map(map_balance_row_to_cache)
            .unwrap_or(CachedAccessKeyBalance {
                access_key_id: access_key_id.to_string(),
                balance_mode: input
                    .balance_mode
                    .clone()
                    .map(|value| normalize_balance_mode(&value))
                    .unwrap_or_else(|| "token_prepaid".to_string()),
                status: input.status.clone().unwrap_or_else(|| "active".to_string()),
                unlimited_until: input.unlimited_until.clone(),
                period_starts_at: input.period_starts_at.clone(),
                period_ends_at: input.period_ends_at.clone(),
                total_tokens: input.total_tokens,
                remaining_tokens: input.remaining_tokens.or(input.total_tokens),
                total_messages: input.total_messages,
                remaining_messages: input.remaining_messages.or(input.total_messages),
                updated_at: format_timestamp(now),
            });

    if let Some(value) = input.balance_mode {
        cached.balance_mode = normalize_balance_mode(&value);
    }
    if let Some(value) = input.status {
        cached.status = value;
    }
    if input.unlimited_until.is_some() {
        cached.unlimited_until = input.unlimited_until;
    }
    if input.period_starts_at.is_some() {
        cached.period_starts_at = input.period_starts_at;
    }
    if input.period_ends_at.is_some() {
        cached.period_ends_at = input.period_ends_at;
    }
    if let Some(value) = input.total_tokens {
        cached.total_tokens = Some(value);
        if current.is_none() && input.remaining_tokens.is_none() {
            cached.remaining_tokens = Some(value);
        }
    }
    if let Some(value) = input.remaining_tokens {
        cached.remaining_tokens = Some(value);
    }
    if let Some(value) = input.total_messages {
        cached.total_messages = Some(value);
        if current.is_none() && input.remaining_messages.is_none() {
            cached.remaining_messages = Some(value);
        }
    }
    if let Some(value) = input.remaining_messages {
        cached.remaining_messages = Some(value);
    }
    apply_balance_delta(
        &mut cached,
        input.token_delta.unwrap_or_default(),
        input.message_delta.unwrap_or_default(),
    );

    sqlx::query(
        r#"
        insert into gateway_access_key_balances (
          access_key_id, balance_mode, status, unlimited_until, period_starts_at, period_ends_at,
          total_tokens, remaining_tokens, total_messages, remaining_messages, updated_at
        ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        on conflict (access_key_id) do update
        set
          balance_mode = excluded.balance_mode,
          status = excluded.status,
          unlimited_until = excluded.unlimited_until,
          period_starts_at = excluded.period_starts_at,
          period_ends_at = excluded.period_ends_at,
          total_tokens = excluded.total_tokens,
          remaining_tokens = excluded.remaining_tokens,
          total_messages = excluded.total_messages,
          remaining_messages = excluded.remaining_messages,
          updated_at = excluded.updated_at
        "#,
    )
    .bind(access_key_id)
    .bind(&cached.balance_mode)
    .bind(&cached.status)
    .bind(parse_optional_timestamp(cached.unlimited_until.as_deref()))
    .bind(parse_optional_timestamp(cached.period_starts_at.as_deref()))
    .bind(parse_optional_timestamp(cached.period_ends_at.as_deref()))
    .bind(cached.total_tokens)
    .bind(cached.remaining_tokens)
    .bind(cached.total_messages)
    .bind(cached.remaining_messages)
    .bind(now)
    .execute(pool)
    .await
    .map_err(map_db_error)?;
    cache_balance(redis_pool, &cached).await?;
    Ok(GatewayAccessKeyBalanceView {
        access_key_id: cached.access_key_id,
        balance_mode: cached.balance_mode,
        status: cached.status,
        unlimited_until: cached.unlimited_until,
        period_starts_at: cached.period_starts_at,
        period_ends_at: cached.period_ends_at,
        total_tokens: cached.total_tokens,
        remaining_tokens: cached.remaining_tokens,
        total_messages: cached.total_messages,
        remaining_messages: cached.remaining_messages,
        updated_at: cached.updated_at,
    })
}
