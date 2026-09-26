//! Normalize Gemini Business widget sessions without dropping source fields.

use super::super::payload_metadata::apply_folder_sync_metadata;
use super::source::{canonicalize_folder_sync_raw_source, read_optional_object_string};
use crate::db;
use crate::error::GatewayError;
use serde_json::Value;

pub(super) fn normalize_gemini_business_import_payload(
    provider_account: &db::GatewayProviderAccountView,
    raw_payload: Value,
    credential_material_kind_hint: Option<&str>,
) -> Result<Value, GatewayError> {
    let Some(raw_map) = raw_payload.as_object() else {
        return Err(GatewayError::bad_request(
            "gemini business provider credential payload 必须是 JSON object",
        ));
    };

    let api_key = raw_map
        .get("apiKey")
        .or_else(|| raw_map.get("api_key"))
        .or_else(|| raw_map.get("token"))
        .or_else(|| raw_map.get("jwt"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let config_id =
        read_optional_object_string(raw_map, &["configId", "config_id"]).or_else(|| {
            raw_map
                .get("extraBody")
                .or_else(|| raw_map.get("extra_body"))
                .and_then(Value::as_object)
                .and_then(|extra| read_optional_object_string(extra, &["configId", "config_id"]))
        });
    let session =
        read_optional_object_string(raw_map, &["session", "widgetSession", "widget_session"])
            .or_else(|| {
                raw_map
                    .get("extraBody")
                    .or_else(|| raw_map.get("extra_body"))
                    .and_then(Value::as_object)
                    .and_then(|extra| {
                        read_optional_object_string(
                            extra,
                            &[
                                "session",
                                "widgetSession",
                                "widget_session",
                                "upstreamSessionId",
                                "upstream_session_id",
                            ],
                        )
                    })
            });

    let mut payload = raw_map.clone();
    if let Some(api_key) = api_key {
        payload.insert("apiKey".to_string(), Value::String(api_key));
    }
    if config_id.is_some() || session.is_some() {
        let mut extra_body = payload
            .remove("extraBody")
            .or_else(|| payload.remove("extra_body"))
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default();
        if let Some(config_id) = config_id {
            extra_body.insert("configId".to_string(), Value::String(config_id));
        }
        if let Some(session) = session {
            extra_body.insert("session".to_string(), Value::String(session));
        }
        payload.insert("extraBody".to_string(), Value::Object(extra_body));
    }
    apply_folder_sync_metadata(
        &mut payload,
        provider_account,
        credential_material_kind_hint.or(Some("jwt_widget_session")),
    );
    payload.insert(
        "rawSource".to_string(),
        canonicalize_folder_sync_raw_source(&raw_payload),
    );
    Ok(Value::Object(payload))
}
