use std::sync::Arc;

use serde_json::{json, Map, Value};
use time::{Duration, OffsetDateTime};
use tracing::warn;
use uuid::Uuid;

use crate::db;
use crate::object_storage::{
    build_gateway_conversation_archive_object_key, gateway_object_storage,
};
use crate::protocol::canonical::EndpointKind;
use crate::state::AppState;

pub const CONVERSATION_ARCHIVE_REDACTION_VERSION: &str = "v1";
pub const CONVERSATION_ARCHIVE_REDACTED_VALUE: &str = "[REDACTED]";
pub const CONVERSATION_ARCHIVE_MODE: &str = "ops_forced_full";
pub const CONVERSATION_ARCHIVE_RETENTION_DAYS: i64 = 180;
pub const CONVERSATION_ARCHIVE_MAX_REQUEST_BYTES: usize = 8 * 1024 * 1024;
pub const CONVERSATION_ARCHIVE_MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct PersistConversationArchiveInput {
    pub request_audit_id: Option<String>,
    pub request_id: String,
    pub project_id: Option<String>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub provider_credential_ref: Option<String>,
    pub protocol_family: String,
    pub protocol_profile: Option<String>,
    pub endpoint_kind: String,
    pub requested_model: Option<String>,
    pub resolved_model: Option<String>,
    pub status: String,
    pub upstream_status: Option<u16>,
    pub failure_class: Option<String>,
    pub failure_scope: Option<String>,
    pub request_payload: Value,
    pub response_payload: Option<Value>,
    pub request_already_truncated: bool,
    pub response_already_truncated: bool,
}

pub fn archive_user_id(
    explicit_neuro_user_id: Option<&str>,
    session_user_id: Option<&str>,
) -> Option<String> {
    trim_nonempty(explicit_neuro_user_id).or_else(|| trim_nonempty(session_user_id))
}

pub fn is_conversation_archive_endpoint(endpoint_kind: EndpointKind) -> bool {
    matches!(
        endpoint_kind,
        EndpointKind::ChatCompletions
            | EndpointKind::Completions
            | EndpointKind::Messages
            | EndpointKind::Responses
    )
}

pub fn sanitize_archive_value(value: &Value) -> Value {
    sanitize_archive_value_at_key(None, value)
}

pub fn truncate_archive_text(input: &str, max_bytes: usize) -> (String, bool) {
    if input.len() <= max_bytes {
        return (input.to_string(), false);
    }
    let mut end = max_bytes.min(input.len());
    while end > 0 && !input.is_char_boundary(end) {
        end -= 1;
    }
    (input[..end].to_string(), true)
}

pub async fn persist_conversation_archive(
    state: &Arc<AppState>,
    input: PersistConversationArchiveInput,
) -> Result<Option<db::GatewayConversationArchiveView>, crate::error::GatewayError> {
    let Some(pg_pool) = state.pg_pool.as_ref() else {
        return Ok(None);
    };

    let archive_id = Uuid::new_v4().to_string();
    let mut archive_errors = Vec::new();

    let request_artifact = build_archive_artifact(&archive_id, "request", input.request_payload);
    let (request_artifact, truncated_request) =
        sanitize_and_limit_archive_json(request_artifact, CONVERSATION_ARCHIVE_MAX_REQUEST_BYTES);
    let request_object_key =
        build_gateway_conversation_archive_object_key(&archive_id, "request.json");
    let persisted_request_key = match gateway_object_storage() {
        Ok(storage) => match storage
            .put_json(&request_object_key, &request_artifact)
            .await
        {
            Ok(()) => Some(request_object_key),
            Err(error) => {
                archive_errors.push(format!("request artifact: {}", error.message));
                None
            }
        },
        Err(error) => {
            archive_errors.push(format!("object storage: {}", error.message));
            None
        }
    };

    let (persisted_response_key, truncated_response) = if let Some(response_payload) =
        input.response_payload
    {
        let response_artifact = build_archive_artifact(&archive_id, "response", response_payload);
        let (response_artifact, truncated_response) = sanitize_and_limit_archive_json(
            response_artifact,
            CONVERSATION_ARCHIVE_MAX_RESPONSE_BYTES,
        );
        let response_object_key =
            build_gateway_conversation_archive_object_key(&archive_id, "response.json");
        let persisted_response_key = match gateway_object_storage() {
            Ok(storage) => match storage
                .put_json(&response_object_key, &response_artifact)
                .await
            {
                Ok(()) => Some(response_object_key),
                Err(error) => {
                    archive_errors.push(format!("response artifact: {}", error.message));
                    None
                }
            },
            Err(error) => {
                archive_errors.push(format!("object storage: {}", error.message));
                None
            }
        };
        (persisted_response_key, truncated_response)
    } else {
        (None, false)
    };

    let archive_status = if archive_errors.is_empty() {
        input.status.clone()
    } else if persisted_request_key.is_some() || persisted_response_key.is_some() {
        "partial".to_string()
    } else {
        "archive_failed".to_string()
    };
    let archive_error = (!archive_errors.is_empty()).then(|| archive_errors.join("; "));

    let view = db::create_conversation_archive(
        pg_pool,
        db::CreateConversationArchiveInput {
            id: archive_id,
            request_audit_id: input.request_audit_id.clone(),
            request_id: input.request_id,
            project_id: input.project_id,
            user_id: input.user_id,
            session_id: input.session_id,
            provider_account_id: input.provider_account_id,
            provider_credential_ref: input.provider_credential_ref,
            protocol_family: input.protocol_family,
            protocol_profile: input.protocol_profile,
            endpoint_kind: input.endpoint_kind,
            requested_model: input.requested_model,
            resolved_model: input.resolved_model,
            status: archive_status,
            upstream_status: input.upstream_status,
            failure_class: input.failure_class,
            failure_scope: input.failure_scope,
            request_object_key: persisted_request_key.clone(),
            response_object_key: persisted_response_key.clone(),
            redaction_version: CONVERSATION_ARCHIVE_REDACTION_VERSION.to_string(),
            truncated_request: truncated_request || input.request_already_truncated,
            truncated_response: truncated_response || input.response_already_truncated,
            archive_error,
            retention_expires_at: Some(
                OffsetDateTime::now_utc() + Duration::days(CONVERSATION_ARCHIVE_RETENTION_DAYS),
            ),
        },
    )
    .await?;

    if let Some(request_audit_id) = input.request_audit_id.as_deref() {
        if persisted_request_key.is_some() || persisted_response_key.is_some() {
            if let Err(error) = db::update_request_audit_artifact_keys(
                pg_pool,
                request_audit_id,
                persisted_request_key.as_deref(),
                persisted_response_key.as_deref(),
            )
            .await
            {
                warn!(
                    request_audit_id = %request_audit_id,
                    error = %error,
                    "failed to attach conversation archive artifact keys to request audit"
                );
            }
        }
    }

    Ok(Some(view))
}

