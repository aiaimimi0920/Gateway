use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::error::GatewayError;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, EndpointKind, MessageRole, ProtocolFamily,
};

pub const SUNO_DEFAULT_MODEL: &str = "chirp-v3-5";
pub const SUNO_LEGACY_MODEL: &str = "chirp-v3-0";
pub const SUNO_DEFAULT_UPSTREAM_MODEL: &str = "chirp-auk-turbo";
pub const SUNO_LEGACY_UPSTREAM_MODEL: &str = "chirp-auk";

const DEFAULT_WAIT_TIMEOUT_SECS: u64 = 150;
const DEFAULT_POLL_INTERVAL_MS: u64 = 3_000;
const DEFAULT_IMAGE_MIME_TYPE: &str = "image/png";
const DEFAULT_VIDEO_MIME_TYPE: &str = "video/mp4";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SunoClip {
    pub id: String,
    pub title: Option<String>,
    pub image_url: Option<String>,
    pub lyric: Option<String>,
    pub audio_url: Option<String>,
    pub video_url: Option<String>,
    pub created_at: Option<String>,
    pub model_name: Option<String>,
    pub prompt: Option<String>,
    pub gpt_description_prompt: Option<String>,
    pub status: String,
    pub clip_type: Option<String>,
    pub tags: Option<String>,
    pub negative_tags: Option<String>,
    pub duration: Option<String>,
    pub error_message: Option<String>,
}

pub fn normalize_music_generations(body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    let body_obj = body.as_object().ok_or_else(|| {
        GatewayError::bad_request("Music generation request body must be a JSON object.")
    })?;

    if body_obj
        .get("stream")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
    {
        return Err(GatewayError::bad_request(
            "Music generation endpoints do not support `stream: true`.",
        )
        .with_code("music_streaming_not_supported"));
    }

    let prompt = prompt_from_request_body(body_obj).ok_or_else(|| {
        GatewayError::bad_request(
            "Suno music requests require one of: prompt, input, lyrics, or parts[].content.",
        )
        .with_code("missing_music_prompt")
    })?;

    let requested_model = body_obj
        .get("model")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| Some(SUNO_DEFAULT_MODEL.to_string()));

    let explicit_session_key =
        read_optional_string(body_obj, &["user", "session_key", "sessionKey"]);

    Ok(CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::MusicGenerations,
        requested_model,
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![crate::protocol::canonical::ContentPart::Text { text: prompt }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: body,
        previous_response_id: None,
        explicit_session_key,
        extra: std::collections::HashMap::new(),
    })
}

pub fn resolve_model(model: &str) -> Result<&str, GatewayError> {
    let trimmed = model.trim();
    if trimmed.is_empty() {
        return Err(
            GatewayError::bad_request("Suno model must be a non-empty string.")
                .with_code("invalid_suno_model"),
        );
    }
    match trimmed {
        SUNO_DEFAULT_MODEL => Ok(SUNO_DEFAULT_UPSTREAM_MODEL),
        SUNO_LEGACY_MODEL => Ok(SUNO_LEGACY_UPSTREAM_MODEL),
        SUNO_DEFAULT_UPSTREAM_MODEL | SUNO_LEGACY_UPSTREAM_MODEL => Ok(trimmed),
        _ => Err(
            GatewayError::bad_request(format!("Unsupported Suno model '{trimmed}'."))
                .with_code("invalid_suno_model"),
        ),
    }
}

pub fn prompt_from_request(req: &CanonicalRelayRequest) -> Result<String, GatewayError> {
    let body_obj = req
        .raw_body
        .as_object()
        .ok_or_else(|| GatewayError::bad_request("Suno request body must be a JSON object."))?;
    prompt_from_request_body(body_obj).ok_or_else(|| {
        GatewayError::bad_request(
            "Suno media requests require one of: prompt, input, lyrics, or parts[].content.",
        )
        .with_code("missing_music_prompt")
    })
}

pub fn wait_audio(req: &CanonicalRelayRequest) -> bool {
    wait_for_completion(req)
}

