use super::*;
use crate::redis::keys;
use redis::AsyncCommands;

fn balance_runtime_key(access_key_id: &str) -> String {
    keys::access_balance_key(access_key_id)
}

pub(super) fn map_balance_row_to_cache(row: &AccessKeyBalanceRow) -> CachedAccessKeyBalance {
    CachedAccessKeyBalance {
        access_key_id: row.access_key_id.clone(),
        balance_mode: row.balance_mode.clone(),
        status: row.status.clone(),
        unlimited_until: row.unlimited_until.map(format_timestamp),
        period_starts_at: row.period_starts_at.map(format_timestamp),
        period_ends_at: row.period_ends_at.map(format_timestamp),
        total_tokens: row.total_tokens,
        remaining_tokens: row.remaining_tokens,
        total_messages: row.total_messages,
        remaining_messages: row.remaining_messages,
        updated_at: format_timestamp(row.updated_at),
    }
}

fn project_balance_decision(
    balance: &CachedAccessKeyBalance,
    estimated_tokens: u64,
) -> AccessBalanceDecision {
    let now = now_utc();
    if balance.status != "active" {
        return AccessBalanceDecision {
            allowed: false,
            balance_mode: Some(balance.balance_mode.clone()),
            pre_deduct_amount: 0,
            remaining_tokens: balance.remaining_tokens,
            remaining_messages: balance.remaining_messages,
            reason: Some("balance_inactive".to_string()),
        };
    }

    let within_period = |starts_at: Option<&str>, ends_at: Option<&str>| -> bool {
        let starts_at = parse_optional_timestamp(starts_at);
        let ends_at = parse_optional_timestamp(ends_at);
        if starts_at.is_some_and(|value| now < value) {
            return false;
        }
        if ends_at.is_some_and(|value| now > value) {
            return false;
        }
        true
    };

    let normalized_mode = normalize_balance_mode(&balance.balance_mode);
    match normalized_mode.as_str() {
        "time_pass" => {
            let unlimited_until = parse_optional_timestamp(balance.unlimited_until.as_deref());
            let allowed = unlimited_until.is_some_and(|value| value > now)
                && within_period(
                    balance.period_starts_at.as_deref(),
                    balance.period_ends_at.as_deref(),
                );
            AccessBalanceDecision {
                allowed,
                balance_mode: Some(normalized_mode.clone()),
                pre_deduct_amount: 0,
                remaining_tokens: balance.remaining_tokens,
                remaining_messages: balance.remaining_messages,
                reason: if allowed {
                    None
                } else {
                    Some("time_pass_inactive".to_string())
                },
            }
        }
        "message_prepaid" => {
            let remaining = balance.remaining_messages.unwrap_or(0);
            let allowed = remaining > 0
                && within_period(
                    balance.period_starts_at.as_deref(),
                    balance.period_ends_at.as_deref(),
                );
            AccessBalanceDecision {
                allowed,
                balance_mode: Some(normalized_mode.clone()),
                pre_deduct_amount: if allowed { 1 } else { 0 },
                remaining_tokens: balance.remaining_tokens,
                remaining_messages: Some(remaining),
                reason: if allowed {
                    None
                } else {
                    Some("message_balance_exhausted".to_string())
                },
            }
        }
        _ => {
            let remaining = balance.remaining_tokens.unwrap_or(0);
            let estimate = estimated_tokens.max(1) as i64;
            let allowed = remaining >= estimate
                && within_period(
                    balance.period_starts_at.as_deref(),
                    balance.period_ends_at.as_deref(),
                );
            AccessBalanceDecision {
                allowed,
                balance_mode: Some("token_prepaid".to_string()),
                pre_deduct_amount: if allowed { estimated_tokens.max(1) } else { 0 },
                remaining_tokens: Some(remaining),
                remaining_messages: balance.remaining_messages,
                reason: if allowed {
                    None
                } else {
                    Some("token_balance_exhausted".to_string())
                },
            }
        }
    }
}

pub(super) async fn cache_balance(
    redis_pool: &RedisPool,
    balance: &CachedAccessKeyBalance,
) -> Result<(), GatewayError> {
    let payload = serde_json::to_string(balance)
        .map_err(|error| GatewayError::server_error(format!("serialize balance cache: {error}")))?;
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let _: () = conn
        .set(balance_runtime_key(&balance.access_key_id), payload)
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("write access balance cache: {error}"))
        })?;
    Ok(())
}

async fn load_cached_balance(
    redis_pool: &RedisPool,
    access_key_id: &str,
) -> Result<Option<CachedAccessKeyBalance>, GatewayError> {
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let raw: Option<String> =
        conn.get(balance_runtime_key(access_key_id))
            .await
            .map_err(|error| {
                GatewayError::server_error(format!("read access balance cache: {error}"))
            })?;
    raw.map(|value| {
        serde_json::from_str::<CachedAccessKeyBalance>(&value).map_err(|error| {
            GatewayError::server_error(format!("deserialize access balance cache: {error}"))
        })
    })
    .transpose()
}

