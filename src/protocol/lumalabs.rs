use base64::Engine;
use rand::{distributions::Alphanumeric, Rng};
use regex::Regex;
use serde_json::{json, Map, Value};

use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::routing::candidate::ProviderAccountPayload;

pub const LUMALABS_DEFAULT_MODEL: &str = "uni-1";
pub const LUMALABS_DEFAULT_IMAGE_MODEL: &str = "uni-1";
pub const LUMALABS_DEFAULT_VIDEO_MODEL: &str = "ray3.14";
pub const LUMALABS_DEFAULT_AUDIO_MODEL: &str = "elevenlabs-music-v1";

const DEFAULT_IMAGE_ACTION_TYPE: &str = "create_image_uni_1";
const DEFAULT_VIDEO_ACTION_TYPE: &str = "create_video_ray3_14";
const DEFAULT_AUDIO_ACTION_TYPE: &str = "text_to_music_elevenlabs_v1";
const DEFAULT_ASPECT_RATIO: &str = "1:1";
const DEFAULT_STYLE: &str = "auto";
const DEFAULT_IMAGE_OUTPUT_FORMAT: &str = "png";
const DEFAULT_VIDEO_OUTPUT_FORMAT: &str = "mp4";
const DEFAULT_AUDIO_OUTPUT_FORMAT: &str = "mp3";
const DEFAULT_IMAGE_ARTIFACT_FIELD: &str = "image";
const DEFAULT_VIDEO_ARTIFACT_FIELD: &str = "video";
const DEFAULT_AUDIO_ARTIFACT_FIELD: &str = "audio";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LumalabsMediaOperation {
    Image,
    Video,
    Audio,
}

impl LumalabsMediaOperation {
    pub fn from_endpoint_kind(
        endpoint_kind: crate::protocol::canonical::EndpointKind,
    ) -> Result<Self, GatewayError> {
        match endpoint_kind {
            crate::protocol::canonical::EndpointKind::ImagesGenerations => Ok(Self::Image),
            crate::protocol::canonical::EndpointKind::ImagesEdits => Err(
                GatewayError::bad_request(
                    "LumaLabs adapters do not currently support /v1/images/edits.",
                )
                .with_provider("lumalabs_compatible")
                .with_code("unsupported_lumalabs_edit_endpoint"),
            ),
            crate::protocol::canonical::EndpointKind::VideosGenerations => Ok(Self::Video),
            crate::protocol::canonical::EndpointKind::MusicGenerations => Ok(Self::Audio),
            _ => Err(
                GatewayError::bad_request(
                    "LumaLabs adapters currently support /v1/images/generations, /v1/videos/generations, and /v1/music/generations.",
                )
                .with_code("unsupported_lumalabs_endpoint"),
            ),
        }
    }