fn build_archive_artifact(archive_id: &str, kind: &str, payload: Value) -> Value {
    json!({
        "archiveId": archive_id,
        "kind": kind,
        "archiveMode": CONVERSATION_ARCHIVE_MODE,
        "redactionVersion": CONVERSATION_ARCHIVE_REDACTION_VERSION,
        "payload": payload,
    })
}

fn sanitize_and_limit_archive_json(value: Value, max_bytes: usize) -> (Value, bool) {
    let sanitized = sanitize_archive_value(&value);
    let serialized = serde_json::to_string(&sanitized).unwrap_or_default();
    if serialized.len() <= max_bytes {
        return (sanitized, false);
    }

    let (prefix, _) = truncate_archive_text(&serialized, max_bytes);
    (
        json!({
            "redactionVersion": CONVERSATION_ARCHIVE_REDACTION_VERSION,
            "truncated": true,
            "rawJsonPrefix": prefix,
        }),
        true,
    )
}

fn sanitize_archive_value_at_key(key: Option<&str>, value: &Value) -> Value {
    if key.is_some_and(is_sensitive_archive_key) {
        return Value::String(CONVERSATION_ARCHIVE_REDACTED_VALUE.to_string());
    }

    match value {
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| sanitize_archive_value_at_key(None, item))
                .collect(),
        ),
        Value::Object(object) => {
            let mut sanitized = Map::with_capacity(object.len());
            for (entry_key, entry_value) in object {
                sanitized.insert(
                    entry_key.clone(),
                    sanitize_archive_value_at_key(Some(entry_key), entry_value),
                );
            }
            Value::Object(sanitized)
        }
        _ => value.clone(),
    }
}

fn is_sensitive_archive_key(key: &str) -> bool {
    let normalized = normalize_archive_key(key);
    matches!(
        normalized.as_str(),
        "authorization"
            | "proxyauthorization"
            | "cookie"
            | "setcookie"
            | "apikey"
            | "key"
            | "secret"
            | "password"
            | "token"
            | "accesstoken"
            | "refreshtoken"
            | "idtoken"
            | "sessiontoken"
            | "bearertoken"
            | "authtoken"
            | "oauthtoken"
            | "providertoken"
            | "csrf"
            | "csrftoken"
            | "xsrf"
            | "xsrftoken"
    ) || normalized.ends_with("secret")
        || normalized.ends_with("password")
        || normalized.ends_with("apikey")
        || normalized.ends_with("authtoken")
        || normalized.ends_with("accesstoken")
        || normalized.ends_with("refreshtoken")
        || normalized.ends_with("sessiontoken")
        || normalized.contains("cookie")
}

fn normalize_archive_key(key: &str) -> String {
    key.chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn trim_nonempty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string)
}