async fn load_balance_from_db(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: &str,
) -> Result<Option<CachedAccessKeyBalance>, GatewayError> {
    let row = sqlx::query_as::<_, AccessKeyBalanceRow>(
        r#"
        select
          access_key_id,
          balance_mode,
          status,
          unlimited_until,
          period_starts_at,
          period_ends_at,
          total_tokens,
          remaining_tokens,
          total_messages,
          remaining_messages,
          updated_at
        from gateway_access_key_balances
        where access_key_id = $1
        limit 1
        "#,
    )
    .bind(access_key_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let cached = map_balance_row_to_cache(&row);
    cache_balance(redis_pool, &cached).await?;
    Ok(Some(cached))
}

pub async fn get_access_key_balance(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: &str,
) -> Result<Option<GatewayAccessKeyBalanceView>, GatewayError> {
    let cached = match load_cached_balance(redis_pool, access_key_id).await? {
        Some(value) => value,
        None => match load_balance_from_db(pool, redis_pool, access_key_id).await? {
            Some(value) => value,
            None => return Ok(None),
        },
    };
    Ok(Some(GatewayAccessKeyBalanceView {
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
    }))
}

pub async fn evaluate_access_key_balance(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: &str,
    estimated_tokens: u64,
) -> Result<AccessBalanceDecision, GatewayError> {
    let balance = match load_cached_balance(redis_pool, access_key_id).await? {
        Some(value) => value,
        None => load_balance_from_db(pool, redis_pool, access_key_id)
            .await?
            .ok_or_else(|| {
                GatewayError::quota_exceeded("当前 key 尚未初始化额度")
                    .with_code("balance_not_initialized")
            })?,
    };
    Ok(project_balance_decision(&balance, estimated_tokens))
}

pub(super) fn apply_balance_delta(
    balance: &mut CachedAccessKeyBalance,
    token_delta: i64,
    message_delta: i64,
) {
    if let Some(value) = balance.remaining_tokens.as_mut() {
        *value += token_delta;
    }
    if let Some(value) = balance.total_tokens.as_mut() {
        if token_delta > 0 {
            *value += token_delta;
        }
    }
    if let Some(value) = balance.remaining_messages.as_mut() {
        *value += message_delta;
    }
    if let Some(value) = balance.total_messages.as_mut() {
        if message_delta > 0 {
            *value += message_delta;
        }
    }
    balance.updated_at = format_timestamp(now_utc());
}

async fn persist_cached_balance(
    pool: &PgPool,
    redis_pool: &RedisPool,
    balance: &CachedAccessKeyBalance,
) -> Result<(), GatewayError> {
    sqlx::query(
        r#"
        update gateway_access_key_balances
        set
          balance_mode = $2,
          status = $3,
          unlimited_until = $4,
          period_starts_at = $5,
          period_ends_at = $6,
          total_tokens = $7,
          remaining_tokens = $8,
          total_messages = $9,
          remaining_messages = $10,
          updated_at = $11
        where access_key_id = $1
        "#,
    )
    .bind(&balance.access_key_id)
    .bind(&balance.balance_mode)
    .bind(&balance.status)
    .bind(parse_optional_timestamp(balance.unlimited_until.as_deref()))
    .bind(parse_optional_timestamp(
        balance.period_starts_at.as_deref(),
    ))
    .bind(parse_optional_timestamp(balance.period_ends_at.as_deref()))
    .bind(balance.total_tokens)
    .bind(balance.remaining_tokens)
    .bind(balance.total_messages)
    .bind(balance.remaining_messages)
    .bind(now_utc())
    .execute(pool)
    .await
    .map_err(map_db_error)?;
    cache_balance(redis_pool, balance).await
}

pub async fn pre_deduct_access_key_balance(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: &str,
    estimated_tokens: u64,
) -> Result<AccessBalanceDecision, GatewayError> {
    let mut balance = match load_cached_balance(redis_pool, access_key_id).await? {
        Some(value) => value,
        None => load_balance_from_db(pool, redis_pool, access_key_id)
            .await?
            .ok_or_else(|| {
                GatewayError::quota_exceeded("当前 key 尚未初始化额度")
                    .with_code("balance_not_initialized")
            })?,
    };
    let decision = project_balance_decision(&balance, estimated_tokens);
    if !decision.allowed {
        return Ok(decision);
    }
    match decision.balance_mode.as_deref() {
        Some("message_prepaid") => {
            apply_balance_delta(&mut balance, 0, -(decision.pre_deduct_amount as i64));
            persist_cached_balance(pool, redis_pool, &balance).await?;
        }
        Some("token_prepaid") => {
            apply_balance_delta(&mut balance, -(decision.pre_deduct_amount as i64), 0);
            persist_cached_balance(pool, redis_pool, &balance).await?;
        }
        _ => {}
    }
    Ok(decision)
}

pub async fn refund_access_key_balance(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: &str,
    pre_deduct_amount: u64,
) -> Result<(), GatewayError> {
    if pre_deduct_amount == 0 {
        return Ok(());
    }
    let Some(mut balance) = load_balance_from_db(pool, redis_pool, access_key_id).await? else {
        return Ok(());
    };
    match normalize_balance_mode(&balance.balance_mode).as_str() {
        "message_prepaid" => apply_balance_delta(&mut balance, 0, pre_deduct_amount as i64),
        "token_prepaid" => apply_balance_delta(&mut balance, pre_deduct_amount as i64, 0),
        _ => {}
    }
    persist_cached_balance(pool, redis_pool, &balance).await
}

pub async fn settle_access_key_balance(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: &str,
    pre_deduct_amount: u64,
    actual_total_tokens: u64,
) -> Result<(), GatewayError> {
    let Some(mut balance) = load_balance_from_db(pool, redis_pool, access_key_id).await? else {
        return Ok(());
    };
    if normalize_balance_mode(&balance.balance_mode) != "token_prepaid" {
        return Ok(());
    }
    if actual_total_tokens > pre_deduct_amount {
        apply_balance_delta(
            &mut balance,
            -((actual_total_tokens - pre_deduct_amount) as i64),
            0,
        );
    } else if pre_deduct_amount > actual_total_tokens {
        apply_balance_delta(
            &mut balance,
            (pre_deduct_amount - actual_total_tokens) as i64,
            0,
        );
    }
    persist_cached_balance(pool, redis_pool, &balance).await
}
