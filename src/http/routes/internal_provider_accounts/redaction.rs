//! Public provider views and recursive payload/header secret masking.

use crate::db;
use serde_json::Value;

pub(super) fn mask_provider_account_view(view: db::GatewayProviderAccountView) -> Value {
    serde_json::json!({
        "id": view.id,
        "label": view.label,
        "serviceProviderKey": view.service_provider_key,
        "serviceProviderLabel": view.service_provider_label,
        "adapter": view.adapter,
        "protocolFamily": view.protocol_family,
        "protocolProfile": view.protocol_profile,
        "status": view.status,
        "sourceProfile": {
            "sourceKind": view.source_kind.clone().unwrap_or_else(|| "unknown".to_string()),
            "aggregatorApiMode": view.aggregator_api_mode,
            "webReverseAccessMode": view.web_reverse_access_mode,
            "sourceNotes": view.source_notes,
            "derived": view.source_kind.is_none(),
        },
        "sourceKind": view.source_kind,
        "aggregatorApiMode": view.aggregator_api_mode,
        "webReverseAccessMode": view.web_reverse_access_mode,
        "sourceNotes": view.source_notes,
        "executionMode": view.execution_mode,
        "endpointExecutionModes": view.endpoint_execution_modes,
        "payload": mask_provider_payload_secrets(view.payload),
        "storageMode": view.storage_mode,
        "cooldownUntil": view.cooldown_until,
        "lastError": view.last_error,
        "failureCount": view.failure_count,
        "lastHealthCheckAt": view.last_health_check_at,
        "createdAt": view.created_at,
        "updatedAt": view.updated_at,
    })
}

pub(super) fn mask_provider_inventory_view(view: db::GatewayProviderInventoryView) -> Value {
    serde_json::json!({
        "providers": view.providers.into_iter().map(|entry| {
            serde_json::json!({
                "providerAccount": mask_provider_account_view(entry.provider_account),
                "providerHealth": entry.provider_health,
                "costHints": entry.cost_hints,
                "providerQuota": entry.provider_quota,
            })
        }).collect::<Vec<_>>(),
        "summary": view.summary,
    })
}

pub(super) fn mask_provider_payload_secrets(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    let normalized = key.to_lowercase();
                    if matches!(
                        normalized.as_str(),
                        "apikey"
                            | "api_key"
                            | "apisecret"
                            | "api_secret"
                            | "auth_token"
                            | "authorization"
                            | "token"
                            | "cookie"
                    ) {
                        (key, mask_secret_value(value))
                    } else if normalized == "headers" {
                        (key, mask_header_map(value))
                    } else {
                        (key, mask_provider_payload_secrets(value))
                    }
                })
                .collect(),
        ),
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(mask_provider_payload_secrets)
                .collect(),
        ),
        other => other,
    }
}

fn mask_header_map(value: Value) -> Value {
    match value {
        Value::Object(headers) => Value::Object(
            headers
                .into_iter()
                .map(|(key, value)| {
                    let normalized = key.to_lowercase();
                    if matches!(
                        normalized.as_str(),
                        "authorization" | "cookie" | "x-api-key" | "x-goog-api-key"
                    ) {
                        (key, mask_secret_value(value))
                    } else {
                        (key, value)
                    }
                })
                .collect(),
        ),
        other => other,
    }
}

fn mask_secret_value(value: Value) -> Value {
    let Some(raw) = value.as_str().map(str::trim) else {
        return Value::String("***".to_string());
    };
    if raw.is_empty() {
        return Value::String(String::new());
    }
    if raw.len() <= 8 {
        let Some(prefix) = raw.get(..raw.len().min(2)) else {
            return Value::String("***".to_string());
        };
        return Value::String(format!("{prefix}***"));
    }
    let (Some(prefix), Some(suffix)) = (raw.get(..4), raw.get(raw.len() - 2..)) else {
        return Value::String("***".to_string());
    };
    Value::String(format!("{prefix}***{suffix}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn secret_preview_rejects_partial_unicode_boundaries() {
        for secret in ["a\u{00e9}", "abc\u{00e9}defgh", "abcdefghi\u{00e9}x"] {
            let masked = mask_provider_payload_secrets(json!({
                "token": secret,
                "headers": {"Authorization": secret, "X-Trace": "preserved"},
                "nested": [{"api_key": secret}]
            }));
            assert_eq!(masked["token"], "***");
            assert_eq!(masked["headers"]["Authorization"], "***");
            assert_eq!(masked["headers"]["X-Trace"], "preserved");
            assert_eq!(masked["nested"][0]["api_key"], "***");
        }
    }

    #[test]
    fn secret_preview_preserves_ascii_and_complete_unicode_boundaries() {
        for (input, expected) in [
            ("", ""),
            ("   ", ""),
            ("abcdefgh", "ab***"),
            ("  abcdefghi  ", "abcd***hi"),
            ("\u{00e9}", "\u{00e9}***"),
            (
                "\u{00e9}\u{00e9}ABCDE\u{00e9}",
                "\u{00e9}\u{00e9}***\u{00e9}",
            ),
        ] {
            assert_eq!(mask_secret_value(json!(input)), json!(expected));
        }
        for value in [Value::Null, json!(42), json!({"synthetic": "secret"})] {
            assert_eq!(mask_secret_value(value), json!("***"));
        }
    }
}
