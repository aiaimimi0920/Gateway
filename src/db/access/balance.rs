//! Public server API; balance policy and transaction ownership live in the shared store.
use super::*;
use crate::access_balance::AccessBalanceStore;

pub async fn get_access_key_balance(
    pool: &PgPool,
    _redis: &RedisPool,
    id: &str,
) -> Result<Option<GatewayAccessKeyBalanceView>, GatewayError> {
    AccessBalanceStore::Postgres(pool).get(id).await
}

pub async fn evaluate_access_key_balance(
    pool: &PgPool,
    _redis: &RedisPool,
    id: &str,
    estimated: u64,
) -> Result<AccessBalanceDecision, GatewayError> {
    AccessBalanceStore::Postgres(pool)
        .evaluate(id, estimated)
        .await
}

pub async fn pre_deduct_access_key_balance(
    pool: &PgPool,
    _redis: &RedisPool,
    id: &str,
    estimated: u64,
) -> Result<AccessBalanceDecision, GatewayError> {
    AccessBalanceStore::Postgres(pool)
        .reserve(id, estimated)
        .await
}

pub async fn refund_access_key_balance(
    pool: &PgPool,
    _redis: &RedisPool,
    id: &str,
    reserved: u64,
) -> Result<(), GatewayError> {
    AccessBalanceStore::Postgres(pool)
        .refund(id, reserved)
        .await
}

pub async fn settle_access_key_balance(
    pool: &PgPool,
    _redis: &RedisPool,
    id: &str,
    reserved: u64,
    actual: u64,
) -> Result<(), GatewayError> {
    AccessBalanceStore::Postgres(pool)
        .settle(id, reserved, actual)
        .await
}
