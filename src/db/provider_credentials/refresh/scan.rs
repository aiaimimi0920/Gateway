//! Keyset admission is bounded to one ID and cannot starve later credentials behind fresh rows.
use crate::{db::map_db_error, error::GatewayError};
use sqlx::PgPool;

pub(crate) async fn next_active(
    pool: &PgPool,
    after: Option<&str>,
) -> Result<Option<String>, GatewayError> {
    sqlx::query_scalar("select c.id from gateway_provider_credentials c join gateway_provider_accounts a on a.id=c.provider_account_id where c.status='active' and c.archived_at is null and a.status='active' and a.adapter='accio_compatible' and ($1::text is null or c.id>$1) order by c.id limit 1")
        .bind(after).fetch_optional(pool).await.map_err(map_db_error)
}