    pub fn unsupported_output_count_error(self) -> GatewayError {
        let (message, code) = match self {
            Self::Image => (
                "LumaLabs image generation currently supports only n=1 requests.",
                "unsupported_lumalabs_image_count",
            ),
            Self::Video => (
                "LumaLabs video generation currently supports only n=1 requests.",
                "unsupported_lumalabs_video_count",
            ),
            Self::Audio => (
                "LumaLabs audio generation currently supports only n=1 requests.",
                "unsupported_lumalabs_audio_count",
            ),
        };
        GatewayError::bad_request(message)
            .with_provider("lumalabs_compatible")
            .with_code(code)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LumalabsRuntime {
    pub realm_id: String,
    pub image_action_type: Option<String>,
    pub video_action_type: Option<String>,
    pub audio_action_type: Option<String>,
    pub image_artifact_field: String,
    pub video_artifact_field: String,
    pub audio_artifact_field: String,
}

pub fn runtime_from_payload(
    payload: &ProviderAccountPayload,
) -> Result<LumalabsRuntime, GatewayError> {
    let extra = payload.extra_body.as_ref().ok_or_else(|| {
        GatewayError::bad_request(
            "LumaLabs credentials require extra_body runtime material (realmId).",
        )
        .with_code("missing_lumalabs_runtime")
    })?;

    let realm_id =
        read_required_hash_string(extra, &["realmId", "realm_id", "boardId", "board_id"])?;
    Ok(LumalabsRuntime {
        realm_id,
        image_action_type: read_optional_hash_string(
            extra,
            &[
                "imageActionType",
                "image_action_type",
                "imagesActionType",
                "images_action_type",
            ],
        ),
        video_action_type: read_optional_hash_string(
            extra,
            &[
                "videoActionType",
                "video_action_type",
                "videosActionType",
                "videos_action_type",
            ],
        ),
        audio_action_type: read_optional_hash_string(
            extra,
            &[
                "audioActionType",
                "audio_action_type",
                "musicActionType",
                "music_action_type",
            ],
        ),
        image_artifact_field: read_optional_hash_string(
            extra,
            &[
                "imageArtifactField",
                "image_artifact_field",
                "imagesArtifactField",
                "images_artifact_field",
            ],
        )
        .unwrap_or_else(|| DEFAULT_IMAGE_ARTIFACT_FIELD.to_string()),
        video_artifact_field: read_optional_hash_string(
            extra,
            &[
                "videoArtifactField",
                "video_artifact_field",
                "videosArtifactField",
                "videos_artifact_field",
            ],
        )
        .unwrap_or_else(|| DEFAULT_VIDEO_ARTIFACT_FIELD.to_string()),
        audio_artifact_field: read_optional_hash_string(
            extra,
            &[
                "audioArtifactField",
                "audio_artifact_field",
                "musicArtifactField",
                "music_artifact_field",
            ],
        )
        .unwrap_or_else(|| DEFAULT_AUDIO_ARTIFACT_FIELD.to_string()),
    })
}

pub fn unsupported_request_plan_error() -> GatewayError {
    GatewayError::bad_request(
        "LumaLabs adapters currently support image, video, and audio-generation passthrough endpoints",
    )
    .with_code("unsupported_lumalabs_endpoint")
}

pub fn unsupported_image_inputs_error() -> GatewayError {
    GatewayError::bad_request(
        "LumaLabs image generation currently supports prompt-only requests and does not accept image uploads or masks.",
    )
    .with_provider("lumalabs_compatible")
    .with_code("unsupported_lumalabs_image_inputs")
}

pub fn extract_remote_executor_signed_url(value: &Value) -> Result<String, GatewayError> {
    value
        .get("signedUrl")
        .and_then(|entry| entry.as_str())
        .map(str::to_string)
        .ok_or_else(|| {
            GatewayError::server_error(
                "Remote LumaLabs browser executor succeeded without a signedUrl.",
            )
            .with_provider("lumalabs_compatible")
            .with_code("lumalabs_browser_executor_missing_signed_url")
        })
}

pub fn extract_browser_worker_signed_url(
    signed_url: Option<String>,
) -> Result<String, GatewayError> {
    signed_url.ok_or_else(|| {
        GatewayError::server_error("LumaLabs browser worker reported success without a signed URL.")
            .with_provider("lumalabs_compatible")
            .with_code("lumalabs_browser_worker_missing_signed_url")
    })
}

pub fn empty_browser_worker_output_error(stderr: &str) -> GatewayError {
    GatewayError::server_error(format!(
        "LumaLabs browser worker did not return JSON output. stderr: {}",
        if stderr.is_empty() { "<empty>" } else { stderr }
    ))
    .with_provider("lumalabs_compatible")
    .with_code("lumalabs_browser_worker_empty_output")
}

pub fn browser_worker_output_parse_error(error: &str, stdout: &str) -> GatewayError {
    GatewayError::server_error(format!(
        "Failed to parse LumaLabs browser worker output: {error}. stdout: {stdout}"
    ))
    .with_provider("lumalabs_compatible")
    .with_code("lumalabs_browser_worker_output_parse_failed")
}

pub fn browser_worker_wait_failed_error(error: &str) -> GatewayError {
    GatewayError::server_error(format!(
        "LumaLabs browser worker failed before producing output: {error}"
    ))
    .with_provider("lumalabs_compatible")
    .with_code("lumalabs_browser_worker_wait_failed")
}

pub fn browser_worker_stdin_error(error: &str) -> GatewayError {
    GatewayError::server_error(format!(
        "Failed to write LumaLabs browser worker input: {error}"
    ))
    .with_provider("lumalabs_compatible")
    .with_code("lumalabs_browser_worker_stdin_failed")
}

pub fn browser_worker_spawn_failed_error(
    script_path: &std::path::Path,
    error: &str,
) -> GatewayError {
    GatewayError::server_error(format!(
        "Failed to launch LumaLabs browser worker at {}: {error}",
        script_path.display()
    ))
    .with_provider("lumalabs_compatible")
    .with_code("lumalabs_browser_worker_spawn_failed")
}

pub fn browser_worker_input_serialize_error(error: &str) -> GatewayError {
    GatewayError::server_error(format!(
        "Failed to serialize LumaLabs browser worker input: {error}"
    ))
    .with_provider("lumalabs_compatible")
    .with_code("lumalabs_browser_worker_input_serialize_failed")
}

pub fn prompt_from_request(req: &CanonicalRelayRequest) -> Result<String, GatewayError> {
    prompt_from_request_for_operation(req, LumalabsMediaOperation::Image)
}

pub fn prompt_from_request_for_operation(
    req: &CanonicalRelayRequest,
    operation: LumalabsMediaOperation,
) -> Result<String, GatewayError> {
    let fields = match operation {
        LumalabsMediaOperation::Image | LumalabsMediaOperation::Video => {
            &["prompt", "input", "description"][..]
        }
        LumalabsMediaOperation::Audio => &["prompt", "input", "lyrics", "description"][..],
    };
    if let Some(prompt) = read_optional_string(&req.raw_body, fields) {
        if !prompt.is_empty() {
            return Ok(prompt);
        }
    }

    let prompt = req.messages_text().trim().to_string();
    if prompt.is_empty() {
        let (message, code) = match operation {
            LumalabsMediaOperation::Image => (
                "LumaLabs image requests require a prompt.",
                "missing_prompt",
            ),
            LumalabsMediaOperation::Video => (
                "LumaLabs video requests require a prompt.",
                "missing_video_prompt",
            ),
            LumalabsMediaOperation::Audio => (
                "LumaLabs audio requests require a prompt or lyrics.",
                "missing_audio_prompt",
            ),
        };
        return Err(GatewayError::bad_request(message).with_code(code));
    }
    Ok(prompt)
}

pub fn requested_image_count(req: &CanonicalRelayRequest) -> usize {
    requested_output_count(req)
}

pub fn requested_output_count(req: &CanonicalRelayRequest) -> usize {
    req.raw_body
        .get("n")
        .and_then(|v| v.as_u64())
        .map(|value| value.max(1) as usize)
        .unwrap_or(1)
}

pub fn default_model_for_operation(operation: LumalabsMediaOperation) -> &'static str {
    match operation {
        LumalabsMediaOperation::Image => LUMALABS_DEFAULT_IMAGE_MODEL,
        LumalabsMediaOperation::Video => LUMALABS_DEFAULT_VIDEO_MODEL,
        LumalabsMediaOperation::Audio => LUMALABS_DEFAULT_AUDIO_MODEL,
    }
}

pub fn default_action_type_for_operation(operation: LumalabsMediaOperation) -> &'static str {
    match operation {
        LumalabsMediaOperation::Image => DEFAULT_IMAGE_ACTION_TYPE,
        LumalabsMediaOperation::Video => DEFAULT_VIDEO_ACTION_TYPE,
        LumalabsMediaOperation::Audio => DEFAULT_AUDIO_ACTION_TYPE,
    }
}

