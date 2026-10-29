//! Operator reset and version-fenced egress undo. Never change credential enable flags or quota.
use super::*;
use time::format_description::well_known::Rfc3339;

#[derive(Debug, Clone)]
pub(crate) struct HealthRow {
    pub row: GatewayProviderCredentialModelStateView,
    pub version: String,
}

pub(crate) fn clear(row: &mut GatewayProviderCredentialModelStateView) {
    row.status = "active".into();
    row.failure_class = None;
    row.failure_scope = None;
    row.failure_count = 0;
    row.last_error = None;
    row.last_upstream_status = None;
    row.cooldown_until = None;
}

pub(crate) async fn record_window_failure(
    pool: &PgPool,
    input: RecordCredentialModelFailureInput,
    before: Option<&HealthRow>,
) -> Result<Option<HealthRow>, GatewayError> {
    if before.is_some_and(|row| row.row.status == "blocked") {
        return Ok(None);
    }
    let id = provider_credential_model_state_id(
        &input.provider_account_id,
        input.provider_credential_id.as_deref(),
        input.provider_credential_ref.as_deref(),
        input.protocol_profile.as_deref(),
        &input.model,
    );
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let version: Option<String> = sqlx::query_scalar("SELECT xmin::text || ':' || ctid::text FROM gateway_provider_credential_model_states WHERE id=$1 FOR UPDATE")
        .bind(&id).fetch_optional(&mut *tx).await.map_err(map_db_error)?;
    if version.as_deref() != before.map(|row| row.version.as_str()) {
        return Ok(None);
    }
    let Some(row) = super::write_failure(&mut *tx, input, version.is_some()).await? else {
        return Ok(None);
    };
    let version: String = sqlx::query_scalar("SELECT xmin::text || ':' || ctid::text FROM gateway_provider_credential_model_states WHERE id=$1")
        .bind(&id).fetch_one(&mut *tx).await.map_err(map_db_error)?;
    tx.commit().await.map_err(map_db_error)?;
    Ok(Some(HealthRow { row, version }))
}

pub(crate) async fn read(pool: &PgPool, id: &str) -> Result<Option<HealthRow>, GatewayError> {
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let row = sqlx::query_as::<_, GatewayProviderCredentialModelStateRow>(
        "SELECT * FROM gateway_provider_credential_model_states WHERE id = $1 FOR SHARE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?;
    let version: Option<String> = sqlx::query_scalar(
        "SELECT xmin::text || ':' || ctid::text FROM gateway_provider_credential_model_states WHERE id=$1",
    ).bind(id).fetch_optional(&mut *tx).await.map_err(map_db_error)?;
    tx.commit().await.map_err(map_db_error)?;
    Ok(row.zip(version).map(|(row, version)| HealthRow {
        row: to_view(row),
        version,
    }))
}

pub(crate) async fn restore(
    pool: &PgPool,
    expected: &HealthRow,
    before: Option<&HealthRow>,
) -> Result<(), GatewayError> {
    let mut row = before
        .map(|r| r.row.clone())
        .unwrap_or_else(|| expected.row.clone());
    if before.is_none() {
        clear(&mut row);
    }
    let cooldown = row
        .cooldown_until
        .as_deref()
        .map(|s| OffsetDateTime::parse(s, &Rfc3339))
        .transpose()
        .map_err(|_| GatewayError::server_error("Invalid health checkpoint timestamp"))?;
    sqlx::query("UPDATE gateway_provider_credential_model_states SET status=$2, failure_class=$3,
        failure_scope=$4, failure_count=$5, last_error=$6, last_upstream_status=$7, cooldown_until=$8,
        updated_at=GREATEST(clock_timestamp(),updated_at+interval '1 microsecond')
        WHERE id=$1 AND xmin::text || ':' || ctid::text=$9")
        .bind(&row.id).bind(row.status).bind(row.failure_class).bind(row.failure_scope)
        .bind(row.failure_count).bind(row.last_error).bind(row.last_upstream_status).bind(cooldown)
        .bind(&expected.version).execute(pool).await.map_err(map_db_error)?;
    Ok(())
}

pub(crate) async fn reset(pool: &PgPool, provider: &str) -> Result<(), GatewayError> {
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    sqlx::query("UPDATE gateway_provider_credential_model_states SET status='active',failure_class=NULL,
        failure_scope=NULL,failure_count=0,last_error=NULL,last_upstream_status=NULL,cooldown_until=NULL,
        updated_at=GREATEST(clock_timestamp(),updated_at+interval '1 microsecond') WHERE provider_account_id=$1")
        .bind(provider).execute(&mut *tx).await.map_err(map_db_error)?;
    // Disabled/archived credential policy is independent and must not be enabled by a provider reset.
    for query in [
        "UPDATE gateway_provider_accounts SET status=CASE WHEN status IN ('cooling','degraded') THEN 'active' ELSE status END,
         cooldown_until=NULL,failure_count=0,last_error=NULL,updated_at=clock_timestamp() WHERE id=$1",
        "UPDATE gateway_provider_credentials SET status=CASE WHEN status IN ('cooling','degraded') THEN 'active' ELSE status END,
         cooldown_until=NULL,failure_count=0,last_error=NULL,updated_at=clock_timestamp() WHERE provider_account_id=$1 AND archived_at IS NULL",
    ] { sqlx::query(query).bind(provider).execute(&mut *tx).await.map_err(map_db_error)?; }
    tx.commit().await.map_err(map_db_error)
}
