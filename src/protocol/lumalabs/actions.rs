//! LumaLabs action request bodies, context headers and returned identities.

use super::request_options::{
    action_type_for_operation, aspect_ratio_from_request, output_format_from_request,
    read_optional_string, seed_from_request, style_from_request,
};
use super::runtime::LumalabsRuntime;
use super::LumalabsMediaOperation;
use super::DEFAULT_AUDIO_OUTPUT_FORMAT;
use super::DEFAULT_IMAGE_ACTION_TYPE;
use super::DEFAULT_IMAGE_ARTIFACT_FIELD;
use super::DEFAULT_VIDEO_OUTPUT_FORMAT;
use super::LUMALABS_DEFAULT_IMAGE_MODEL;
use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use rand::{distributions::Alphanumeric, Rng};
use serde_json::json;
use serde_json::Map;
use serde_json::Value;

pub fn generate_optimistic_output_id() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(8)
        .map(char::from)
        .collect::<String>()
        .to_lowercase()
}

pub fn build_action_request(
    prompt: &str,
    aspect_ratio: &str,
    seed: &str,
    style: &str,
    output_format: &str,
    optimistic_output_id: &str,
) -> Value {
    json!({
        "type": DEFAULT_IMAGE_ACTION_TYPE,
        "fields": {
            "prompt": prompt,
            "aspect_ratio": aspect_ratio,
            "seed": seed,
            "style": style,
            "output_format": output_format,
        },
        "optimistic_output_ids": [optimistic_output_id],
    })
}

pub fn build_action_request_for_operation(
    req: &CanonicalRelayRequest,
    runtime: &LumalabsRuntime,
    operation: LumalabsMediaOperation,
    model: &str,
    prompt: &str,
    optimistic_output_id: &str,
) -> Value {
    let mut fields = Map::new();
    fields.insert("prompt".to_string(), Value::String(prompt.to_string()));

    match operation {
        LumalabsMediaOperation::Image => {
            fields.insert(
                "aspect_ratio".to_string(),
                Value::String(aspect_ratio_from_request(req)),
            );
            fields.insert("style".to_string(), Value::String(style_from_request(req)));
            fields.insert(
                "output_format".to_string(),
                Value::String(output_format_from_request(req)),
            );
            let seed = seed_from_request(req);
            if !seed.is_empty() {
                fields.insert("seed".to_string(), Value::String(seed));
            }
            if model.trim() != LUMALABS_DEFAULT_IMAGE_MODEL {
                fields.insert("model".to_string(), Value::String(model.trim().to_string()));
            }
        }
        LumalabsMediaOperation::Video => {
            fields.insert(
                "aspect_ratio".to_string(),
                Value::String(aspect_ratio_from_request(req)),
            );
            fields.insert(
                "output_format".to_string(),
                Value::String(
                    read_optional_string(&req.raw_body, &["output_format", "format"])
                        .unwrap_or_else(|| DEFAULT_VIDEO_OUTPUT_FORMAT.to_string()),
                ),
            );
            maybe_insert_alias_value(
                &mut fields,
                &req.raw_body,
                &["duration", "durationSeconds", "duration_seconds"],
                "duration_seconds",
            );
            maybe_insert_alias_value(
                &mut fields,
                &req.raw_body,
                &["negative_prompt", "negativePrompt"],
                "negative_prompt",
            );
            maybe_insert_alias_value(
                &mut fields,
                &req.raw_body,
                &["camera_motion", "cameraMotion"],
                "camera_motion",
            );
            maybe_insert_alias_value(&mut fields, &req.raw_body, &["resolution"], "resolution");
            maybe_insert_alias_value(&mut fields, &req.raw_body, &["loop"], "loop");
            maybe_insert_alias_value(&mut fields, &req.raw_body, &["seed"], "seed");
        }
        LumalabsMediaOperation::Audio => {
            let action_type = action_type_for_operation(req, runtime, operation);
            fields.insert(
                "output_format".to_string(),
                Value::String(
                    read_optional_string(&req.raw_body, &["output_format", "format"])
                        .unwrap_or_else(|| DEFAULT_AUDIO_OUTPUT_FORMAT.to_string()),
                ),
            );
            maybe_insert_alias_value(
                &mut fields,
                &req.raw_body,
                &["duration", "durationSeconds", "duration_seconds"],
                "duration_seconds",
            );
            maybe_insert_alias_value(
                &mut fields,
                &req.raw_body,
                &["negative_prompt", "negativePrompt"],
                "negative_prompt",
            );
            if audio_action_type_supports_lyrics(&action_type) {
                maybe_insert_alias_value(&mut fields, &req.raw_body, &["lyrics"], "lyrics");
            }
            if audio_action_type_supports_voice(&action_type) {
                maybe_insert_alias_value(&mut fields, &req.raw_body, &["voice"], "voice");
            }
            maybe_insert_alias_value(&mut fields, &req.raw_body, &["mode"], "mode");
            maybe_insert_alias_value(&mut fields, &req.raw_body, &["seed"], "seed");
        }
    }

    json!({
        "type": action_type_for_operation(req, runtime, operation),
        "fields": Value::Object(fields),
        "optimistic_output_ids": [optimistic_output_id],
    })
}

pub fn extract_action_output_id(body: &Value) -> Result<String, GatewayError> {
    extract_action_output_id_for_field(body, DEFAULT_IMAGE_ARTIFACT_FIELD)
}

pub fn extract_action_output_id_for_field(
    body: &Value,
    artifact_field: &str,
) -> Result<String, GatewayError> {
    body.get("output_artifacts")
        .and_then(|v| v.get(artifact_field))
        .and_then(|v| v.as_array())
        .and_then(|values| values.first())
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| {
            GatewayError::server_error(format!(
                "LumaLabs action response did not include output_artifacts.{artifact_field}[0]."
            ))
            .with_provider("lumalabs_compatible")
            .with_code("lumalabs_missing_output_id")
        })
}

pub fn extract_action_id(body: &Value) -> Option<String> {
    body.get("action")
        .and_then(|v| v.get("id"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub fn build_board_referer(base_url: &str, realm_id: &str) -> String {
    format!("{}/board/{}", base_url.trim_end_matches('/'), realm_id)
}

pub fn build_client_context(locale: &str) -> String {
    format!(
        "id={},name=web,locale={}",
        uuid::Uuid::new_v4(),
        locale.trim()
    )
}

fn audio_action_type_supports_lyrics(action_type: &str) -> bool {
    let normalized = action_type.trim().to_ascii_lowercase();
    normalized.contains("music")
}

fn audio_action_type_supports_voice(action_type: &str) -> bool {
    let normalized = action_type.trim().to_ascii_lowercase();
    normalized.contains("speech") || normalized.contains("tts")
}

fn maybe_insert_alias_value(
    fields: &mut Map<String, Value>,
    body: &Value,
    aliases: &[&str],
    target: &str,
) {
    if let Some(value) = clone_optional_value(body, aliases) {
        fields.insert(target.to_string(), value);
    }
}

fn clone_optional_value(body: &Value, keys: &[&str]) -> Option<Value> {
    let map = body.as_object()?;
    for key in keys {
        if let Some(value) = map.get(*key) {
            return Some(value.clone());
        }
    }
    None
}
