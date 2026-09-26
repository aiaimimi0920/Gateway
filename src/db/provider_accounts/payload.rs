use super::input::parse_execution_mode;
use super::{GatewayProviderAccountRow, GatewayProviderAccountView};
use crate::db::format_timestamp;
use crate::error::GatewayError;
use crate::object_storage::{build_gateway_provider_account_object_key, gateway_object_storage};
use serde_json::Value;

pub(super) async fn provider_account_view_from_row(
    row: GatewayProviderAccountRow,
) -> Result<GatewayProviderAccountView, GatewayError> {
    let payload = read_payload_from_row(&row).await?;
    Ok(GatewayProviderAccountView {
        id: row.id,
        label: row.label,
        service_provider_key: row.service_provider_key,
        service_provider_label: row.service_provider_label,
        adapter: row.adapter,
        protocol_family: row.protocol_family,
        protocol_profile: row.protocol_profile,
        status: row.status,
        source_kind: row.source_kind,
        aggregator_api_mode: row.aggregator_api_mode,
        web_reverse_access_mode: row.web_reverse_access_mode,
        source_notes: row.source_notes,
        execution_mode: parse_execution_mode(row.execution_mode.as_str())?,
        endpoint_execution_modes: row
            .endpoint_execution_modes
            .map(|value| serde_json::from_value(value.0).unwrap_or_default()),
        payload,
        storage_mode: row.storage_mode,
        cooldown_until: row.cooldown_until.map(format_timestamp),
        last_error: row.last_error,
        failure_count: row.failure_count,
        last_health_check_at: row.last_health_check_at.map(format_timestamp),
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    })
}

async fn read_payload_from_row(row: &GatewayProviderAccountRow) -> Result<Value, GatewayError> {
    match &row.payload_inline {
        Some(payload) => Ok(payload.0.clone()),
        None if row.payload_object_key.is_some() => {
            gateway_object_storage()?
                .read_json(row.payload_object_key.as_deref().unwrap_or_default())
                .await
        }
        None => Err(GatewayError::conflict("Provider account payload 缺失")),
    }
}

pub(super) async fn persist_payload_for_account(
    provider_account_id: &str,
    payload: &Value,
    storage_mode: &str,
    existing_object_key: Option<&str>,
) -> Result<(Option<Value>, Option<String>), GatewayError> {
    if storage_mode == "inline" {
        if let Some(existing_object_key) = existing_object_key {
            gateway_object_storage()?
                .delete_object(existing_object_key)
                .await?;
        }
        return Ok((Some(payload.clone()), None));
    }

    let object_key = existing_object_key
        .map(str::to_string)
        .unwrap_or_else(|| build_gateway_provider_account_object_key(provider_account_id));
    gateway_object_storage()?
        .put_json(&object_key, payload)
        .await?;
    Ok((None, Some(object_key)))
}