pub fn wait_for_completion(req: &CanonicalRelayRequest) -> bool {
    req.raw_body
        .get("wait_completion")
        .or_else(|| req.raw_body.get("waitCompletion"))
        .or_else(|| req.raw_body.get("wait_audio"))
        .or_else(|| req.raw_body.get("waitAudio"))
        .and_then(|value| value.as_bool())
        .unwrap_or(true)
}

pub fn requested_output_count(req: &CanonicalRelayRequest) -> usize {
    req.raw_body
        .get("n")
        .and_then(|value| value.as_u64())
        .map(|value| value.max(1) as usize)
        .unwrap_or(1)
}

pub fn prefers_url_response(req: &CanonicalRelayRequest) -> Result<bool, GatewayError> {
    let Some(value) = req.raw_body.get("response_format") else {
        return Ok(true);
    };
    let Some(format) = value.as_str() else {
        return Err(
            GatewayError::bad_request("response_format must be a string when provided.")
                .with_code("invalid_suno_image_response_format"),
        );
    };
    match format {
        "url" => Ok(true),
        "b64_json" => Ok(false),
        _ => Err(GatewayError::bad_request(
            "Suno image endpoints currently support response_format=url or b64_json.",
        )
        .with_code("unsupported_suno_image_response_format")),
    }
}

pub fn wait_timeout_secs(req: &CanonicalRelayRequest) -> u64 {
    let Some(body_obj) = req.raw_body.as_object() else {
        return DEFAULT_WAIT_TIMEOUT_SECS;
    };
    read_optional_u64(
        body_obj,
        &[
            "wait_timeout_secs",
            "waitTimeoutSecs",
            "timeout_secs",
            "timeoutSecs",
        ],
    )
    .map(|value| value.clamp(10, 600))
    .unwrap_or(DEFAULT_WAIT_TIMEOUT_SECS)
}

pub fn poll_interval_ms(req: &CanonicalRelayRequest) -> u64 {
    let Some(body_obj) = req.raw_body.as_object() else {
        return DEFAULT_POLL_INTERVAL_MS;
    };
    read_optional_u64(
        body_obj,
        &[
            "poll_interval_ms",
            "pollIntervalMs",
            "poll_secs",
            "pollSecs",
        ],
    )
    .map(|value| {
        if value <= 30 {
            (value * 1000).clamp(1_000, 10_000)
        } else {
            value.clamp(1_000, 10_000)
        }
    })
    .unwrap_or(DEFAULT_POLL_INTERVAL_MS)
}

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

pub fn build_feed_poll_request(clip_ids: &[String]) -> Value {
    json!({
        "filters": {
            "ids": {
                "presence": "True",
                "clipIds": clip_ids,
            }
        },
        "limit": clip_ids.len(),
    })
}

pub fn build_browser_token_header_value(timestamp_ms: i64) -> String {
    let encoded = base64::engine::general_purpose::STANDARD
        .encode(json!({ "timestamp": timestamp_ms }).to_string().as_bytes());
    json!({
        "token": encoded,
    })
    .to_string()
}

pub fn extract_clip_ids(body: &Value) -> Result<Vec<String>, GatewayError> {
    let clips = body
        .get("clips")
        .and_then(|value| value.as_array())
        .ok_or_else(|| {
            GatewayError::server_error("Suno generation response missing clips array.")
                .with_provider("suno_compatible")
                .with_code("suno_missing_generation_clips")
        })?;

    let ids = clips
        .iter()
        .filter_map(|clip| clip.get("id").and_then(|value| value.as_str()))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();

    if ids.is_empty() {
        return Err(GatewayError::server_error(
            "Suno generation response clips array did not contain ids.",
        )
        .with_provider("suno_compatible")
        .with_code("suno_missing_clip_ids"));
    }

    Ok(ids)
}

pub fn extract_clips_from_feed(body: &Value) -> Result<Vec<SunoClip>, GatewayError> {
    let clips = body
        .get("clips")
        .and_then(|value| value.as_array())
        .ok_or_else(|| {
            GatewayError::server_error("Suno feed response missing clips array.")
                .with_provider("suno_compatible")
                .with_code("suno_missing_feed_clips")
        })?;

    let parsed = clips.iter().filter_map(parse_clip).collect::<Vec<_>>();

    if parsed.is_empty() {
        return Err(GatewayError::server_error(
            "Suno feed response did not contain any valid clips.",
        )
        .with_provider("suno_compatible")
        .with_code("suno_invalid_feed_clips"));
    }

    Ok(parsed)
}