pub fn media_operation_name(operation: LumalabsMediaOperation) -> &'static str {
    match operation {
        LumalabsMediaOperation::Image => "image",
        LumalabsMediaOperation::Video => "video",
        LumalabsMediaOperation::Audio => "audio",
    }
}

fn requested_action_type_override(req: &CanonicalRelayRequest) -> Option<String> {
    read_optional_string(
        &req.raw_body,
        &["action_type", "web_action_type", "upstream_action_type"],
    )
}

fn configured_action_type_for_operation(
    runtime: &LumalabsRuntime,
    operation: LumalabsMediaOperation,
) -> Option<String> {
    match operation {
        LumalabsMediaOperation::Image => runtime.image_action_type.clone(),
        LumalabsMediaOperation::Video => runtime.video_action_type.clone(),
        LumalabsMediaOperation::Audio => runtime.audio_action_type.clone(),
    }
}

pub fn action_type_for_operation(
    req: &CanonicalRelayRequest,
    runtime: &LumalabsRuntime,
    operation: LumalabsMediaOperation,
) -> String {
    requested_action_type_override(req)
        .or_else(|| configured_action_type_for_operation(runtime, operation))
        .unwrap_or_else(|| default_action_type_for_operation(operation).to_string())
}

pub fn should_auto_discover_action_type(
    req: &CanonicalRelayRequest,
    runtime: &LumalabsRuntime,
    operation: LumalabsMediaOperation,
) -> bool {
    requested_action_type_override(req).is_none()
        && configured_action_type_for_operation(runtime, operation).is_none()
}

pub fn output_artifact_field_for_operation(
    req: &CanonicalRelayRequest,
    runtime: &LumalabsRuntime,
    operation: LumalabsMediaOperation,
) -> String {
    read_optional_string(
        &req.raw_body,
        &[
            "artifact_field",
            "output_artifact_field",
            "web_output_artifact_field",
        ],
    )
    .unwrap_or_else(|| match operation {
        LumalabsMediaOperation::Image => runtime.image_artifact_field.clone(),
        LumalabsMediaOperation::Video => runtime.video_artifact_field.clone(),
        LumalabsMediaOperation::Audio => runtime.audio_artifact_field.clone(),
    })
}

pub fn prefers_url_response(req: &CanonicalRelayRequest) -> Result<bool, GatewayError> {
    let Some(value) = req.raw_body.get("response_format") else {
        return Ok(true);
    };
    let Some(format) = value.as_str() else {
        return Err(
            GatewayError::bad_request("response_format must be a string when provided.")
                .with_code("invalid_image_response_format"),
        );
    };
    match format {
        "url" => Ok(true),
        "b64_json" => Ok(false),
        _ => Err(GatewayError::bad_request(
            "LumaLabs image endpoints currently support response_format=url or b64_json.",
        )
        .with_code("unsupported_image_response_format")),
    }
}

pub fn aspect_ratio_from_request(req: &CanonicalRelayRequest) -> String {
    let Some(size) = read_optional_string(&req.raw_body, &["size", "aspect_ratio"]) else {
        return DEFAULT_ASPECT_RATIO.to_string();
    };

    match size.as_str() {
        "1024x1024" | "1:1" => "1:1".to_string(),
        "1024x1792" | "9:16" => "9:16".to_string(),
        "1792x1024" | "16:9" => "16:9".to_string(),
        "auto" => "auto".to_string(),
        _ if is_ratio_string(&size) => size,
        _ => DEFAULT_ASPECT_RATIO.to_string(),
    }
}

pub fn style_from_request(req: &CanonicalRelayRequest) -> String {
    read_optional_string(&req.raw_body, &["style"]).unwrap_or_else(|| DEFAULT_STYLE.to_string())
}

pub fn output_format_from_request(req: &CanonicalRelayRequest) -> String {
    read_optional_string(&req.raw_body, &["output_format", "format"])
        .unwrap_or_else(|| DEFAULT_IMAGE_OUTPUT_FORMAT.to_string())
}

pub fn seed_from_request(req: &CanonicalRelayRequest) -> String {
    read_optional_string(&req.raw_body, &["seed"]).unwrap_or_default()
}

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

pub fn extract_signed_url_from_event_payload(output_id: &str, payload: &str) -> Option<String> {
    if let Ok(value) = serde_json::from_str::<Value>(payload) {
        if let Some(url) = find_signed_url_in_value(output_id, &value) {
            return Some(url);
        }
    }

    let normalized = normalize_event_string(payload);
    extract_signed_url_from_text(output_id, &normalized)
}

