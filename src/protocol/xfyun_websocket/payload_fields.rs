use super::XFYUN_WEBSOCKET_DEFAULT_PATH;
use crate::error::GatewayError;
use crate::routing::candidate::ProviderAccountPayload;
use serde_json::Value;
use std::collections::HashMap;

pub(super) fn read_required_app_id(
    payload: &ProviderAccountPayload,
) -> Result<String, GatewayError> {
    read_payload_string(payload.extra_body.as_ref(), &["appId", "app_id"]).ok_or_else(|| {
        GatewayError::unauthorized("XFYun native WebSocket provider missing APPID")
            .with_code("xfyun_websocket_missing_app_id")
            .with_provider("xfyun_websocket_compatible")
    })
}

pub(super) fn read_required_api_secret(
    payload: &ProviderAccountPayload,
) -> Result<String, GatewayError> {
    payload
        .auth_token
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .or_else(|| read_payload_string(payload.extra_body.as_ref(), &["apiSecret", "api_secret"]))
        .ok_or_else(|| {
            GatewayError::unauthorized("XFYun native WebSocket provider missing APISecret")
                .with_code("xfyun_websocket_missing_api_secret")
                .with_provider("xfyun_websocket_compatible")
        })
}

pub(super) fn websocket_path(payload: &ProviderAccountPayload) -> String {
    payload
        .chat_completions_path
        .clone()
        .or_else(|| {
            read_payload_string(
                payload.extra_body.as_ref(),
                &["wsPath", "websocketPath", "path"],
            )
        })
        .unwrap_or_else(|| XFYUN_WEBSOCKET_DEFAULT_PATH.to_string())
}

pub(super) fn read_payload_string(
    extra: Option<&HashMap<String, Value>>,
    keys: &[&str],
) -> Option<String> {
    let extra = extra?;
    keys.iter().find_map(|key| {
        extra
            .get(*key)
            .and_then(|entry| entry.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

pub(super) fn read_payload_array(
    extra: Option<&HashMap<String, Value>>,
    keys: &[&str],
) -> Option<Value> {
    let extra = extra?;
    keys.iter()
        .find_map(|key| extra.get(*key))
        .and_then(|entry| entry.as_array().cloned())
        .map(Value::Array)
}