pub fn clips_ready(clips: &[SunoClip]) -> bool {
    clips.iter().all(|clip| {
        matches!(
            clip.status.as_str(),
            "streaming" | "complete" | "completed" | "error" | "failed"
        )
    })
}

pub fn build_music_generation_response(
    model: &str,
    prompt: &str,
    clips: &[SunoClip],
    completed: bool,
    message: Option<&str>,
) -> Value {
    json!({
        "object": "music.generation",
        "provider": "suno",
        "created": current_unix_timestamp(),
        "completed": completed,
        "model": model,
        "prompt": prompt,
        "message": message.filter(|value| !value.trim().is_empty()),
        "data": clips,
    })
}

pub fn image_urls_for_requested_count(
    req: &CanonicalRelayRequest,
    clips: &[SunoClip],
) -> Result<Vec<String>, GatewayError> {
    let urls = clips
        .iter()
        .filter_map(|clip| clip.image_url.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .take(requested_output_count(req))
        .map(str::to_string)
        .collect::<Vec<_>>();
    if urls.is_empty() {
        return Err(
            GatewayError::server_error("Suno clips did not contain any image output.")
                .with_provider("suno_compatible")
                .with_code("suno_missing_image_asset"),
        );
    }
    Ok(urls)
}

pub fn build_openai_images_response_from_urls(
    req: &CanonicalRelayRequest,
    prompt: &str,
    clips: &[SunoClip],
) -> Result<Value, GatewayError> {
    let urls = image_urls_for_requested_count(req, clips)?;
    Ok(json!({
        "created": current_unix_timestamp(),
        "data": urls.into_iter().map(|url| json!({
            "url": url,
            "revised_prompt": prompt,
            "mime_type": infer_mime_type_from_url(&url, DEFAULT_IMAGE_MIME_TYPE),
        })).collect::<Vec<_>>(),
        "n": requested_output_count(req),
    }))
}

pub fn build_openai_images_response_from_bytes(
    req: &CanonicalRelayRequest,
    prompt: &str,
    images: &[(String, Vec<u8>)],
) -> Result<Value, GatewayError> {
    let data = images
        .iter()
        .take(requested_output_count(req))
        .map(|(mime_type, bytes)| {
            json!({
                "b64_json": base64::engine::general_purpose::STANDARD.encode(bytes),
                "revised_prompt": prompt,
                "mime_type": mime_type,
            })
        })
        .collect::<Vec<_>>();
    if data.is_empty() {
        return Err(
            GatewayError::server_error("Suno image download backfill returned no bytes.")
                .with_provider("suno_compatible")
                .with_code("suno_missing_image_bytes"),
        );
    }
    Ok(json!({
        "created": current_unix_timestamp(),
        "data": data,
        "n": requested_output_count(req),
    }))
}

pub fn unsupported_media_endpoint_error() -> GatewayError {
    GatewayError::bad_request("Suno response finalizer received an unsupported endpoint.")
        .with_provider("suno_compatible")
        .with_code("unsupported_suno_endpoint")
}

pub fn missing_browser_worker_result_error() -> GatewayError {
    GatewayError::server_error("Suno browser worker reported success without a result payload.")
        .with_provider("suno_compatible")
        .with_code("suno_browser_worker_missing_result")
}

pub fn missing_browser_worker_clips_error() -> GatewayError {
    GatewayError::server_error("Suno browser worker completed without returning any clips.")
        .with_provider("suno_compatible")
        .with_code("suno_missing_feed_clips")
}

pub fn empty_browser_worker_output_error(stderr: &str) -> GatewayError {
    GatewayError::server_error(format!(
        "Suno browser worker did not return JSON output. stderr: {}",
        if stderr.is_empty() { "<empty>" } else { stderr }
    ))
    .with_provider("suno_compatible")
    .with_code("suno_browser_worker_empty_output")
}

pub fn browser_worker_output_parse_error(error: &str, stdout: &str) -> GatewayError {
    GatewayError::server_error(format!(
        "Failed to parse Suno browser worker output: {error}. stdout: {stdout}"
    ))
    .with_provider("suno_compatible")
    .with_code("suno_browser_worker_output_parse_failed")
}

pub fn browser_worker_wait_failed_error(error: &str) -> GatewayError {
    GatewayError::server_error(format!(
        "Suno browser worker failed before producing output: {error}"
    ))
    .with_provider("suno_compatible")
    .with_code("suno_browser_worker_wait_failed")
}

pub fn browser_worker_timeout_error() -> GatewayError {
    GatewayError::server_error("Suno browser worker timed out before producing output.")
        .with_provider("suno_compatible")
        .with_code("suno_browser_worker_timeout")
}

pub fn browser_worker_stdin_error(error: &str) -> GatewayError {
    GatewayError::server_error(format!(
        "Failed to write Suno browser worker input: {error}"
    ))
    .with_provider("suno_compatible")
    .with_code("suno_browser_worker_stdin_failed")
}

pub fn browser_worker_spawn_failed_error(
    script_path: &std::path::Path,
    error: &str,
) -> GatewayError {
    GatewayError::server_error(format!(
        "Failed to launch Suno browser worker at {}: {error}",
        script_path.display()
    ))
    .with_provider("suno_compatible")
    .with_code("suno_browser_worker_spawn_failed")
}

pub fn browser_worker_input_serialize_error(error: &str) -> GatewayError {
    GatewayError::server_error(format!(
        "Failed to serialize Suno browser worker input: {error}"
    ))
    .with_provider("suno_compatible")
    .with_code("suno_browser_worker_input_serialize_failed")
}

pub fn remote_browser_worker_result_parse_error(error: &str) -> GatewayError {
    GatewayError::server_error(format!(
        "Failed to parse remote Suno browser worker result: {error}"
    ))
    .with_provider("suno_compatible")
    .with_code("suno_remote_result_parse_failed")
}

pub fn build_video_generation_response(
    model: &str,
    prompt: &str,
    clips: &[SunoClip],
    completed: bool,
    message: Option<&str>,
) -> Result<Value, GatewayError> {
    let clip = first_clip_with_media_url(clips, |candidate| candidate.video_url.as_deref())
        .ok_or_else(|| {
            GatewayError::server_error("Suno clips did not contain any video output.")
                .with_provider("suno_compatible")
                .with_code("suno_missing_video_asset")
        })?;
    let video_url = clip.video_url.as_deref().unwrap_or_default();
    Ok(json!({
        "object": "video.generation",
        "provider": "suno",
        "created": current_unix_timestamp(),
        "completed": completed,
        "model": model,
        "prompt": prompt,
        "message": message.filter(|value| !value.trim().is_empty()),
        "data": [serialize_media_asset(
            clip,
            "video",
            video_url,
            &infer_mime_type_from_url(video_url, DEFAULT_VIDEO_MIME_TYPE),
        )],
    }))
}

pub fn infer_mime_type_from_url(url: &str, fallback: &str) -> String {
    let normalized = url
        .split('?')
        .next()
        .unwrap_or(url)
        .trim()
        .to_ascii_lowercase();
    if normalized.ends_with(".jpg") || normalized.ends_with(".jpeg") {
        return "image/jpeg".to_string();
    }
    if normalized.ends_with(".webp") {
        return "image/webp".to_string();
    }
    if normalized.ends_with(".gif") {
        return "image/gif".to_string();
    }
    if normalized.ends_with(".png") {
        return "image/png".to_string();
    }
    if normalized.ends_with(".mp4") {
        return "video/mp4".to_string();
    }
    if normalized.ends_with(".webm") {
        return "video/webm".to_string();
    }
    fallback.to_string()
}

fn parse_clip(value: &Value) -> Option<SunoClip> {
    let metadata = value.get("metadata");
    let id = value.get("id")?.as_str()?.trim().to_string();
    if id.is_empty() {
        return None;
    }
    let status = value
        .get("status")
        .and_then(|value| value.as_str())
        .unwrap_or("pending")
        .trim()
        .to_string();
    Some(SunoClip {
        id,
        title: read_value_string(value, &["title"]),
        image_url: read_value_string(value, &["image_url", "imageUrl"]),
        lyric: metadata.and_then(|value| read_value_string(value, &["prompt"])),
        audio_url: read_value_string(value, &["audio_url", "audioUrl"]),
        video_url: read_value_string(value, &["video_url", "videoUrl"]),
        created_at: read_value_string(value, &["created_at", "createdAt"]),
        model_name: read_value_string(value, &["model_name", "modelName"]),
        prompt: metadata.and_then(|value| read_value_string(value, &["prompt"])),
        gpt_description_prompt: metadata.and_then(|value| {
            read_value_string(value, &["gpt_description_prompt", "gptDescriptionPrompt"])
        }),
        status,
        clip_type: metadata.and_then(|value| read_value_string(value, &["type"])),
        tags: metadata.and_then(|value| read_value_string(value, &["tags"])),
        negative_tags: metadata
            .and_then(|value| read_value_string(value, &["negative_tags", "negativeTags"])),
        duration: metadata.and_then(|value| read_value_string(value, &["duration"])),
        error_message: metadata
            .and_then(|value| read_value_string(value, &["error_message", "errorMessage"])),
    })
}

fn first_clip_with_media_url<'a, F>(clips: &'a [SunoClip], picker: F) -> Option<&'a SunoClip>
where
    F: Fn(&'a SunoClip) -> Option<&'a str>,
{
    clips.iter().find(|clip| {
        picker(clip)
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
    })
}

fn serialize_media_asset(clip: &SunoClip, kind: &str, url: &str, mime_type: &str) -> Value {
    json!({
        "id": clip.id,
        "kind": kind,
        "url": url,
        "mime_type": mime_type,
        "title": clip.title,
        "status": clip.status,
        "image_url": clip.image_url,
        "audio_url": clip.audio_url,
        "video_url": clip.video_url,
        "created_at": clip.created_at,
        "model_name": clip.model_name,
        "prompt": clip.prompt,
        "lyric": clip.lyric,
        "tags": clip.tags,
        "negative_tags": clip.negative_tags,
        "duration": clip.duration,
        "error_message": clip.error_message,
    })
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

fn prompt_from_request_body(body_obj: &Map<String, Value>) -> Option<String> {
    if let Some(value) = read_optional_string(body_obj, &["lyrics", "prompt", "input"]) {
        return Some(value);
    }

    let parts = body_obj.get("parts")?.as_array()?;
    let combined = parts
        .iter()
        .filter_map(|part| part.get("content").and_then(|value| value.as_str()))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    if combined.is_empty() {
        None
    } else {
        Some(combined)
    }
}

fn read_value_string(value: &Value, aliases: &[&str]) -> Option<String> {
    let obj = value.as_object()?;
    read_optional_string(obj, aliases)
}

fn read_optional_string(obj: &Map<String, Value>, aliases: &[&str]) -> Option<String> {
    aliases
        .iter()
        .find_map(|key| obj.get(*key).and_then(|value| value.as_str()))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn read_optional_u64(obj: &Map<String, Value>, aliases: &[&str]) -> Option<u64> {
    aliases.iter().find_map(|key| {
        obj.get(*key).and_then(|value| match value {
            Value::Number(number) => number.as_u64(),
            Value::String(text) => text.trim().parse::<u64>().ok(),
            _ => None,
        })
    })
}

fn read_optional_i64(obj: &Map<String, Value>, aliases: &[&str]) -> Option<i64> {
    aliases.iter().find_map(|key| {
        obj.get(*key).and_then(|value| match value {
            Value::Number(number) => number.as_i64(),
            Value::String(text) => text.trim().parse::<i64>().ok(),
            _ => None,
        })
    })
}

fn read_optional_bool(obj: &Map<String, Value>, aliases: &[&str]) -> Option<bool> {
    aliases.iter().find_map(|key| {
        obj.get(*key).and_then(|value| match value {
            Value::Bool(flag) => Some(*flag),
            Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" => Some(true),
                "false" | "0" | "no" => Some(false),
                _ => None,
            },
            _ => None,
        })
    })
}

fn current_unix_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_music_generations_defaults_model() {
        let request = normalize_music_generations(json!({
            "prompt": "warm synthwave duet"
        }))
        .unwrap();
        assert_eq!(request.endpoint_kind, EndpointKind::MusicGenerations);
        assert_eq!(request.requested_model.as_deref(), Some(SUNO_DEFAULT_MODEL));
    }

    #[test]
    fn build_generate_request_uses_description_mode_by_default() {
        let request = normalize_music_generations(json!({
            "prompt": "warm synthwave duet"
        }))
        .unwrap();
        let body = build_generate_request(&request, SUNO_DEFAULT_MODEL, None).unwrap();
        assert_eq!(body["gpt_description_prompt"], "warm synthwave duet");
        assert_eq!(body["prompt"], "");
        assert_eq!(body["mv"], SUNO_DEFAULT_UPSTREAM_MODEL);
        assert_eq!(body["metadata"]["create_mode"], "simple");
        assert_eq!(body["override_fields"], json!([]));
        assert!(body["transaction_uuid"].as_str().is_some());
    }

    #[test]
    fn build_generate_request_uses_custom_mode_for_lyrics() {
        let request = normalize_music_generations(json!({
            "lyrics": "hello from the bridge",
            "tags": "lofi",
            "title": "Bridge"
        }))
        .unwrap();
        let body = build_generate_request(&request, SUNO_DEFAULT_MODEL, Some("tier-123")).unwrap();
        assert_eq!(body["prompt"], "hello from the bridge");
        assert_eq!(body["gpt_description_prompt"], "");
        assert_eq!(body["tags"], "lofi");
        assert_eq!(body["metadata"]["create_mode"], "custom");
        assert_eq!(body["metadata"]["user_tier"], "tier-123");
    }

    #[test]
    fn response_format_defaults_to_url_for_images() {
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ImagesGenerations,
            requested_model: Some(SUNO_DEFAULT_MODEL.to_string()),
            stream: false,
            messages: vec![],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({"prompt": "cover art"}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: std::collections::HashMap::new(),
        };
        assert!(prefers_url_response(&req).unwrap());
    }

    #[test]
    fn build_openai_images_response_from_urls_uses_clip_images() {
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ImagesGenerations,
            requested_model: Some(SUNO_DEFAULT_MODEL.to_string()),
            stream: false,
            messages: vec![],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({
                "prompt": "cover art",
                "n": 1,
                "response_format": "url"
            }),
            previous_response_id: None,
            explicit_session_key: None,
            extra: std::collections::HashMap::new(),
        };
        let response = build_openai_images_response_from_urls(
            &req,
            "cover art",
            &[SunoClip {
                id: "clip-1".to_string(),
                title: Some("Cover".to_string()),
                image_url: Some("https://cdn.example.com/cover.webp".to_string()),
                lyric: None,
                audio_url: None,
                video_url: None,
                created_at: None,
                model_name: None,
                prompt: None,
                gpt_description_prompt: None,
                status: "complete".to_string(),
                clip_type: None,
                tags: None,
                negative_tags: None,
                duration: None,
                error_message: None,
            }],
        )
        .unwrap();
        assert_eq!(
            response["data"][0]["url"].as_str(),
            Some("https://cdn.example.com/cover.webp")
        );
        assert_eq!(
            response["data"][0]["mime_type"].as_str(),
            Some("image/webp")
        );
    }

    #[test]
    fn build_video_generation_response_uses_platform_media_shape() {
        let response = build_video_generation_response(
            SUNO_DEFAULT_MODEL,
            "cinematic performance clip",
            &[SunoClip {
                id: "clip-1".to_string(),
                title: Some("Performance".to_string()),
                image_url: Some("https://cdn.example.com/cover.png".to_string()),
                lyric: Some("silver rain".to_string()),
                audio_url: Some("https://cdn.example.com/track.mp3".to_string()),
                video_url: Some("https://cdn.example.com/track.mp4".to_string()),
                created_at: Some("2026-04-21T00:00:00Z".to_string()),
                model_name: Some(SUNO_DEFAULT_UPSTREAM_MODEL.to_string()),
                prompt: Some("cinematic performance clip".to_string()),
                gpt_description_prompt: None,
                status: "complete".to_string(),
                clip_type: None,
                tags: Some("pop".to_string()),
                negative_tags: None,
                duration: Some("120".to_string()),
                error_message: None,
            }],
            true,
            None,
        )
        .unwrap();
        assert_eq!(response["object"], "video.generation");
        assert_eq!(response["data"][0]["kind"], "video");
        assert_eq!(response["data"][0]["mime_type"], "video/mp4");
        assert_eq!(
            response["data"][0]["url"].as_str(),
            Some("https://cdn.example.com/track.mp4")
        );
    }

    #[test]
    fn unsupported_media_endpoint_error_matches_contract() {
        let err = unsupported_media_endpoint_error();
        assert_eq!(err.http_status, Some(400));
        assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
        assert_eq!(err.code.as_deref(), Some("unsupported_suno_endpoint"));
    }

    #[test]
    fn missing_browser_worker_result_error_matches_contract() {
        let err = missing_browser_worker_result_error();
        assert_eq!(err.http_status, Some(500));
        assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
        assert_eq!(
            err.code.as_deref(),
            Some("suno_browser_worker_missing_result")
        );
        assert_eq!(
            err.message.as_str(),
            "Suno browser worker reported success without a result payload."
        );
    }

    #[test]
    fn missing_browser_worker_clips_error_matches_contract() {
        let err = missing_browser_worker_clips_error();
        assert_eq!(err.http_status, Some(500));
        assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
        assert_eq!(err.code.as_deref(), Some("suno_missing_feed_clips"));
        assert_eq!(
            err.message.as_str(),
            "Suno browser worker completed without returning any clips."
        );
    }

    #[test]
    fn empty_browser_worker_output_error_formats_stderr_fallback() {
        let with_stderr = empty_browser_worker_output_error("permission denied");
        assert_eq!(with_stderr.http_status, Some(500));
        assert_eq!(
            with_stderr.provider_name.as_deref(),
            Some("suno_compatible")
        );
        assert_eq!(
            with_stderr.code.as_deref(),
            Some("suno_browser_worker_empty_output")
        );
        assert_eq!(
            with_stderr.message.as_str(),
            "Suno browser worker did not return JSON output. stderr: permission denied"
        );

        let empty = empty_browser_worker_output_error("");
        assert_eq!(
            empty.message.as_str(),
            "Suno browser worker did not return JSON output. stderr: <empty>"
        );
    }

    #[test]
    fn browser_worker_output_parse_error_formats_error_and_stdout() {
        let err = browser_worker_output_parse_error("expected value", "{\"oops\":");
        assert_eq!(err.http_status, Some(500));
        assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
        assert_eq!(
            err.code.as_deref(),
            Some("suno_browser_worker_output_parse_failed")
        );
        assert_eq!(
            err.message.as_str(),
            "Failed to parse Suno browser worker output: expected value. stdout: {\"oops\":"
        );
    }

    #[test]
    fn browser_worker_wait_failed_error_formats_cause() {
        let err = browser_worker_wait_failed_error("The pipe has been ended");
        assert_eq!(err.http_status, Some(500));
        assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
        assert_eq!(err.code.as_deref(), Some("suno_browser_worker_wait_failed"));
        assert_eq!(
            err.message.as_str(),
            "Suno browser worker failed before producing output: The pipe has been ended"
        );
    }

    #[test]
    fn browser_worker_timeout_error_matches_contract() {
        let err = browser_worker_timeout_error();
        assert_eq!(err.http_status, Some(500));
        assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
        assert_eq!(err.code.as_deref(), Some("suno_browser_worker_timeout"));
        assert_eq!(
            err.message.as_str(),
            "Suno browser worker timed out before producing output."
        );
    }

    #[test]
    fn browser_worker_stdin_error_formats_cause() {
        let err = browser_worker_stdin_error("broken pipe");
        assert_eq!(err.http_status, Some(500));
        assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
        assert_eq!(
            err.code.as_deref(),
            Some("suno_browser_worker_stdin_failed")
        );
        assert_eq!(
            err.message.as_str(),
            "Failed to write Suno browser worker input: broken pipe"
        );
    }

    #[test]
    fn browser_worker_spawn_failed_error_formats_script_path_and_cause() {
        let err = browser_worker_spawn_failed_error(
            std::path::Path::new("C:/tmp/suno-browser-worker.mjs"),
            "The system cannot find the file specified. (os error 2)",
        );
        assert_eq!(err.http_status, Some(500));
        assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
        assert_eq!(
            err.code.as_deref(),
            Some("suno_browser_worker_spawn_failed")
        );
        assert_eq!(
            err.message.as_str(),
            "Failed to launch Suno browser worker at C:/tmp/suno-browser-worker.mjs: The system cannot find the file specified. (os error 2)"
        );
    }

    #[test]
    fn browser_worker_input_serialize_error_formats_cause() {
        let err = browser_worker_input_serialize_error("missing field `prompt`");
        assert_eq!(err.http_status, Some(500));
        assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
        assert_eq!(
            err.code.as_deref(),
            Some("suno_browser_worker_input_serialize_failed")
        );
        assert_eq!(
            err.message.as_str(),
            "Failed to serialize Suno browser worker input: missing field `prompt`"
        );
    }

    #[test]
    fn remote_browser_worker_result_parse_error_formats_cause() {
        let err = remote_browser_worker_result_parse_error("expected value");
        assert_eq!(err.http_status, Some(500));
        assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
        assert_eq!(err.code.as_deref(), Some("suno_remote_result_parse_failed"));
        assert_eq!(
            err.message.as_str(),
            "Failed to parse remote Suno browser worker result: expected value"
        );
    }

    #[test]
    fn resolve_model_maps_public_models_to_live_web_mv_values() {
        assert_eq!(
            resolve_model(SUNO_DEFAULT_MODEL).unwrap(),
            SUNO_DEFAULT_UPSTREAM_MODEL
        );
        assert_eq!(
            resolve_model(SUNO_LEGACY_MODEL).unwrap(),
            SUNO_LEGACY_UPSTREAM_MODEL
        );
        assert_eq!(
            resolve_model(SUNO_DEFAULT_UPSTREAM_MODEL).unwrap(),
            SUNO_DEFAULT_UPSTREAM_MODEL
        );
    }

    #[test]
    fn build_feed_poll_request_uses_feed_v3_shape() {
        let body = build_feed_poll_request(&["clip-a".to_string(), "clip-b".to_string()]);
        assert_eq!(body["filters"]["ids"]["presence"], "True");
        assert_eq!(
            body["filters"]["ids"]["clipIds"],
            json!(["clip-a", "clip-b"])
        );
        assert_eq!(body["limit"], 2);
    }

    #[test]
    fn browser_token_header_value_matches_live_shape() {
        let value = build_browser_token_header_value(1_775_875_135_731);
        assert_eq!(
            value,
            "{\"token\":\"eyJ0aW1lc3RhbXAiOjE3NzU4NzUxMzU3MzF9\"}"
        );
    }

    #[test]
    fn extract_clips_from_feed_maps_audio_fields() {
        let clips = extract_clips_from_feed(&json!({
            "clips": [{
                "id": "clip_1",
                "status": "streaming",
                "audio_url": "https://cdn.example/audio.mp3",
                "video_url": "https://cdn.example/video.mp4",
                "metadata": {
                    "prompt": "hello",
                    "tags": "pop"
                }
            }]
        }))
        .unwrap();
        assert_eq!(clips.len(), 1);
        assert_eq!(
            clips[0].audio_url.as_deref(),
            Some("https://cdn.example/audio.mp3")
        );
        assert_eq!(clips[0].tags.as_deref(), Some("pop"));
    }

    #[test]
    fn clips_ready_requires_terminal_states() {
        assert!(clips_ready(&[SunoClip {
            id: "clip".to_string(),
            title: None,
            image_url: None,
            lyric: None,
            audio_url: None,
            video_url: None,
            created_at: None,
            model_name: None,
            prompt: None,
            gpt_description_prompt: None,
            status: "complete".to_string(),
            clip_type: None,
            tags: None,
            negative_tags: None,
            duration: None,
            error_message: None,
        }]));
        assert!(!clips_ready(&[SunoClip {
            id: "clip".to_string(),
            title: None,
            image_url: None,
            lyric: None,
            audio_url: None,
            video_url: None,
            created_at: None,
            model_name: None,
            prompt: None,
            gpt_description_prompt: None,
            status: "submitted".to_string(),
            clip_type: None,
            tags: None,
            negative_tags: None,
            duration: None,
            error_message: None,
        }]));
    }
}