pub fn infer_mime_type_from_url(url: &str) -> String {
    let lower = url.to_lowercase();
    if lower.contains(".mp4") {
        "video/mp4".to_string()
    } else if lower.contains(".webm") {
        "video/webm".to_string()
    } else if lower.contains(".mp3") {
        "audio/mpeg".to_string()
    } else if lower.contains(".wav") {
        "audio/wav".to_string()
    } else if lower.contains(".m4a") {
        "audio/mp4".to_string()
    } else if lower.contains(".ogg") {
        "audio/ogg".to_string()
    } else if lower.contains(".jpg") || lower.contains(".jpeg") {
        "image/jpeg".to_string()
    } else if lower.contains(".webp") || lower.contains("format=webp") {
        "image/webp".to_string()
    } else if lower.contains(".gif") {
        "image/gif".to_string()
    } else {
        "image/png".to_string()
    }
}

pub fn build_openai_images_response_from_url(
    req: &CanonicalRelayRequest,
    prompt: &str,
    signed_url: &str,
) -> Value {
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    json!({
        "created": created,
        "data": [{
            "url": signed_url,
            "revised_prompt": prompt,
        }],
        "n": requested_image_count(req),
    })
}

pub fn build_openai_images_response_from_bytes(
    prompt: &str,
    mime_type: &str,
    bytes: &[u8],
) -> Value {
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    json!({
        "created": created,
        "data": [{
            "b64_json": base64::engine::general_purpose::STANDARD.encode(bytes),
            "revised_prompt": prompt,
            "mime_type": mime_type,
        }],
    })
}

pub fn build_video_generation_response(model: &str, prompt: &str, signed_url: &str) -> Value {
    build_media_generation_response("video.generation", "video", model, prompt, signed_url)
}

pub fn build_audio_generation_response(model: &str, prompt: &str, signed_url: &str) -> Value {
    build_media_generation_response("audio.generation", "audio", model, prompt, signed_url)
}

fn read_optional_string(value: &Value, keys: &[&str]) -> Option<String> {
    let map = value.as_object()?;
    for key in keys {
        if let Some(value) = map.get(*key).and_then(|v| v.as_str()) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn read_optional_hash_string(
    map: &std::collections::HashMap<String, Value>,
    keys: &[&str],
) -> Option<String> {
    for key in keys {
        if let Some(value) = map.get(*key).and_then(|v| v.as_str()) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn read_required_hash_string(
    map: &std::collections::HashMap<String, Value>,
    keys: &[&str],
) -> Result<String, GatewayError> {
    for key in keys {
        if let Some(value) = map.get(*key).and_then(|v| v.as_str()) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed.to_string());
            }
        }
    }

    Err(
        GatewayError::bad_request(format!("Missing required runtime field `{}`.", keys[0]))
            .with_code("missing_lumalabs_runtime_field"),
    )
}

fn is_ratio_string(value: &str) -> bool {
    let mut parts = value.split(':');
    matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some(left), Some(right), None)
            if !left.is_empty()
                && !right.is_empty()
                && left.chars().all(|c| c.is_ascii_digit())
                && right.chars().all(|c| c.is_ascii_digit())
    )
}

fn find_signed_url_in_value(output_id: &str, value: &Value) -> Option<String> {
    match value {
        Value::String(text) => {
            let normalized = normalize_event_string(text);
            if let Some(url) = extract_signed_url_from_text(output_id, &normalized) {
                return Some(url);
            }
            if normalized.starts_with('{') || normalized.starts_with('[') {
                if let Ok(inner) = serde_json::from_str::<Value>(&normalized) {
                    return find_signed_url_in_value(output_id, &inner);
                }
            }
            None
        }
        Value::Array(values) => values
            .iter()
            .find_map(|entry| find_signed_url_in_value(output_id, entry)),
        Value::Object(map) => {
            for key in &[
                "url",
                "src",
                "imageUrl",
                "image_url",
                "downloadUrl",
                "download_url",
                "signedUrl",
                "signed_url",
                "cdnUrl",
                "cdn_url",
            ] {
                if let Some(url) = map
                    .get(*key)
                    .and_then(|entry| find_signed_url_in_value(output_id, entry))
                {
                    return Some(url);
                }
            }

            map.values()
                .find_map(|entry| find_signed_url_in_value(output_id, entry))
        }
        _ => None,
    }
}

