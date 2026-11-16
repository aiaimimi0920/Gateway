//! Keep the original DB permission locked across the one HTTP attempt, not merely until reserve.
use super::{journal::load_record, snapshot::Snapshot, source::load_locked};
use crate::{
    console::refresh_grants::{GrantPhase, GrantRecord},
    db::map_db_error,
    error::GatewayError,
};
use sqlx::{PgPool, Postgres, Transaction};

pub(crate) async fn authorize_send<'a>(
    pool: &'a PgPool,
    expected: &Snapshot,
    sending: &GrantRecord,
) -> Result<Option<Transaction<'a, Postgres>>, GatewayError> {
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let Some(actual) = load_locked(&mut tx, &expected.request.credential).await? else {
        return Ok(None);
    };
    if actual.request != expected.request || !sending.matches_request(&actual.request) {
        return Ok(None);
    }
    let head: Option<String> = sqlx::query_scalar(
        "select material from gateway_provider_refresh_heads where family=$1 for update",
    )
    .bind(sending.family_id())
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?;
    if head.as_deref() != Some(sending.material_id().as_str()) {
        return Ok(None);
    }
    let Some((current, _)) =
        load_record(&mut tx, &sending.family_id(), &sending.material_id()).await?
    else {
        return Ok(None);
    };
    if current.phase() != GrantPhase::Sending || !current.same_attempt(sending) {
        return Ok(None);
    }
    // The caller retains this read-only transaction through the bounded HTTP response.
    // SQLx owns rollback on Drop; no statement extends or clears permanent Sending.
    Ok(Some(tx))
}
