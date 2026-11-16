//! Permanent PostgreSQL family heads fence uncertain sends, including duplicate credentials.
use super::{
    snapshot::{invalid, Snapshot},
    source::load_locked,
};
use crate::{
    console::refresh_grants::{GrantRecord, GrantReservation, GrantResult},
    db::map_db_error,
    error::GatewayError,
};
use sqlx::{PgPool, Postgres, Transaction};

pub(crate) async fn read_snapshot(
    pool: &PgPool,
    credential: &str,
) -> Result<Option<Snapshot>, GatewayError> {
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let snapshot = load_locked(&mut tx, credential).await?;
    tx.commit().await.map_err(map_db_error)?;
    Ok(snapshot)
}

pub(crate) async fn reserve(
    pool: &PgPool,
    expected: &Snapshot,
) -> Result<GrantReservation, GatewayError> {
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let Some(actual) = load_locked(&mut tx, &expected.request.credential).await? else {
        return Ok(GrantReservation::Stale);
    };
    if actual.request != expected.request {
        return Ok(GrantReservation::Stale);
    }
    let family = expected.request.family_id();
    let material = expected.request.material_id();
    let inserted = sqlx::query("insert into gateway_provider_refresh_heads(family,material) values($1,$2) on conflict do nothing")
        .bind(&family).bind(&material).execute(&mut *tx).await.map_err(map_db_error)?.rows_affected() == 1;
    let head: String = sqlx::query_scalar(
        "select material from gateway_provider_refresh_heads where family=$1 for update",
    )
    .bind(&family)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_db_error)?;
    if let Some((record, _)) = load_record(&mut tx, &family, &material).await? {
        let reservation = if head != material || !record.matches_request(&expected.request) {
            GrantReservation::Blocked
        } else {
            GrantReservation::Existing(record)
        };
        tx.commit().await.map_err(map_db_error)?;
        return Ok(reservation);
    }
    if !inserted {
        let (prior, _) = load_record(&mut tx, &family, &head)
            .await?
            .ok_or_else(invalid)?;
        if !expected.request.can_follow(&prior) {
            return Ok(GrantReservation::Blocked);
        }
    }
    let record = GrantRecord::sending(&expected.request);
    let raw = record.to_json().map_err(|_| invalid())?;
    sqlx::query(
        "insert into gateway_provider_refresh_grants(family,material,record) values($1,$2,$3)",
    )
    .bind(&family)
    .bind(&material)
    .bind(raw)
    .execute(&mut *tx)
    .await
    .map_err(map_db_error)?;
    sqlx::query("update gateway_provider_refresh_heads set material=$2 where family=$1")
        .bind(&family)
        .bind(&material)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
    // Only an acknowledged commit permits HTTP. A lost acknowledgement never permits retry.
    tx.commit().await.map_err(map_db_error)?;
    Ok(GrantReservation::Reserved(record))
}

/// Read consumption/result evidence without reserving a currently fresh credential.
pub(crate) async fn load_current(
    pool: &PgPool,
    snapshot: &Snapshot,
) -> Result<Option<GrantRecord>, GatewayError> {
    let family = snapshot.request.family_id();
    let material = snapshot.request.material_id();
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let head: Option<String> = sqlx::query_scalar(
        "select material from gateway_provider_refresh_heads where family=$1 for update",
    )
    .bind(&family)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?;
    if head.as_deref() != Some(material.as_str()) {
        return Ok(None);
    }
    let record = load_record(&mut tx, &family, &material)
        .await?
        .map(|(record, _)| record);
    tx.commit().await.map_err(map_db_error)?;
    Ok(record)
}

pub(crate) async fn store_result(
    pool: &PgPool,
    original: &GrantRecord,
    result: &GrantResult,
) -> Result<GrantRecord, GatewayError> {
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let (current, _) = load_record(&mut tx, &original.family_id(), &original.material_id())
        .await?
        .ok_or_else(invalid)?;
    if !current.same_attempt(original) {
        return Err(invalid());
    }
    // Save accepted material even after disable/delete; publication checks authority separately.
    let next = current.with_result(result).map_err(|_| invalid())?;
    let changed = sqlx::query("update gateway_provider_refresh_grants set record=$3 where family=$1 and material=$2 and record=$4")
        .bind(original.family_id()).bind(original.material_id()).bind(next.to_json().map_err(|_| invalid())?)
        .bind(current.to_json().map_err(|_| invalid())?).execute(&mut *tx).await.map_err(map_db_error)?.rows_affected();
    if changed != 1 {
        return Err(invalid());
    }
    tx.commit().await.map_err(map_db_error)?;
    Ok(next)
}

pub(super) async fn load_record(
    tx: &mut Transaction<'_, Postgres>,
    family: &str,
    material: &str,
) -> Result<Option<(GrantRecord, Option<String>)>, GatewayError> {
    let row: Option<(String, Option<String>)> = sqlx::query_as("select record,published_authority from gateway_provider_refresh_grants where family=$1 and material=$2 for update")
        .bind(family).bind(material).fetch_optional(&mut **tx).await.map_err(map_db_error)?;
    row.map(|(raw, authority)| {
        let record = GrantRecord::from_json(&raw).map_err(|_| invalid())?;
        if record.family_id() != family
            || record.material_id() != material
            || (record.phase() == crate::console::refresh_grants::GrantPhase::Published)
                != authority.is_some()
            || authority
                .as_ref()
                .is_some_and(|value| !crate::console::refresh_grants::valid_digest(value))
        {
            return Err(invalid());
        }
        Ok((record, authority))
    })
    .transpose()
}