fn extract_signed_url_from_text(output_id: &str, text: &str) -> Option<String> {
    static URL_REGEX: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let regex = URL_REGEX.get_or_init(|| {
        Regex::new(r#"https://[^\s"'<>\\]+"#).expect("lumalabs url regex must compile")
    });

    regex
        .find_iter(text)
        .map(|m| cleanup_url_candidate(m.as_str()))
        .find(|candidate| is_signed_output_url(output_id, candidate))
}

fn is_signed_output_url(output_id: &str, candidate: &str) -> bool {
    candidate.contains(output_id)
        && candidate.starts_with("https://")
        && (candidate.contains("cdn.")
            || candidate.contains("Key-Pair-Id=")
            || candidate.contains("Policy="))
}

fn cleanup_url_candidate(candidate: &str) -> String {
    candidate
        .trim_matches(|ch| matches!(ch, '"' | '\'' | ',' | ';' | ')' | ']' | '}'))
        .replace("\\\\u0026", "&")
        .replace("\\\\u003d", "=")
        .replace("\\\\/", "/")
        .replace("\\u0026", "&")
        .replace("\\u003d", "=")
        .replace("\\/", "/")
}

fn normalize_event_string(value: &str) -> String {
    value
        .replace("\\\\u0026", "&")
        .replace("\\\\u003d", "=")
        .replace("\\\\/", "/")
        .replace("\\u0026", "&")
        .replace("\\u003d", "=")
        .replace("\\/", "/")
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

fn build_media_generation_response(
    object: &str,
    kind: &str,
    model: &str,
    prompt: &str,
    signed_url: &str,
) -> Value {
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    json!({
        "object": object,
        "provider": "lumalabs",
        "created": created,
        "completed": true,
        "model": model,
        "prompt": prompt,
        "data": [{
            "url": signed_url,
            "mime_type": infer_mime_type_from_url(signed_url),
            "kind": kind,
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::{
        CanonicalMessage, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
    };
    use crate::upstream::client::UpstreamClient;
    use serde_json::json;
    use std::collections::HashMap;

    fn make_payload(adapter: &str, base_url: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
            adapter: adapter.to_string(),
            base_url: base_url.to_string(),
            api_key: "sk-test".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: None,
            headers: HashMap::new(),
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: None,
            chat_completions_path: None,
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            extra_body: None,
            session_auth: None,
            keepalive: None,
        }
    }

    fn make_request(raw_body: Value) -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ImagesGenerations,
            requested_model: Some(LUMALABS_DEFAULT_MODEL.to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "draw a floating island".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body,
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }

    #[test]
    fn lumalabs_unsupported_request_plan_error_matches_contract() {
        let err = unsupported_request_plan_error();
        assert_eq!(err.http_status, Some(400));
        assert_eq!(err.code.as_deref(), Some("unsupported_lumalabs_endpoint"));
        assert!(err.message.contains("image, video, and audio-generation"));
    }

    #[test]
    fn plan_lumalabs_chat_endpoint_rejected_locally() {
        let payload = make_payload("lumalabs_compatible", "https://app.lumalabs.ai");
        let mut req = make_request(json!({ "prompt": "ignored" }));
        req.endpoint_kind = EndpointKind::ChatCompletions;
        let err = UpstreamClient::build_request_plan(&payload, &req, "uni-1", false)
            .expect_err("lumalabs chat requests should be rejected");
        assert_eq!(err.http_status, Some(400));
        assert_eq!(err.code.as_deref(), Some("unsupported_lumalabs_endpoint"));
    }

    #[test]
    fn aspect_ratio_maps_common_openai_sizes() {
        let req = make_request(json!({ "prompt": "island", "size": "1792x1024" }));
        assert_eq!(aspect_ratio_from_request(&req), "16:9");
    }

    #[test]
    fn action_request_uses_uni_1_shape() {
        let body = build_action_request("hello", "16:9", "", "auto", "png", "abcd1234");
        assert_eq!(body["type"], DEFAULT_IMAGE_ACTION_TYPE);
        assert_eq!(body["fields"]["aspect_ratio"], "16:9");
        assert_eq!(body["optimistic_output_ids"][0], "abcd1234");
    }

    #[test]
    fn extracts_output_id_from_action_response() {
        let output_id = extract_action_output_id(&json!({
            "output_artifacts": {
                "image": ["0k57jd93"]
            }
        }))
        .unwrap();
        assert_eq!(output_id, "0k57jd93");
    }

    #[test]
    fn extracts_signed_url_from_nested_event_json() {
        let payload = json!({
            "artifact": {
                "signedUrl": "https://cdn.prod01.labs.lumalabs.ai/realm/cdn-cgi/0k57jd93.png?Policy=test&Signature=sig&Key-Pair-Id=kp"
            }
        })
        .to_string();
        let url = extract_signed_url_from_event_payload("0k57jd93", &payload).unwrap();
        assert!(url.contains("0k57jd93.png"));
        assert!(url.contains("Policy=test"));
    }

    #[test]
    fn extracts_signed_url_from_escaped_text_payload() {
        let payload = r#"data: {\"url\":\"https:\/\/cdn.prod01.labs.lumalabs.ai\/realm\/cdn-cgi\/0k57jd93.png?Policy=test\\u0026Signature=sig\\u0026Key-Pair-Id=kp\"}"#;
        let url = extract_signed_url_from_event_payload("0k57jd93", payload).unwrap();
        assert!(url.contains("Signature=sig"));
        assert!(url.contains("Key-Pair-Id=kp"));
    }

    #[test]
    fn runtime_reads_realm_id_from_payload_extra_body() {
        let payload = ProviderAccountPayload {
            adapter: "lumalabs_compatible".to_string(),
            base_url: "https://app.lumalabs.ai".to_string(),
            api_key: "wos-session".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: Some(LUMALABS_DEFAULT_MODEL.to_string()),
            headers: HashMap::new(),
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: None,
            chat_completions_path: None,
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            extra_body: Some(
                [(
                    "realmId".to_string(),
                    json!("4675f69b-4aa5-4b25-bfc7-c480d8efd537"),
                )]
                .into_iter()
                .collect(),
            ),
            session_auth: None,
            keepalive: None,
        };

        let runtime = runtime_from_payload(&payload).unwrap();
        assert_eq!(runtime.realm_id, "4675f69b-4aa5-4b25-bfc7-c480d8efd537");
    }

    #[test]
    fn response_format_defaults_to_url() {
        let req = make_request(json!({ "prompt": "hello" }));
        assert!(prefers_url_response(&req).unwrap());
    }

    #[test]
    fn runtime_reads_optional_video_and_audio_contract_fields() {
        let payload = ProviderAccountPayload {
            adapter: "lumalabs_compatible".to_string(),
            base_url: "https://app.lumalabs.ai".to_string(),
            api_key: "wos-session".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: Some(LUMALABS_DEFAULT_IMAGE_MODEL.to_string()),
            headers: HashMap::new(),
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: None,
            chat_completions_path: None,
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            extra_body: Some(
                [
                    (
                        "realmId".to_string(),
                        json!("4675f69b-4aa5-4b25-bfc7-c480d8efd537"),
                    ),
                    ("videoActionType".to_string(), json!("create_video_ray3_14")),
                    ("videoArtifactField".to_string(), json!("video")),
                    (
                        "audioActionType".to_string(),
                        json!("text_to_music_elevenlabs_v1"),
                    ),
                    ("audioArtifactField".to_string(), json!("audio")),
                ]
                .into_iter()
                .collect(),
            ),
            session_auth: None,
            keepalive: None,
        };

        let runtime = runtime_from_payload(&payload).unwrap();
        assert_eq!(
            runtime.video_action_type.as_deref(),
            Some("create_video_ray3_14")
        );
        assert_eq!(runtime.video_artifact_field, "video");
        assert_eq!(
            runtime.audio_action_type.as_deref(),
            Some("text_to_music_elevenlabs_v1")
        );
        assert_eq!(runtime.audio_artifact_field, "audio");
    }

    #[test]
    fn media_operation_from_endpoint_kind_rejects_edits_and_chat() {
        assert_eq!(
            LumalabsMediaOperation::from_endpoint_kind(
                crate::protocol::canonical::EndpointKind::ImagesGenerations
            )
            .unwrap(),
            LumalabsMediaOperation::Image
        );
        assert_eq!(
            LumalabsMediaOperation::from_endpoint_kind(
                crate::protocol::canonical::EndpointKind::VideosGenerations
            )
            .unwrap(),
            LumalabsMediaOperation::Video
        );
        assert_eq!(
            LumalabsMediaOperation::from_endpoint_kind(
                crate::protocol::canonical::EndpointKind::MusicGenerations
            )
            .unwrap(),
            LumalabsMediaOperation::Audio
        );

        let edit_err = LumalabsMediaOperation::from_endpoint_kind(
            crate::protocol::canonical::EndpointKind::ImagesEdits,
        )
        .expect_err("image edits should be rejected");
        assert_eq!(
            edit_err.code.as_deref(),
            Some("unsupported_lumalabs_edit_endpoint")
        );

        let chat_err = LumalabsMediaOperation::from_endpoint_kind(
            crate::protocol::canonical::EndpointKind::ChatCompletions,
        )
        .expect_err("chat should be rejected");
        assert_eq!(
            chat_err.code.as_deref(),
            Some("unsupported_lumalabs_endpoint")
        );
    }

    #[test]
    fn unsupported_output_count_error_is_operation_specific() {
        let image = LumalabsMediaOperation::Image.unsupported_output_count_error();
        assert_eq!(image.http_status, Some(400));
        assert_eq!(
            image.code.as_deref(),
            Some("unsupported_lumalabs_image_count")
        );

        let video = LumalabsMediaOperation::Video.unsupported_output_count_error();
        assert_eq!(video.http_status, Some(400));
        assert_eq!(
            video.code.as_deref(),
            Some("unsupported_lumalabs_video_count")
        );

        let audio = LumalabsMediaOperation::Audio.unsupported_output_count_error();
        assert_eq!(audio.http_status, Some(400));
        assert_eq!(
            audio.code.as_deref(),
            Some("unsupported_lumalabs_audio_count")
        );
    }

    #[test]
    fn unsupported_image_inputs_error_matches_contract() {
        let err = unsupported_image_inputs_error();
        assert_eq!(err.http_status, Some(400));
        assert_eq!(
            err.code.as_deref(),
            Some("unsupported_lumalabs_image_inputs")
        );
        assert_eq!(
            err.message.as_str(),
            "LumaLabs image generation currently supports prompt-only requests and does not accept image uploads or masks."
        );
    }

    #[test]
    fn extract_remote_executor_signed_url_reads_payload_and_rejects_missing_value() {
        let ok = extract_remote_executor_signed_url(&json!({
            "signedUrl": "https://cdn.example.com/out.png?sig=test"
        }))
        .expect("signed url should be extracted");
        assert_eq!(ok, "https://cdn.example.com/out.png?sig=test");

        let err = extract_remote_executor_signed_url(&json!({ "ok": true }))
            .expect_err("missing signedUrl should be rejected");
        assert_eq!(err.http_status, Some(500));
        assert_eq!(
            err.code.as_deref(),
            Some("lumalabs_browser_executor_missing_signed_url")
        );
        assert_eq!(
            err.message.as_str(),
            "Remote LumaLabs browser executor succeeded without a signedUrl."
        );
    }

    #[test]
    fn extract_browser_worker_signed_url_rejects_missing_value() {
        let ok = extract_browser_worker_signed_url(Some(
            "https://cdn.example.com/out.png?sig=test".to_string(),
        ))
        .expect("signed url should be returned");
        assert_eq!(ok, "https://cdn.example.com/out.png?sig=test");

        let err = extract_browser_worker_signed_url(None)
            .expect_err("missing signed url should be rejected");
        assert_eq!(err.http_status, Some(500));
        assert_eq!(
            err.code.as_deref(),
            Some("lumalabs_browser_worker_missing_signed_url")
        );
        assert_eq!(
            err.message.as_str(),
            "LumaLabs browser worker reported success without a signed URL."
        );
    }

    #[test]
    fn empty_browser_worker_output_error_formats_stderr_fallback() {
        let with_stderr = empty_browser_worker_output_error("permission denied");
        assert_eq!(with_stderr.http_status, Some(500));
        assert_eq!(
            with_stderr.provider_name.as_deref(),
            Some("lumalabs_compatible")
        );
        assert_eq!(
            with_stderr.code.as_deref(),
            Some("lumalabs_browser_worker_empty_output")
        );
        assert_eq!(
            with_stderr.message.as_str(),
            "LumaLabs browser worker did not return JSON output. stderr: permission denied"
        );

        let empty = empty_browser_worker_output_error("");
        assert_eq!(
            empty.message.as_str(),
            "LumaLabs browser worker did not return JSON output. stderr: <empty>"
        );
    }

    #[test]
    fn browser_worker_output_parse_error_formats_error_and_stdout() {
        let err = browser_worker_output_parse_error("expected value", "{\"oops\":");
        assert_eq!(err.http_status, Some(500));
        assert_eq!(err.provider_name.as_deref(), Some("lumalabs_compatible"));
        assert_eq!(
            err.code.as_deref(),
            Some("lumalabs_browser_worker_output_parse_failed")
        );
        assert_eq!(
            err.message.as_str(),
            "Failed to parse LumaLabs browser worker output: expected value. stdout: {\"oops\":"
        );
    }

    #[test]
    fn browser_worker_wait_failed_error_formats_cause() {
        let err = browser_worker_wait_failed_error("The pipe has been ended");
        assert_eq!(err.http_status, Some(500));
        assert_eq!(err.provider_name.as_deref(), Some("lumalabs_compatible"));
        assert_eq!(
            err.code.as_deref(),
            Some("lumalabs_browser_worker_wait_failed")
        );
        assert_eq!(
            err.message.as_str(),
            "LumaLabs browser worker failed before producing output: The pipe has been ended"
        );
    }

    #[test]
    fn browser_worker_stdin_error_formats_cause() {
        let err = browser_worker_stdin_error("broken pipe");
        assert_eq!(err.http_status, Some(500));
        assert_eq!(err.provider_name.as_deref(), Some("lumalabs_compatible"));
        assert_eq!(
            err.code.as_deref(),
            Some("lumalabs_browser_worker_stdin_failed")
        );
        assert_eq!(
            err.message.as_str(),
            "Failed to write LumaLabs browser worker input: broken pipe"
        );
    }

    #[test]
    fn browser_worker_spawn_failed_error_formats_script_path_and_cause() {
        let err = browser_worker_spawn_failed_error(
            std::path::Path::new("C:/tmp/lumalabs-browser-worker.mjs"),
            "The system cannot find the file specified. (os error 2)",
        );
        assert_eq!(err.http_status, Some(500));
        assert_eq!(err.provider_name.as_deref(), Some("lumalabs_compatible"));
        assert_eq!(
            err.code.as_deref(),
            Some("lumalabs_browser_worker_spawn_failed")
        );
        assert_eq!(
            err.message.as_str(),
            "Failed to launch LumaLabs browser worker at C:/tmp/lumalabs-browser-worker.mjs: The system cannot find the file specified. (os error 2)"
        );
    }

    #[test]
    fn browser_worker_input_serialize_error_formats_cause() {
        let err = browser_worker_input_serialize_error("missing field `prompt`");
        assert_eq!(err.http_status, Some(500));
        assert_eq!(err.provider_name.as_deref(), Some("lumalabs_compatible"));
        assert_eq!(
            err.code.as_deref(),
            Some("lumalabs_browser_worker_input_serialize_failed")
        );
        assert_eq!(
            err.message.as_str(),
            "Failed to serialize LumaLabs browser worker input: missing field `prompt`"
        );
    }

    #[test]
    fn auto_discovery_is_enabled_only_for_default_action_resolution() {
        let runtime = LumalabsRuntime {
            realm_id: "realm-1".to_string(),
            image_action_type: None,
            video_action_type: None,
            audio_action_type: None,
            image_artifact_field: DEFAULT_IMAGE_ARTIFACT_FIELD.to_string(),
            video_artifact_field: DEFAULT_VIDEO_ARTIFACT_FIELD.to_string(),
            audio_artifact_field: DEFAULT_AUDIO_ARTIFACT_FIELD.to_string(),
        };
        let req = make_request(json!({ "prompt": "hello world" }));

        assert!(should_auto_discover_action_type(
            &req,
            &runtime,
            LumalabsMediaOperation::Video
        ));
        assert!(should_auto_discover_action_type(
            &req,
            &runtime,
            LumalabsMediaOperation::Audio
        ));
    }

    #[test]
    fn auto_discovery_is_disabled_for_runtime_action_override() {
        let runtime = LumalabsRuntime {
            realm_id: "realm-1".to_string(),
            image_action_type: None,
            video_action_type: Some("custom_video_action".to_string()),
            audio_action_type: Some("custom_audio_action".to_string()),
            image_artifact_field: DEFAULT_IMAGE_ARTIFACT_FIELD.to_string(),
            video_artifact_field: DEFAULT_VIDEO_ARTIFACT_FIELD.to_string(),
            audio_artifact_field: DEFAULT_AUDIO_ARTIFACT_FIELD.to_string(),
        };
        let req = make_request(json!({ "prompt": "hello world" }));

        assert!(!should_auto_discover_action_type(
            &req,
            &runtime,
            LumalabsMediaOperation::Video
        ));
        assert!(!should_auto_discover_action_type(
            &req,
            &runtime,
            LumalabsMediaOperation::Audio
        ));
    }

    #[test]
    fn auto_discovery_is_disabled_for_request_action_override() {
        let runtime = LumalabsRuntime {
            realm_id: "realm-1".to_string(),
            image_action_type: None,
            video_action_type: None,
            audio_action_type: None,
            image_artifact_field: DEFAULT_IMAGE_ARTIFACT_FIELD.to_string(),
            video_artifact_field: DEFAULT_VIDEO_ARTIFACT_FIELD.to_string(),
            audio_artifact_field: DEFAULT_AUDIO_ARTIFACT_FIELD.to_string(),
        };
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::VideosGenerations,
            requested_model: Some(LUMALABS_DEFAULT_VIDEO_MODEL.to_string()),
            stream: false,
            messages: vec![],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({
                "prompt": "cinematic dragon flight",
                "action_type": "request_video_override"
            }),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        };

        assert!(!should_auto_discover_action_type(
            &req,
            &runtime,
            LumalabsMediaOperation::Video
        ));
    }

    #[test]
    fn video_action_request_uses_runtime_action_type_and_video_fields() {
        let runtime = LumalabsRuntime {
            realm_id: "realm-1".to_string(),
            image_action_type: None,
            video_action_type: Some("create_video_ray3_14".to_string()),
            audio_action_type: None,
            image_artifact_field: DEFAULT_IMAGE_ARTIFACT_FIELD.to_string(),
            video_artifact_field: DEFAULT_VIDEO_ARTIFACT_FIELD.to_string(),
            audio_artifact_field: DEFAULT_AUDIO_ARTIFACT_FIELD.to_string(),
        };
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::VideosGenerations,
            requested_model: Some(LUMALABS_DEFAULT_VIDEO_MODEL.to_string()),
            stream: false,
            messages: vec![],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({
                "prompt": "cinematic dragon flight",
                "size": "16:9",
                "durationSeconds": 10,
                "resolution": "1080p"
            }),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        };

        let body = build_action_request_for_operation(
            &req,
            &runtime,
            LumalabsMediaOperation::Video,
            LUMALABS_DEFAULT_VIDEO_MODEL,
            "cinematic dragon flight",
            "vid12345",
        );

        assert_eq!(body["type"], "create_video_ray3_14");
        assert!(body["fields"].get("model").is_none());
        assert_eq!(body["fields"]["aspect_ratio"], "16:9");
        assert_eq!(body["fields"]["duration_seconds"], 10);
        assert_eq!(body["fields"]["resolution"], "1080p");
        assert_eq!(body["optimistic_output_ids"][0], "vid12345");
    }

    #[test]
    fn audio_action_request_supports_lyrics_and_duration() {
        let runtime = LumalabsRuntime {
            realm_id: "realm-1".to_string(),
            image_action_type: None,
            video_action_type: None,
            audio_action_type: Some("text_to_music_elevenlabs_v1".to_string()),
            image_artifact_field: DEFAULT_IMAGE_ARTIFACT_FIELD.to_string(),
            video_artifact_field: DEFAULT_VIDEO_ARTIFACT_FIELD.to_string(),
            audio_artifact_field: DEFAULT_AUDIO_ARTIFACT_FIELD.to_string(),
        };
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::MusicGenerations,
            requested_model: Some(LUMALABS_DEFAULT_AUDIO_MODEL.to_string()),
            stream: false,
            messages: vec![],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({
                "lyrics": "silver rain over neon streets",
                "duration": 24,
                "voice": "alto"
            }),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        };

        let prompt =
            prompt_from_request_for_operation(&req, LumalabsMediaOperation::Audio).unwrap();
        assert_eq!(prompt, "silver rain over neon streets");
        let body = build_action_request_for_operation(
            &req,
            &runtime,
            LumalabsMediaOperation::Audio,
            LUMALABS_DEFAULT_AUDIO_MODEL,
            &prompt,
            "aud12345",
        );

        assert_eq!(body["type"], "text_to_music_elevenlabs_v1");
        assert_eq!(body["fields"]["lyrics"], "silver rain over neon streets");
        assert!(body["fields"].get("voice").is_none());
        assert_eq!(body["fields"]["duration_seconds"], 24);
        assert_eq!(body["fields"]["output_format"], "mp3");
    }

    #[test]
    fn extracts_video_output_id_from_configured_artifact_field() {
        let output_id = extract_action_output_id_for_field(
            &json!({
                "output_artifacts": {
                    "video": ["ray-video-123"]
                }
            }),
            "video",
        )
        .unwrap();
        assert_eq!(output_id, "ray-video-123");
    }

    #[test]
    fn video_generation_response_uses_video_object_shape() {
        let response = build_video_generation_response(
            "ray-2",
            "cinematic skyline at dusk",
            "https://cdn.prod01.labs.lumalabs.ai/output/clip.mp4",
        );
        assert_eq!(response["object"], "video.generation");
        assert_eq!(response["data"][0]["kind"], "video");
        assert_eq!(response["data"][0]["mime_type"], "video/mp4");
    }

    #[test]
    fn audio_generation_response_uses_audio_object_shape() {
        let response = build_audio_generation_response(
            "music-v1",
            "warm lo-fi rain ambience",
            "https://cdn.prod01.labs.lumalabs.ai/output/track.mp3",
        );
        assert_eq!(response["object"], "audio.generation");
        assert_eq!(response["data"][0]["kind"], "audio");
        assert_eq!(response["data"][0]["mime_type"], "audio/mpeg");
    }
}
