//! Suno generation bodies and browser challenge/token wire contracts.

use super::field_values::{read_optional_bool, read_optional_i64, read_optional_string};
use super::request_options::{prompt_from_request, resolve_model};
use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use base64::Engine;
use serde_json::json;
use serde_json::Map;
use serde_json::Value;

pub fn build_generate_request(
    req: &CanonicalRelayRequest,
    model: &str,
    user_tier: Option<&str>,
) -> Result<Value, GatewayError> {
    let body_obj = req
        .raw_body
        .as_object()
        .ok_or_else(|| GatewayError::bad_request("Suno request body must be a JSON object."))?;
    let prompt = prompt_from_request(req)?;
    let mut payload = Map::new();
    let mut metadata = Map::new();

    payload.insert("token".to_string(), Value::Null);
    payload.insert(
        "generation_type".to_string(),
        Value::String("TEXT".to_string()),
    );
    payload.insert(
        "mv".to_string(),
        Value::String(resolve_model(model)?.to_string()),
    );

    let continue_clip_id = read_optional_string(
        body_obj,
        &["continue_clip_id", "continueClipId", "clip_id", "clipId"],
    );
    let continue_at = read_optional_i64(body_obj, &["continue_at", "continueAt"]);
    let challenge_token =
        read_optional_string(body_obj, &["token", "captcha_token", "captchaToken"]);
    let make_instrumental =
        read_optional_bool(body_obj, &["make_instrumental", "makeInstrumental"]).unwrap_or(false);
    let is_max_mode = read_optional_bool(body_obj, &["is_max_mode", "isMaxMode"]).unwrap_or(false);
    let disable_volume_normalization = read_optional_bool(
        body_obj,
        &["disable_volume_normalization", "disableVolumeNormalization"],
    )
    .unwrap_or(false);
    let create_mode = if is_custom_mode(body_obj) {
        "custom"
    } else {
        "simple"
    };
    let lyrics_model = read_optional_string(body_obj, &["lyrics_model", "lyricsModel"])
        .unwrap_or_else(|| "default".to_string());
    let effective_user_tier =
        read_optional_string(body_obj, &["user_tier", "userTier"]).or_else(|| {
            user_tier
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        });

    payload.insert(
        "make_instrumental".to_string(),
        Value::Bool(make_instrumental),
    );
    payload.insert("user_uploaded_images_b64".to_string(), Value::Null);

    if is_custom_mode(body_obj) {
        payload.insert("prompt".to_string(), Value::String(prompt));
        payload.insert(
            "gpt_description_prompt".to_string(),
            Value::String(String::new()),
        );
        if let Some(value) = read_optional_string(body_obj, &["tags"]) {
            payload.insert("tags".to_string(), Value::String(value));
        }
        if let Some(value) = read_optional_string(body_obj, &["title"]) {
            payload.insert("title".to_string(), Value::String(value));
        }
        if let Some(value) = read_optional_string(body_obj, &["negative_tags", "negativeTags"]) {
            payload.insert("negative_tags".to_string(), Value::String(value));
        }
    } else {
        payload.insert("gpt_description_prompt".to_string(), Value::String(prompt));
        payload.insert("prompt".to_string(), Value::String(String::new()));
    }

    if let Some(value) = challenge_token {
        payload.insert("token".to_string(), Value::String(value));
    }
    if let Some(value) = continue_clip_id {
        payload.insert("continue_clip_id".to_string(), Value::String(value));
    } else {
        payload.insert("continue_clip_id".to_string(), Value::Null);
    }
    if let Some(value) = continue_at {
        payload.insert("continue_at".to_string(), Value::Number(value.into()));
    } else {
        payload.insert("continue_at".to_string(), Value::Null);
    }

    metadata.insert(
        "web_client_pathname".to_string(),
        Value::String("/create".to_string()),
    );
    metadata.insert("is_max_mode".to_string(), Value::Bool(is_max_mode));
    metadata.insert(
        "create_mode".to_string(),
        Value::String(create_mode.to_string()),
    );
    if let Some(value) = effective_user_tier {
        metadata.insert("user_tier".to_string(), Value::String(value));
    }
    metadata.insert(
        "create_session_token".to_string(),
        Value::String(uuid::Uuid::new_v4().to_string()),
    );
    metadata.insert(
        "disable_volume_normalization".to_string(),
        Value::Bool(disable_volume_normalization),
    );
    metadata.insert("lyrics_model".to_string(), Value::String(lyrics_model));
    payload.insert("metadata".to_string(), Value::Object(metadata));
    payload.insert("override_fields".to_string(), Value::Array(Vec::new()));
    payload.insert("cover_clip_id".to_string(), Value::Null);
    payload.insert("cover_start_s".to_string(), Value::Null);
    payload.insert("cover_end_s".to_string(), Value::Null);
    payload.insert("persona_id".to_string(), Value::Null);
    payload.insert("artist_clip_id".to_string(), Value::Null);
    payload.insert("artist_start_s".to_string(), Value::Null);
    payload.insert("artist_end_s".to_string(), Value::Null);
    payload.insert("continued_aligned_prompt".to_string(), Value::Null);
    payload.insert(
        "transaction_uuid".to_string(),
        Value::String(uuid::Uuid::new_v4().to_string()),
    );

    Ok(Value::Object(payload))
}

pub fn build_challenge_check_request() -> Value {
    json!({
        "ctype": "generation"
    })
}

pub fn challenge_required(body: &Value) -> Result<bool, GatewayError> {
    body.get("required")
        .and_then(|value| value.as_bool())
        .ok_or_else(|| {
            GatewayError::server_error("Suno challenge probe response missing required flag.")
                .with_provider("suno_compatible")
                .with_code("suno_invalid_challenge_probe")
        })
}

pub fn has_challenge_token(req: &CanonicalRelayRequest) -> bool {
    req.raw_body
        .as_object()
        .and_then(|body_obj| {
            read_optional_string(body_obj, &["token", "captcha_token", "captchaToken"])
        })
        .is_some()
}

pub fn build_browser_token_header_value(timestamp_ms: i64) -> String {
    let encoded = base64::engine::general_purpose::STANDARD
        .encode(json!({ "timestamp": timestamp_ms }).to_string().as_bytes());
    json!({
        "token": encoded,
    })
    .to_string()
}

fn is_custom_mode(body_obj: &Map<String, Value>) -> bool {
    body_obj
        .get("custom_mode")
        .or_else(|| body_obj.get("customMode"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
        || read_optional_string(body_obj, &["lyrics"]).is_some()
        || read_optional_string(body_obj, &["tags"]).is_some()
        || read_optional_string(body_obj, &["title"]).is_some()
        || read_optional_string(body_obj, &["negative_tags", "negativeTags"]).is_some()
        || read_optional_string(body_obj, &["continue_clip_id", "continueClipId"]).is_some()
        || read_optional_i64(body_obj, &["continue_at", "continueAt"]).is_some()
}
