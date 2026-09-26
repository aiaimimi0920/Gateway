use crate::db::{format_timestamp, map_db_error};
use crate::error::GatewayError;
use serde_json::Value;
use sqlx::PgPool;
use time::OffsetDateTime;

use super::*;

pub(super) async fn insert_signal_event(
    pool: &PgPool,
    policy: &CredentialStockPolicyView,
    signal_key: &str,
    severity: StockSeverity,
    payload: &Value,
) -> Result<CredentialStockSignalEventView, GatewayError> {
    let id = format!("credential_stock_signal_{}", uuid::Uuid::new_v4());
    let now = OffsetDateTime::now_utc();
    let row = sqlx::query_as::<_, CredentialStockSignalEventRow>(
        r#"
        insert into gateway_credential_stock_signal_events (
          id, policy_id, stock_class_key, signal_key, severity, stream,
          payload, published_at, created_at
        ) values ($1, $2, $3, $4, $5, $6, $7, null, $8)
        returning *
        "#,
    )
    .bind(id)
    .bind(policy.id.as_str())
    .bind(policy.stock_class_key.as_str())
    .bind(signal_key)
    .bind(format!("{:?}", severity).to_ascii_lowercase())
    .bind(policy.signal_stream.as_str())
    .bind(sqlx::types::Json(payload.clone()))
    .bind(now)
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;
    Ok(signal_event_view_from_row(row))
}

pub(super) async fn mark_policy_signal_sent(
    pool: &PgPool,
    policy_id: &str,
    signal_key: &str,
) -> Result<(), GatewayError> {
    sqlx::query(
        r#"
        update gateway_credential_stock_policies
        set last_signal_key = $2, last_signal_at = $3, updated_at = $3
        where id = $1
        "#,
    )
    .bind(policy_id)
    .bind(signal_key)
    .bind(OffsetDateTime::now_utc())
    .execute(pool)
    .await
    .map_err(map_db_error)?;
    Ok(())
}

pub(super) async fn mark_signal_event_published(
    pool: &PgPool,
    event_id: &str,
) -> Result<String, GatewayError> {
    let published_at = OffsetDateTime::now_utc();
    sqlx::query(
        r#"
        update gateway_credential_stock_signal_events
        set published_at = $2
        where id = $1
        "#,
    )
    .bind(event_id)
    .bind(published_at)
    .execute(pool)
    .await
    .map_err(map_db_error)?;
    Ok(format_timestamp(published_at))
}

fn signal_event_view_from_row(
    row: CredentialStockSignalEventRow,
) -> CredentialStockSignalEventView {
    CredentialStockSignalEventView {
        id: row.id,
        policy_id: row.policy_id,
        stock_class_key: row.stock_class_key,
        signal_key: row.signal_key,
        severity: match row.severity.as_str() {
            "critical" => StockSeverity::Critical,
            "warning" => StockSeverity::Warning,
            "overstock" => StockSeverity::Overstock,
            _ => StockSeverity::Healthy,
        },
        stream: row.stream,
        payload: row.payload.0,
        published_at: row.published_at.map(format_timestamp),
        created_at: format_timestamp(row.created_at),
    }
}
