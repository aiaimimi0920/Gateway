use super::*;

pub async fn adjust_access_key_balance(
    pool: &PgPool,
    _redis: &RedisPool,
    id: &str,
    input: AccessKeyBalanceAdjustInput,
) -> Result<GatewayAccessKeyBalanceView, GatewayError> {
    crate::access_balance::AccessBalanceStore::Postgres(pool)
        .adjust(id, input)
        .await
}
