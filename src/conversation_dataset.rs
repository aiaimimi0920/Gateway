use serde_json::{json, Map, Value};

use crate::db::GatewayConversationArchiveView;

const REDACTED: &str = "[redacted]";

pub fn clean_training_json_value(value: &Value) -> Value {
    clean_training_json_value_at_key(None, value)
}

pub fn build_clean_dataset_row(
    archive: &GatewayConversationArchiveView,
    request_artifact: Option<Value>,
    response_artifact: Option<Value>,
) -> Value {
    json!({
        "archiveId": archive.id,
        "requestId": archive.request_id,
        "projectId": archive.project_id,
        "userId": archive.user_id,
        "sessionId": archive.session_id,
        "providerAccountId": archive.provider_account_id,
        "providerCredentialRef": archive.provider_credential_ref,
        "protocolFamily": archive.protocol_family,
        "protocolProfile": archive.protocol_profile,
        "endpointKind": archive.endpoint_kind,
        "requestedModel": archive.requested_model,
        "resolvedModel": archive.resolved_model,
        "status": archive.status,
        "failureClass": archive.failure_class,
        "failureScope": archive.failure_scope,
        "request": request_artifact
            .as_ref()
            .map(clean_training_json_value)
            .unwrap_or(Value::Null),
        "response": response_artifact
            .as_ref()
            .map(clean_training_json_value)
            .unwrap_or(Value::Null),
        "redactionVersion": archive.redaction_version,
        "createdAt": archive.created_at,
    })
}

pub fn deterministic_sample_indices(total: usize, sample_size: usize) -> Vec<usize> {
    if total == 0 || sample_size == 0 {
        return Vec::new();
    }
    if sample_size >= total {
        return (0..total).collect();
    }
    if sample_size == 1 {
        return vec![0];
    }

    let max_index = total - 1;
    let sample_max = sample_size - 1;
    let mut indices = Vec::with_capacity(sample_size);
    for position in 0..sample_size {
        let numerator = position * max_index;
        let rounded = (numerator + sample_max / 2) / sample_max;
        if indices.last().copied() != Some(rounded) {
            indices.push(rounded);
        }
    }
    indices
}

pub fn can_publish_dataset(status: &str) -> bool {
    status == "approved"
}

fn clean_training_json_value_at_key(key: Option<&str>, value: &Value) -> Value {
    if key.is_some_and(is_sensitive_key) {
        return Value::String(REDACTED.to_string());
    }

    match value {
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| clean_training_json_value_at_key(None, item))
                .collect(),
        ),
        Value::Object(map) => {
            let mut cleaned = Map::with_capacity(map.len());
            for (entry_key, entry_value) in map {
                cleaned.insert(
                    entry_key.clone(),
                    clean_training_json_value_at_key(Some(entry_key), entry_value),
                );
            }
            Value::Object(cleaned)
        }
        Value::String(raw) if looks_like_inline_secret(raw) => Value::String(REDACTED.to_string()),
        other => other.clone(),
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized: String = key
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    matches!(
        normalized.as_str(),
        "authorization"
            | "cookie"
            | "setcookie"
            | "apikey"
            | "api key"
            | "accesskey"
            | "secret"
            | "token"
            | "accesstoken"
            | "refreshtoken"
            | "sessiontoken"
            | "password"
    ) || normalized.ends_with("apikey")
        || normalized.ends_with("token")
        || normalized.ends_with("secret")
}

fn looks_like_inline_secret(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with("Bearer ")
        || trimmed.starts_with("sk-")
        || trimmed.starts_with("sess-")
        || trimmed.starts_with("ya29.")
}
