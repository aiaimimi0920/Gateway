use super::{GatewayProviderCredentialRow, GatewayProviderCredentialView};
use crate::db::format_timestamp;
use crate::error::GatewayError;
use crate::object_storage::{build_gateway_provider_credential_object_key, gateway_object_storage};
use serde_json::{Map, Value};

pub fn merge_provider_account_and_credential_payloads(
    account_payload: &Value,
    credential_payload: &Value,
) -> Value {
    match (account_payload, credential_payload) {
        (Value::Object(account), Value::Object(credential)) => {
            let mut merged = account.clone();
            deep_merge_object(&mut merged, credential);
            Value::Object(merged)
        }
        (_, value) => value.clone(),
    }
}

pub(super) async fn provider_credential_view_from_row(
    row: GatewayProviderCredentialRow,
) -> Result<GatewayProviderCredentialView, GatewayError> {
    let payload = read_payload_from_row(&row).await?;
    Ok(GatewayProviderCredentialView {
        id: row.id,
        provider_account_id: row.provider_account_id,
        label: row.label,
        status: row.status,
        payload,
        storage_mode: row.storage_mode,
        source_kind: row.source_kind,
        source_path: row.source_path,
        source_hash: row.source_hash,
        sync_mode: row.sync_mode,
        sync_state: row.sync_state,
        sync_error: row.sync_error,
        cooldown_until: row.cooldown_until.map(format_timestamp),
        last_error: row.last_error,
        failure_count: row.failure_count,
        last_health_check_at: row.last_health_check_at.map(format_timestamp),
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
        archived_at: row.archived_at.map(format_timestamp),
    })
}

async fn read_payload_from_row(row: &GatewayProviderCredentialRow) -> Result<Value, GatewayError> {
    match &row.payload_inline {
        Some(payload) => Ok(payload.0.clone()),
        None if row.payload_object_key.is_some() => {
            gateway_object_storage()?
                .read_json(row.payload_object_key.as_deref().unwrap_or_default())
                .await
        }
        None => Err(GatewayError::conflict("Provider credential payload 缺失")),
    }
}

pub(super) async fn persist_payload_for_credential(
    provider_credential_id: &str,
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
        .unwrap_or_else(|| build_gateway_provider_credential_object_key(provider_credential_id));
    gateway_object_storage()?
        .put_json(&object_key, payload)
        .await?;
    Ok((None, Some(object_key)))
}

fn deep_merge_object(target: &mut Map<String, Value>, overlay: &Map<String, Value>) {
    for (key, value) in overlay {
        if should_preserve_existing_scalar_for_empty_overlay(target.get(key), key, value) {
            continue;
        }
        match (target.get_mut(key), value) {
            (Some(Value::Object(existing)), Value::Object(incoming)) => {
                deep_merge_object(existing, incoming);
            }
            (Some(existing), Value::String(incoming))
                if incoming.trim().is_empty() && !matches!(existing, Value::Null) =>
            {
                // Credential payload overlays are allowed to omit account-owned
                // defaults via blank strings. Keep the non-empty account-side
                // value instead of degrading runtime routing into relative URLs.
            }
            _ => {
                target.insert(key.clone(), value.clone());
            }
        }
    }
}

fn should_preserve_existing_scalar_for_empty_overlay(
    existing: Option<&Value>,
    key: &str,
    incoming: &Value,
) -> bool {
    if !matches!(
        key,
        "adapter" | "baseUrl" | "base_url" | "apiKey" | "api_key"
    ) {
        return false;
    }

    let Some(existing_text) = existing.and_then(Value::as_str).map(str::trim) else {
        return false;
    };
    let Some(incoming_text) = incoming.as_str().map(str::trim) else {
        return false;
    };

    !existing_text.is_empty() && incoming_text.is_empty()
}
