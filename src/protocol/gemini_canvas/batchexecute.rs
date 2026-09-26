use serde_json::{json, Value};

use super::{
    defaults::*, request_ids::current_reqid,
    stream_request::media_operation_selection_preflight_mode_index,
};
use crate::error::GatewayError;
use crate::protocol::gemini_web;

fn normalize_source_path(source_path: &str) -> String {
    let trimmed = source_path.trim();
    if trimmed.is_empty() {
        "/app".to_string()
    } else if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

pub fn build_text_batchexecute_request(
    rpcid: &str,
    payload: Value,
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    let language = bootstrap.language.trim();
    let language = if language.is_empty() { "en" } else { language };
    let inner_payload = serde_json::to_string(&payload).map_err(|error| {
        GatewayError::server_error(format!(
            "serialize Gemini Canvas batchexecute payload: {error}"
        ))
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_batchexecute_serialize_failed")
    })?;
    let rpc = Value::Array(vec![
        Value::String(rpcid.to_string()),
        Value::String(inner_payload),
        Value::Null,
        Value::String("generic".to_string()),
    ]);
    let f_req = serde_json::to_string(&vec![Value::Array(vec![rpc])]).map_err(|error| {
        GatewayError::server_error(format!(
            "serialize Gemini Canvas batchexecute wrapper: {error}"
        ))
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_batchexecute_serialize_failed")
    })?;

    let mut query = vec![
        ("rpcids".to_string(), rpcid.to_string()),
        (
            "source-path".to_string(),
            normalize_source_path(source_path),
        ),
        ("hl".to_string(), language.to_string()),
        ("_reqid".to_string(), current_reqid().to_string()),
        ("rt".to_string(), "c".to_string()),
    ];
    if let Some(build_label) = bootstrap
        .build_label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query.push(("bl".to_string(), build_label.to_string()));
    }
    if let Some(session_id) = bootstrap
        .session_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query.push(("f.sid".to_string(), session_id.to_string()));
    }

    let mut form = vec![("f.req".to_string(), f_req)];
    if let Some(access_token) = bootstrap
        .access_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        form.push(("at".to_string(), access_token.to_string()));
    }

    Ok(gemini_web::GeminiWebRequest { query, form })
}

pub fn build_text_state_variant_preflight_request(
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
    state_len: usize,
    tail_index: usize,
    tail_value: Value,
    marker: &str,
    rpcid: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    let mut state = vec![Value::Null; state_len];
    if tail_index >= state_len {
        return Err(GatewayError::server_error(format!(
            "Gemini Canvas state preflight tail index {tail_index} was out of bounds for length {state_len}."
        ))
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_state_preflight_invalid_shape"));
    }
    state[tail_index] = tail_value;
    let payload = vec![
        Value::Array(state),
        Value::Array(vec![Value::Array(vec![Value::String(marker.to_string())])]),
    ];
    build_text_batchexecute_request(rpcid, Value::Array(payload), bootstrap, source_path)
}

pub fn build_mode_selection_preflight_request(
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
    selected_id: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    build_text_state_variant_preflight_request(
        bootstrap,
        source_path,
        100,
        99,
        Value::String(selected_id.to_string()),
        "last_selected_mode_id_on_web",
        GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID,
    )
}

pub fn build_text_mode_selection_preflight_request(
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    build_mode_selection_preflight_request(
        bootstrap,
        source_path,
        GEMINI_CANVAS_TEXT_LAST_SELECTED_MODE_ID,
    )
}

pub fn build_media_operation_selection_preflight_request(
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
    mode_index: i64,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    let mode_index = media_operation_selection_preflight_mode_index(mode_index);
    build_text_batchexecute_request(
        GEMINI_CANVAS_MEDIA_OPERATION_SELECTION_RPCID,
        json!([[[1, mode_index], [2, mode_index], [6, mode_index]]]),
        bootstrap,
        source_path,
    )
}

pub fn build_text_bootstrap_preflight_request(
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    build_text_batchexecute_request(
        GEMINI_CANVAS_TEXT_BOOTSTRAP_RPCID,
        Value::Array(Vec::new()),
        bootstrap,
        source_path,
    )
}

pub fn build_text_state_preflight_request(
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    build_text_batchexecute_request(
        GEMINI_CANVAS_TEXT_STATE_PREFLIGHT_RPCID,
        json!([[null, null, null, null, true]]),
        bootstrap,
        source_path,
    )
}

pub fn build_image_state_keys_preflight_request(
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    build_text_batchexecute_request(
        GEMINI_CANVAS_TEXT_STATE_PREFLIGHT_RPCID,
        json!([[GEMINI_CANVAS_IMAGE_STATE_KEYS_PREFLIGHT_KEYS]]),
        bootstrap,
        source_path,
    )
}
