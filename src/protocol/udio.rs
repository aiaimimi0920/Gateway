use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Number, Value};

use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind};

pub const UDIO_DEFAULT_MODEL: &str = "udio-music";
const UDIO_DEFAULT_MODEL_TYPE: &str = "udio32-v1.5";

const DEFAULT_WAIT_TIMEOUT_SECS: u64 = 240;
const DEFAULT_POLL_INTERVAL_MS: u64 = 3_000;

pub fn unsupported_request_plan_error() -> GatewayError {
    GatewayError::bad_request(
        "Udio adapters currently support image, music, and video-generation passthrough endpoints",
    )
    .with_provider("udio_compatible")
    .with_code("unsupported_udio_endpoint")
}

pub fn missing_browser_runtime_error() -> GatewayError {
    GatewayError::server_error(
        "Udio browser-backed requests require a session cookie or runtimeStateObjectKey-backed browser state.",
    )
    .with_provider("udio_compatible")
    .with_code("missing_udio_browser_runtime")
}

pub fn missing_browser_worker_result_error() -> GatewayError {
    GatewayError::server_error("Udio browser worker reported success without a result payload.")
        .with_provider("udio_compatible")
        .with_code("udio_browser_worker_missing_result")
}

pub fn missing_browser_worker_track_ids_error() -> GatewayError {
    GatewayError::server_error("Udio browser worker completed without returning any track ids.")
        .with_provider("udio_compatible")
        .with_code("udio_missing_track_ids")
}

pub fn empty_browser_worker_output_error(stderr: &str) -> GatewayError {
    GatewayError::server_error(format!(
        "Udio browser worker did not return JSON output. stderr: {}",
        if stderr.is_empty() { "<empty>" } else { stderr }
    ))
    .with_provider("udio_compatible")
    .with_code("udio_browser_worker_empty_output")
}

pub fn browser_worker_output_parse_error(error: &str, stdout: &str) -> GatewayError {
    GatewayError::server_error(format!(
        "Failed to parse Udio browser worker output: {error}. stdout: {stdout}"
    ))
    .with_provider("udio_compatible")
    .with_code("udio_browser_worker_output_parse_failed")
}

pub fn browser_worker_wait_failed_error(error: &str) -> GatewayError {
    GatewayError::server_error(format!(
        "Udio browser worker failed before producing output: {error}"
    ))
    .with_provider("udio_compatible")
    .with_code("udio_browser_worker_wait_failed")
}

pub fn browser_worker_timeout_error() -> GatewayError {
    GatewayError::server_error("Udio browser worker timed out before producing output.")
        .with_provider("udio_compatible")
        .with_code("udio_browser_worker_timeout")
}

pub fn browser_worker_stdin_error(error: &str) -> GatewayError {
    GatewayError::server_error(format!(
        "Failed to write Udio browser worker input: {error}"
    ))
    .with_provider("udio_compatible")
    .with_code("udio_browser_worker_stdin_failed")
}

pub fn browser_worker_spawn_failed_error(
    script_path: &std::path::Path,
    error: &str,
) -> GatewayError {
    GatewayError::server_error(format!(
        "Failed to launch Udio browser worker at {}: {error}",
        script_path.display()
    ))
    .with_provider("udio_compatible")
    .with_code("udio_browser_worker_spawn_failed")
}

pub fn browser_worker_input_serialize_error(error: &str) -> GatewayError {
    GatewayError::server_error(format!(
        "Failed to serialize Udio browser worker input: {error}"
    ))
    .with_provider("udio_compatible")
    .with_code("udio_browser_worker_input_serialize_failed")
}

pub fn remote_browser_worker_result_parse_error(error: &str) -> GatewayError {
    GatewayError::server_error(format!(
        "Failed to parse remote Udio browser worker result: {error}"
    ))
    .with_provider("udio_compatible")
    .with_code("udio_remote_result_parse_failed")
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UdioSong {
    pub id: String,
    pub title: Option<String>,
    pub image_url: Option<String>,
    pub audio_url: Option<String>,
    pub video_url: Option<String>,
    pub created_at: Option<String>,
    pub duration_seconds: Option<f64>,
    pub prompt: Option<String>,
    pub lyrics: Option<String>,
    pub lyric_input: Option<String>,
    pub finished: bool,
    pub ready_to_stream: bool,
    pub estimated_duration_seconds: Option<f64>,
    pub status: String,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UdioOutputKind {
    Image,
    Music,
    Video,
}

impl UdioOutputKind {
    pub fn from_endpoint_kind(endpoint_kind: EndpointKind) -> Result<Self, GatewayError> {
        match endpoint_kind {
            EndpointKind::ImagesGenerations => Ok(Self::Image),
            EndpointKind::MusicGenerations => Ok(Self::Music),
            EndpointKind::VideosGenerations => Ok(Self::Video),
            _ => Err(GatewayError::bad_request(
                "Udio adapters support only image, music, and video generation endpoints.",
            )
            .with_provider("udio_compatible")
            .with_code("unsupported_udio_endpoint")),
        }
    }

    pub fn browser_target_key(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Music => "audio",
            Self::Video => "video",
        }
    }

    fn object_name(self) -> &'static str {
        match self {
            Self::Image => "image.generation",
            Self::Music => "music.generation",
            Self::Video => "video.generation",
        }
    }

    fn media_kind(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Music => "audio",
            Self::Video => "video",
        }
    }

    fn missing_output_code(self) -> &'static str {
        match self {
            Self::Image => "udio_missing_image_url",
            Self::Music => "udio_missing_audio_url",
            Self::Video => "udio_missing_video_url",
        }
    }

    fn missing_output_message(self) -> &'static str {
        match self {
            Self::Image => "Udio songs did not produce any image URLs.",
            Self::Music => "Udio songs did not produce any audio URLs.",
            Self::Video => "Udio songs did not produce any video URLs.",
        }
    }

    pub fn timeout_message(self) -> &'static str {
        match self {
            Self::Image => "Timed out waiting for Udio cover art to reach a terminal state.",
            Self::Music => "Timed out waiting for Udio audio to reach a terminal state.",
            Self::Video => "Timed out waiting for Udio video to reach a terminal state.",
        }
    }
}

pub fn prompt_from_request(req: &CanonicalRelayRequest) -> Result<String, GatewayError> {
    let body_obj = req
        .raw_body
        .as_object()
        .ok_or_else(|| GatewayError::bad_request("Udio request body must be a JSON object."))?;

    extract_prompt(body_obj).ok_or_else(|| {
        GatewayError::bad_request(
            "Udio music requests require one of: prompt, input, lyrics, or parts[].content.",
        )
        .with_code("missing_udio_prompt")
    })
}

pub fn wait_audio(req: &CanonicalRelayRequest) -> bool {
    req.raw_body
        .get("wait_audio")
        .or_else(|| req.raw_body.get("waitAudio"))
        .and_then(|value| value.as_bool())
        .unwrap_or(true)
}

pub fn requested_output_count(req: &CanonicalRelayRequest) -> usize {
    read_number_fields(
        req.raw_body.as_object().unwrap_or(&Map::new()),
        &["num_songs", "numSongs", "n"],
    )
    .map(|value| value.clamp(1.0, 2.0) as usize)
    .unwrap_or(1)
}

pub fn prefers_url_response(req: &CanonicalRelayRequest) -> Result<bool, GatewayError> {
    let Some(value) = req
        .raw_body
        .get("response_format")
        .or_else(|| req.raw_body.get("responseFormat"))
    else {
        return Ok(true);
    };

    let format = value.as_str().ok_or_else(|| {
        GatewayError::bad_request("response_format must be a string when provided.")
            .with_provider("udio_compatible")
            .with_code("invalid_udio_image_response_format")
    })?;

    match format.trim().to_ascii_lowercase().as_str() {
        "url" => Ok(true),
        "b64_json" => Ok(false),
        _ => Err(GatewayError::bad_request(
            "Udio image endpoints currently support response_format=url or b64_json.",
        )
        .with_provider("udio_compatible")
        .with_code("unsupported_udio_image_response_format")),
    }
}

pub fn wait_timeout_secs(req: &CanonicalRelayRequest) -> u64 {
    read_optional_u64(
        req.raw_body.as_object().unwrap_or(&Map::new()),
        &[
            "wait_timeout_secs",
            "waitTimeoutSecs",
            "timeout_secs",
            "timeoutSecs",
        ],
    )
    .map(|value| value.clamp(15, 900))
    .unwrap_or(DEFAULT_WAIT_TIMEOUT_SECS)
}

pub fn poll_interval_ms(req: &CanonicalRelayRequest) -> u64 {
    read_optional_u64(
        req.raw_body.as_object().unwrap_or(&Map::new()),
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
    _model: &str,
) -> Result<Value, GatewayError> {
    let body_obj = req
        .raw_body
        .as_object()
        .ok_or_else(|| GatewayError::bad_request("Udio request body must be a JSON object."))?;

    let prompt = prompt_from_request(req)?;
    let lyrics = read_optional_string(
        body_obj,
        &[
            "lyricInput",
            "lyric_input",
            "custom_lyrics",
            "customLyrics",
            "lyrics",
        ],
    )
    .unwrap_or_default();
    let instrumental = read_optional_bool(body_obj, &["instrumental"]).unwrap_or(false);

    let mut gen_params = body_obj
        .get("gen_params")
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    let sampler_options = body_obj
        .get("samplerOptions")
        .or_else(|| body_obj.get("sampler_options"))
        .and_then(|value| value.as_object());
    let lyrics_type = normalize_lyrics_type(body_obj, &gen_params, &lyrics, instrumental);
    let bypass_prompt_optimization = read_optional_bool(
        body_obj,
        &["bypass_prompt_optimization", "bypassPromptOptimization"],
    )
    .or_else(|| read_object_bool(&gen_params, &["bypass_prompt_optimization"]))
    .or(Some(false));
    let seed = read_number_fields(body_obj, &["seed"])
        .or_else(|| sampler_options.and_then(|value| read_number_fields(value, &["seed"])))
        .or_else(|| read_number_fields_from_object(&gen_params, &["seed"]))
        .or(Some(-1.0));
    let song_section_start =
        read_number_fields(body_obj, &["song_section_start", "songSectionStart"])
            .or_else(|| read_number_fields_from_object(&gen_params, &["song_section_start"]))
            .or(Some(0.0));
    let song_section_end = read_number_fields(body_obj, &["song_section_end", "songSectionEnd"])
        .or_else(|| read_number_fields_from_object(&gen_params, &["song_section_end"]))
        .or(Some(1.0));
    let prompt_strength = read_number_fields(body_obj, &["prompt_strength", "promptStrength"])
        .or_else(|| read_number_fields_from_object(&gen_params, &["prompt_strength"]))
        .or(Some(0.5));
    let clarity_strength = read_number_fields(body_obj, &["clarity_strength", "clarityStrength"])
        .or_else(|| read_number_fields_from_object(&gen_params, &["clarity_strength"]))
        .or(Some(0.25));
    let lyrics_strength = read_number_fields(body_obj, &["lyrics_strength", "lyricsStrength"])
        .or_else(|| read_number_fields_from_object(&gen_params, &["lyrics_strength"]))
        .or(Some(0.5));
    let generation_quality =
        read_number_fields(body_obj, &["generation_quality", "generationQuality"])
            .or_else(|| read_number_fields_from_object(&gen_params, &["generation_quality"]))
            .or(Some(0.75));
    let negative_prompt = read_optional_string(body_obj, &["negative_prompt", "negativePrompt"])
        .or_else(|| read_object_string(&gen_params, &["negative_prompt"]))
        .or(Some(String::new()));
    let model_type = read_optional_string(
        body_obj,
        &[
            "model_type",
            "modelType",
            "upstream_model_type",
            "upstreamModelType",
        ],
    )
    .or_else(|| read_object_string(&gen_params, &["model_type"]))
    .or(Some(UDIO_DEFAULT_MODEL_TYPE.to_string()));
    let use_allegro = read_optional_bool(body_obj, &["use_allegro", "useAllegro"])
        .or_else(|| read_object_bool(&gen_params, &["use_allegro"]))
        .or(Some(true));
    let use_style = read_optional_bool(body_obj, &["use_style", "useStyle"])
        .or_else(|| read_object_bool(&gen_params, &["use_style"]))
        .or(Some(false));
    let bpm_enabled = read_optional_bool(body_obj, &["bpm_enabled", "bpmEnabled"])
        .or_else(|| read_object_bool(&gen_params, &["bpm_enabled"]))
        .or(Some(false));
    let bpm = read_number_fields(body_obj, &["bpm"])
        .or_else(|| read_number_fields_from_object(&gen_params, &["bpm"]));
    let num_auto_extend_samples = read_number_fields(
        body_obj,
        &["num_auto_extend_samples", "numAutoExtendSamples"],
    )
    .or_else(|| read_number_fields_from_object(&gen_params, &["num_auto_extend_samples"]))
    .or(Some(3.0));
    let audio_conditioning_path = read_optional_string(
        body_obj,
        &["audio_conditioning_path", "audioConditioningPath"],
    )
    .or_else(|| {
        sampler_options.and_then(|value| {
            read_optional_string(value, &["audio_conditioning_path", "audioConditioningPath"])
        })
    })
    .or_else(|| read_object_string(&gen_params, &["audio_conditioning_path"]));
    let audio_conditioning_song_id = read_optional_string(
        body_obj,
        &["audio_conditioning_song_id", "audioConditioningSongId"],
    )
    .or_else(|| {
        sampler_options.and_then(|value| {
            read_optional_string(
                value,
                &["audio_conditioning_song_id", "audioConditioningSongId"],
            )
        })
    })
    .or_else(|| read_object_string(&gen_params, &["audio_conditioning_song_id"]));
    let audio_conditioning_type = read_optional_string(
        body_obj,
        &["audio_conditioning_type", "audioConditioningType"],
    )
    .or_else(|| {
        sampler_options.and_then(|value| {
            read_optional_string(value, &["audio_conditioning_type", "audioConditioningType"])
        })
    })
    .or_else(|| read_object_string(&gen_params, &["audio_conditioning_type"]));
    let crop_start_time = read_number_fields(body_obj, &["crop_start_time", "cropStartTime"])
        .or_else(|| {
            sampler_options
                .and_then(|value| read_number_fields(value, &["crop_start_time", "cropStartTime"]))
        })
        .or_else(|| read_number_fields_from_object(&gen_params, &["crop_start_time"]));

    maybe_insert_string(&mut gen_params, "prompt", Some(prompt));
    maybe_insert_string(&mut gen_params, "lyrics", Some(lyrics.clone()));
    maybe_insert_string(&mut gen_params, "lyrics_type", Some(lyrics_type));
    maybe_insert_bool(
        &mut gen_params,
        "bypass_prompt_optimization",
        bypass_prompt_optimization,
    );
    maybe_insert_number(&mut gen_params, "seed", seed);
    maybe_insert_number(&mut gen_params, "song_section_start", song_section_start);
    maybe_insert_number(&mut gen_params, "song_section_end", song_section_end);
    maybe_insert_number(&mut gen_params, "prompt_strength", prompt_strength);
    maybe_insert_number(&mut gen_params, "clarity_strength", clarity_strength);
    maybe_insert_number(&mut gen_params, "lyrics_strength", lyrics_strength);
    maybe_insert_number(&mut gen_params, "generation_quality", generation_quality);
    maybe_insert_string(&mut gen_params, "negative_prompt", negative_prompt);
    maybe_insert_string(&mut gen_params, "model_type", model_type);
    maybe_insert_bool(&mut gen_params, "use_allegro", use_allegro);
    maybe_insert_bool(&mut gen_params, "use_style", use_style);
    maybe_insert_bool(&mut gen_params, "bpm_enabled", bpm_enabled);
    maybe_insert_number(&mut gen_params, "bpm", bpm);
    maybe_insert_number(
        &mut gen_params,
        "num_auto_extend_samples",
        num_auto_extend_samples,
    );
    maybe_insert_string(
        &mut gen_params,
        "audio_conditioning_path",
        audio_conditioning_path,
    );
    maybe_insert_string(
        &mut gen_params,
        "audio_conditioning_song_id",
        audio_conditioning_song_id,
    );
    maybe_insert_string(
        &mut gen_params,
        "audio_conditioning_type",
        audio_conditioning_type,
    );
    maybe_insert_number(&mut gen_params, "crop_start_time", crop_start_time);

    let mut config = body_obj
        .get("config")
        .and_then(|value| value.as_object().cloned())
        .or_else(|| {
            gen_params
                .get("config")
                .and_then(|value| value.as_object().cloned())
        })
        .unwrap_or_default();
    let config_mode =
        read_optional_string(body_obj, &["mode", "generation_mode", "generationMode"])
            .or_else(|| read_object_string(&config, &["mode"]))
            .or(Some("regular".to_string()));
    maybe_insert_string(&mut config, "mode", config_mode);
    gen_params.insert("config".to_string(), Value::Object(config));

    let mut body = Map::new();
    body.insert("gen_params".to_string(), Value::Object(gen_params));
    let requested_num_songs = read_number_fields(body_obj, &["num_songs", "numSongs", "n"])
        .map(|value| value.clamp(1.0, 2.0));
    if requested_num_songs
        .map(|value| (value.round() as i64) > 1)
        .unwrap_or(false)
    {
        maybe_insert_number(&mut body, "num_songs", requested_num_songs);
    }
    maybe_insert_string(
        &mut body,
        "webhook_url",
        read_optional_string(body_obj, &["webhook_url", "webhookUrl"]),
    );
    maybe_insert_bool(
        &mut body,
        "demoRequest",
        read_optional_bool(body_obj, &["demoRequest", "demo_request"]),
    );
    maybe_insert_string(
        &mut body,
        "captchaToken",
        read_optional_string(body_obj, &["captchaToken", "captcha_token"]),
    );

    Ok(Value::Object(body))
}

pub fn extract_track_ids(body: &Value) -> Result<Vec<String>, GatewayError> {
    let ids = body
        .get("track_ids")
        .or_else(|| body.get("trackIds"))
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|ids| !ids.is_empty())
        .or_else(|| {
            body.get("songs")
                .and_then(|value| value.as_array())
                .map(|songs| {
                    songs
                        .iter()
                        .filter_map(|song| song.get("id").and_then(|value| value.as_str()))
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
        })
        .filter(|ids| !ids.is_empty())
        .ok_or_else(|| {
            GatewayError::server_error("Udio generation response missing track ids.")
                .with_provider("udio_compatible")
                .with_code("udio_missing_track_ids")
        })?;

    Ok(ids)
}

pub fn extract_songs_from_feed(body: &Value) -> Result<Vec<UdioSong>, GatewayError> {
    let songs = body
        .get("songs")
        .and_then(|value| value.as_array())
        .ok_or_else(|| {
            GatewayError::server_error("Udio songs response missing songs array.")
                .with_provider("udio_compatible")
                .with_code("udio_missing_songs")
        })?;

    let parsed = songs.iter().filter_map(parse_song).collect::<Vec<_>>();
    if parsed.is_empty() {
        return Err(GatewayError::server_error(
            "Udio songs response did not contain any valid songs.",
        )
        .with_provider("udio_compatible")
        .with_code("udio_invalid_songs"));
    }

    Ok(parsed)
}

pub fn songs_ready(songs: &[UdioSong]) -> bool {
    songs_ready_for_output(songs, UdioOutputKind::Music)
}

pub fn songs_ready_for_output(songs: &[UdioSong], output_kind: UdioOutputKind) -> bool {
    !songs.is_empty() && songs.iter().all(|song| song_has_output(song, output_kind))
}

pub fn collect_output_urls(
    songs: &[UdioSong],
    output_kind: UdioOutputKind,
) -> Result<Vec<String>, GatewayError> {
    let urls = songs
        .iter()
        .filter_map(|song| output_url(song, output_kind).map(str::to_string))
        .collect::<Vec<_>>();
    if urls.is_empty() {
        return Err(missing_output_error(output_kind));
    }
    Ok(urls)
}

pub fn build_music_generation_response(
    model: &str,
    prompt: &str,
    songs: &[UdioSong],
    completed: bool,
    message: Option<&str>,
) -> Value {
    json!({
        "object": "music.generation",
        "provider": "udio",
        "created": current_unix_timestamp(),
        "completed": completed,
        "model": model,
        "prompt": prompt,
        "message": message.filter(|value| !value.trim().is_empty()),
        "data": songs,
    })
}

pub fn build_openai_images_response_from_urls(
    req: &CanonicalRelayRequest,
    prompt: &str,
    urls: &[String],
) -> Result<Value, GatewayError> {
    Ok(json!({
        "created": current_unix_timestamp(),
        "data": urls
            .iter()
            .take(requested_output_count(req))
            .map(|url| json!({
                "url": url,
                "revised_prompt": prompt,
            }))
            .collect::<Vec<_>>(),
    }))
}

pub fn build_openai_images_response_from_bytes(
    req: &CanonicalRelayRequest,
    prompt: &str,
    images: &[(String, Vec<u8>)],
) -> Value {
    json!({
        "created": current_unix_timestamp(),
        "data": images
            .iter()
            .take(requested_output_count(req))
            .map(|(mime_type, bytes)| {
                json!({
                    "b64_json": base64::engine::general_purpose::STANDARD.encode(bytes),
                    "revised_prompt": prompt,
                    "mime_type": mime_type,
                })
            })
            .collect::<Vec<_>>(),
    })
}

pub fn build_video_generation_response(
    model: &str,
    prompt: &str,
    songs: &[UdioSong],
    completed: bool,
    message: Option<&str>,
) -> Result<Value, GatewayError> {
    let data = songs
        .iter()
        .filter_map(|song| serialize_media_asset(song, UdioOutputKind::Video))
        .collect::<Vec<_>>();
    if data.is_empty() {
        return Err(missing_output_error(UdioOutputKind::Video));
    }

    Ok(json!({
        "object": UdioOutputKind::Video.object_name(),
        "provider": "udio",
        "created": current_unix_timestamp(),
        "completed": completed,
        "model": model,
        "prompt": prompt,
        "message": message.filter(|value| !value.trim().is_empty()),
        "data": data,
    }))
}

pub fn pending_songs(track_ids: &[String]) -> Vec<UdioSong> {
    track_ids
        .iter()
        .map(|id| UdioSong {
            id: id.clone(),
            title: None,
            image_url: None,
            audio_url: None,
            video_url: None,
            created_at: None,
            duration_seconds: None,
            prompt: None,
            lyrics: None,
            lyric_input: None,
            finished: false,
            ready_to_stream: false,
            estimated_duration_seconds: None,
            status: "pending".to_string(),
            error_message: None,
        })
        .collect()
}

fn parse_song(value: &Value) -> Option<UdioSong> {
    let id = read_value_string(value, &["id"])?;
    let finished = read_value_bool(value, &["finished"]).unwrap_or(false);
    let ready_to_stream =
        read_value_bool(value, &["readyToStream", "ready_to_stream"]).unwrap_or(false);
    let status = read_value_string(value, &["status", "state"]).unwrap_or_else(|| {
        if finished {
            "finished".to_string()
        } else if ready_to_stream {
            "ready".to_string()
        } else {
            "pending".to_string()
        }
    });
    let prompt = read_value_string(value, &["prompt", "description"]);
    let lyric_input = read_value_string(value, &["lyricInput", "lyric_input", "lyrics"]);
    let audio_url = read_value_string(value, &["song_path", "songPath", "audio_url", "audioUrl"]);

    Some(UdioSong {
        id,
        title: read_value_string(value, &["title"]),
        image_url: read_value_string(
            value,
            &[
                "image_url",
                "imageUrl",
                "image_path",
                "imagePath",
                "cover_image_url",
                "coverImageUrl",
                "cover_art_url",
                "coverArtUrl",
            ],
        ),
        audio_url,
        video_url: read_value_string(value, &["video_url", "videoUrl", "video_path", "videoPath"]),
        created_at: read_value_string(value, &["created_at", "createdAt"]),
        duration_seconds: read_value_number(
            value,
            &["duration_seconds", "durationSeconds", "duration"],
        ),
        prompt,
        lyrics: read_value_string(value, &["lyrics"]),
        lyric_input,
        finished,
        ready_to_stream,
        estimated_duration_seconds: read_value_number(
            value,
            &["estimatedDuration", "estimated_duration"],
        ),
        status,
        error_message: read_value_string(
            value,
            &["error_message", "errorMessage", "error_detail", "error"],
        ),
    })
}

fn song_has_output(song: &UdioSong, output_kind: UdioOutputKind) -> bool {
    output_url(song, output_kind).is_some()
}

fn output_url(song: &UdioSong, output_kind: UdioOutputKind) -> Option<&str> {
    match output_kind {
        UdioOutputKind::Image => song.image_url.as_deref(),
        UdioOutputKind::Music => song.audio_url.as_deref(),
        UdioOutputKind::Video => song.video_url.as_deref(),
    }
    .map(str::trim)
    .filter(|value| !value.is_empty())
}

fn missing_output_error(output_kind: UdioOutputKind) -> GatewayError {
    GatewayError::server_error(output_kind.missing_output_message())
        .with_provider("udio_compatible")
        .with_code(output_kind.missing_output_code())
}

fn serialize_media_asset(song: &UdioSong, output_kind: UdioOutputKind) -> Option<Value> {
    let url = output_url(song, output_kind)?;
    Some(json!({
        "kind": output_kind.media_kind(),
        "url": url,
        "track_id": song.id.clone(),
        "title": song.title.clone(),
        "status": song.status.clone(),
        "image_url": song.image_url.clone(),
        "audio_url": song.audio_url.clone(),
        "video_url": song.video_url.clone(),
        "duration_seconds": song.duration_seconds,
    }))
}

fn extract_prompt(map: &Map<String, Value>) -> Option<String> {
    for field in ["prompt", "input", "lyrics"] {
        if let Some(text) = map
            .get(field)
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(text.to_string());
        }
    }

    let parts = map.get("parts")?.as_array()?;
    let mut texts = Vec::new();
    for part in parts {
        match part {
            Value::String(text) => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    texts.push(trimmed.to_string());
                }
            }
            Value::Object(obj) => {
                if let Some(text) = obj
                    .get("content")
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    texts.push(text.to_string());
                }
            }
            _ => {}
        }
    }

    if texts.is_empty() {
        None
    } else {
        Some(texts.join("\n"))
    }
}

fn read_optional_string(obj: &Map<String, Value>, aliases: &[&str]) -> Option<String> {
    aliases
        .iter()
        .find_map(|key| obj.get(*key).and_then(|value| value.as_str()))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn read_optional_bool(obj: &Map<String, Value>, aliases: &[&str]) -> Option<bool> {
    aliases.iter().find_map(|key| {
        obj.get(*key).and_then(|value| match value {
            Value::Bool(flag) => Some(*flag),
            Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => Some(true),
                "false" | "0" | "no" | "off" => Some(false),
                _ => None,
            },
            _ => None,
        })
    })
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

fn read_number_fields(obj: &Map<String, Value>, aliases: &[&str]) -> Option<f64> {
    aliases
        .iter()
        .find_map(|key| obj.get(*key))
        .and_then(read_number_value)
}

fn read_number_value(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn read_value_string(value: &Value, aliases: &[&str]) -> Option<String> {
    let obj = value.as_object()?;
    read_optional_string(obj, aliases)
}

fn read_object_string(obj: &Map<String, Value>, aliases: &[&str]) -> Option<String> {
    read_optional_string(obj, aliases)
}

fn read_object_bool(obj: &Map<String, Value>, aliases: &[&str]) -> Option<bool> {
    read_optional_bool(obj, aliases)
}

fn read_number_fields_from_object(obj: &Map<String, Value>, aliases: &[&str]) -> Option<f64> {
    read_number_fields(obj, aliases)
}

fn read_value_bool(value: &Value, aliases: &[&str]) -> Option<bool> {
    let obj = value.as_object()?;
    aliases.iter().find_map(|key| {
        obj.get(*key).and_then(|value| match value {
            Value::Bool(flag) => Some(*flag),
            Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            },
            _ => None,
        })
    })
}

fn read_value_number(value: &Value, aliases: &[&str]) -> Option<f64> {
    let obj = value.as_object()?;
    read_number_fields(obj, aliases)
}

fn maybe_insert_string(target: &mut Map<String, Value>, key: &str, value: Option<String>) {
    if !target.contains_key(key) {
        if let Some(value) = value {
            target.insert(key.to_string(), Value::String(value));
        }
    }
}

fn maybe_insert_bool(target: &mut Map<String, Value>, key: &str, value: Option<bool>) {
    if !target.contains_key(key) {
        if let Some(value) = value {
            target.insert(key.to_string(), Value::Bool(value));
        }
    }
}

fn maybe_insert_number(target: &mut Map<String, Value>, key: &str, value: Option<f64>) {
    if !target.contains_key(key) {
        if let Some(value) = value.and_then(number_from_f64_preserving_integers) {
            target.insert(key.to_string(), value);
        }
    }
}

fn number_from_f64_preserving_integers(value: f64) -> Option<Value> {
    if !value.is_finite() {
        return None;
    }
    if value.fract() == 0.0 {
        if value >= 0.0 && value <= u64::MAX as f64 {
            return Some(Value::Number(Number::from(value as u64)));
        }
        if value >= i64::MIN as f64 && value <= i64::MAX as f64 {
            return Some(Value::Number(Number::from(value as i64)));
        }
    }
    Number::from_f64(value).map(Value::Number)
}

fn normalize_lyrics_type(
    body_obj: &Map<String, Value>,
    gen_params: &Map<String, Value>,
    lyrics: &str,
    instrumental: bool,
) -> String {
    if instrumental {
        return "instrumental".to_string();
    }

    let raw = read_optional_string(
        body_obj,
        &["lyrics_type", "lyricsType", "lyrics_mode", "lyricsMode"],
    )
    .or_else(|| read_object_string(gen_params, &["lyrics_type"]));

    match raw
        .as_deref()
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("instrumental") => "instrumental".to_string(),
        Some("user") | Some("lyricinput") => "user".to_string(),
        Some("generate") | Some("infer") | Some("lyricprompt") => "generate".to_string(),
        Some(other) if !other.is_empty() => other.to_string(),
        _ if !lyrics.trim().is_empty() => "user".to_string(),
        _ => "generate".to_string(),
    }
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
    use crate::protocol::canonical::{
        CanonicalMessage, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
    };
    use crate::routing::candidate::ProviderAccountPayload;
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

    fn make_request(body: Value) -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::MusicGenerations,
            requested_model: Some(UDIO_DEFAULT_MODEL.to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "placeholder".to_string(),
                }],
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
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }

    #[test]
    fn build_generate_request_maps_seed_and_lyrics() {
        let req = make_request(json!({
            "prompt": "dreamy synthwave soundtrack",
            "seed": 42,
            "lyrics": "electric city lights"
        }));

        let body = build_generate_request(&req, UDIO_DEFAULT_MODEL).unwrap();
        assert_eq!(body["gen_params"]["prompt"], "dreamy synthwave soundtrack");
        assert_eq!(body["gen_params"]["seed"], 42);
        assert_eq!(body["gen_params"]["lyrics"], "electric city lights");
        assert_eq!(body["gen_params"]["lyrics_type"], "user");
        assert!(body.get("model").is_none());
    }

    #[test]
    fn udio_unsupported_request_plan_error_matches_contract() {
        let err = unsupported_request_plan_error();
        assert_eq!(err.http_status, Some(400));
        assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
        assert_eq!(err.code.as_deref(), Some("unsupported_udio_endpoint"));
    }

    #[test]
    fn plan_udio_chat_endpoint_rejected_locally() {
        let payload = make_payload("udio_compatible", "https://www.udio.com");
        let mut req = make_request(json!({ "prompt": "ignored" }));
        req.endpoint_kind = EndpointKind::ChatCompletions;
        let err = crate::upstream::client::UpstreamClient::build_request_plan(
            &payload,
            &req,
            "udio-music",
            false,
        )
        .expect_err("udio chat requests should be rejected");
        assert_eq!(err.http_status, Some(400));
        assert_eq!(err.code.as_deref(), Some("unsupported_udio_endpoint"));
    }

    #[test]
    fn build_generate_request_preserves_conditioning_fields() {
        let req = make_request(json!({
            "prompt": "continue the chorus with brighter strings",
            "audio_conditioning_path": "https://cdn.example.com/song.mp3",
            "audio_conditioning_song_id": "song-123",
            "audio_conditioning_type": "continuation",
            "crop_start_time": 0.9,
            "samplerOptions": {
                "seed": 7
            }
        }));

        let body = build_generate_request(&req, UDIO_DEFAULT_MODEL).unwrap();
        assert_eq!(body["gen_params"]["seed"], 7);
        assert_eq!(
            body["gen_params"]["audio_conditioning_path"],
            "https://cdn.example.com/song.mp3"
        );
        assert_eq!(body["gen_params"]["audio_conditioning_song_id"], "song-123");
        assert_eq!(
            body["gen_params"]["audio_conditioning_type"],
            "continuation"
        );
        assert_eq!(body["gen_params"]["crop_start_time"], 0.9);
        assert_eq!(body["gen_params"]["config"]["mode"], "regular");
    }

    #[test]
    fn build_generate_request_maps_n_to_num_songs() {
        let req = make_request(json!({
            "prompt": "double take chorus",
            "n": 2
        }));

        let body = build_generate_request(&req, UDIO_DEFAULT_MODEL).unwrap();
        assert_eq!(body["num_songs"], 2);
    }

    #[test]
    fn build_generate_request_omits_num_songs_for_single_song_default() {
        let req = make_request(json!({
            "prompt": "single take chorus"
        }));

        let body = build_generate_request(&req, UDIO_DEFAULT_MODEL).unwrap();
        assert!(body.get("num_songs").is_none());

        let explicit_single = make_request(json!({
            "prompt": "single take chorus",
            "n": 1
        }));
        let explicit_single_body =
            build_generate_request(&explicit_single, UDIO_DEFAULT_MODEL).unwrap();
        assert!(explicit_single_body.get("num_songs").is_none());
    }

    #[test]
    fn build_generate_request_preserves_captcha_token_aliases() {
        let direct = make_request(json!({
            "prompt": "challenge aware chorus",
            "captchaToken": "token-direct"
        }));
        let direct_body = build_generate_request(&direct, UDIO_DEFAULT_MODEL).unwrap();
        assert_eq!(direct_body["captchaToken"], "token-direct");

        let alias = make_request(json!({
            "prompt": "challenge aware chorus",
            "captcha_token": "token-alias"
        }));
        let alias_body = build_generate_request(&alias, UDIO_DEFAULT_MODEL).unwrap();
        assert_eq!(alias_body["captchaToken"], "token-alias");
    }

    #[test]
    fn missing_browser_runtime_error_matches_contract() {
        let err = missing_browser_runtime_error();
        assert_eq!(err.http_status, Some(500));
        assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
        assert_eq!(err.code.as_deref(), Some("missing_udio_browser_runtime"));
    }

    #[test]
    fn missing_browser_worker_result_error_matches_contract() {
        let err = missing_browser_worker_result_error();
        assert_eq!(err.http_status, Some(500));
        assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
        assert_eq!(
            err.code.as_deref(),
            Some("udio_browser_worker_missing_result")
        );
        assert_eq!(
            err.message.as_str(),
            "Udio browser worker reported success without a result payload."
        );
    }

    #[test]
    fn missing_browser_worker_track_ids_error_matches_contract() {
        let err = missing_browser_worker_track_ids_error();
        assert_eq!(err.http_status, Some(500));
        assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
        assert_eq!(err.code.as_deref(), Some("udio_missing_track_ids"));
        assert_eq!(
            err.message.as_str(),
            "Udio browser worker completed without returning any track ids."
        );
    }

    #[test]
    fn empty_browser_worker_output_error_formats_stderr_fallback() {
        let with_stderr = empty_browser_worker_output_error("permission denied");
        assert_eq!(with_stderr.http_status, Some(500));
        assert_eq!(
            with_stderr.provider_name.as_deref(),
            Some("udio_compatible")
        );
        assert_eq!(
            with_stderr.code.as_deref(),
            Some("udio_browser_worker_empty_output")
        );
        assert_eq!(
            with_stderr.message.as_str(),
            "Udio browser worker did not return JSON output. stderr: permission denied"
        );

        let empty = empty_browser_worker_output_error("");
        assert_eq!(
            empty.message.as_str(),
            "Udio browser worker did not return JSON output. stderr: <empty>"
        );
    }

    #[test]
    fn browser_worker_output_parse_error_formats_error_and_stdout() {
        let err = browser_worker_output_parse_error("expected value", "{\"oops\":");
        assert_eq!(err.http_status, Some(500));
        assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
        assert_eq!(
            err.code.as_deref(),
            Some("udio_browser_worker_output_parse_failed")
        );
        assert_eq!(
            err.message.as_str(),
            "Failed to parse Udio browser worker output: expected value. stdout: {\"oops\":"
        );
    }

    #[test]
    fn browser_worker_wait_failed_error_formats_cause() {
        let err = browser_worker_wait_failed_error("The pipe has been ended");
        assert_eq!(err.http_status, Some(500));
        assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
        assert_eq!(err.code.as_deref(), Some("udio_browser_worker_wait_failed"));
        assert_eq!(
            err.message.as_str(),
            "Udio browser worker failed before producing output: The pipe has been ended"
        );
    }

    #[test]
    fn browser_worker_timeout_error_matches_contract() {
        let err = browser_worker_timeout_error();
        assert_eq!(err.http_status, Some(500));
        assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
        assert_eq!(err.code.as_deref(), Some("udio_browser_worker_timeout"));
        assert_eq!(
            err.message.as_str(),
            "Udio browser worker timed out before producing output."
        );
    }

    #[test]
    fn browser_worker_stdin_error_formats_cause() {
        let err = browser_worker_stdin_error("broken pipe");
        assert_eq!(err.http_status, Some(500));
        assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
        assert_eq!(
            err.code.as_deref(),
            Some("udio_browser_worker_stdin_failed")
        );
        assert_eq!(
            err.message.as_str(),
            "Failed to write Udio browser worker input: broken pipe"
        );
    }

    #[test]
    fn browser_worker_spawn_failed_error_formats_script_path_and_cause() {
        let err = browser_worker_spawn_failed_error(
            std::path::Path::new("C:/tmp/udio-browser-worker.mjs"),
            "The system cannot find the file specified. (os error 2)",
        );
        assert_eq!(err.http_status, Some(500));
        assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
        assert_eq!(
            err.code.as_deref(),
            Some("udio_browser_worker_spawn_failed")
        );
        assert_eq!(
            err.message.as_str(),
            "Failed to launch Udio browser worker at C:/tmp/udio-browser-worker.mjs: The system cannot find the file specified. (os error 2)"
        );
    }

    #[test]
    fn browser_worker_input_serialize_error_formats_cause() {
        let err = browser_worker_input_serialize_error("missing field `prompt`");
        assert_eq!(err.http_status, Some(500));
        assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
        assert_eq!(
            err.code.as_deref(),
            Some("udio_browser_worker_input_serialize_failed")
        );
        assert_eq!(
            err.message.as_str(),
            "Failed to serialize Udio browser worker input: missing field `prompt`"
        );
    }

    #[test]
    fn remote_browser_worker_result_parse_error_formats_cause() {
        let err = remote_browser_worker_result_parse_error("expected value");
        assert_eq!(err.http_status, Some(500));
        assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
        assert_eq!(err.code.as_deref(), Some("udio_remote_result_parse_failed"));
        assert_eq!(
            err.message.as_str(),
            "Failed to parse remote Udio browser worker result: expected value"
        );
    }

    #[test]
    fn songs_ready_requires_audio_url_even_when_ready_to_stream() {
        let songs = extract_songs_from_feed(&json!({
            "songs": [
                {
                    "id": "song-1",
                    "readyToStream": true,
                    "estimatedDuration": 130
                }
            ]
        }))
        .unwrap();

        assert!(songs[0].ready_to_stream);
        assert_eq!(songs[0].estimated_duration_seconds, Some(130.0));
        assert!(!songs_ready(&songs));
    }

    #[test]
    fn songs_ready_for_image_and_video_waits_for_target_artifact() {
        let songs = extract_songs_from_feed(&json!({
            "songs": [
                {
                    "id": "song-1",
                    "readyToStream": true,
                    "image_url": "https://cdn.example.com/song-1.jpg",
                    "video_url": "https://cdn.example.com/song-1.mp4"
                }
            ]
        }))
        .unwrap();

        assert!(songs_ready_for_output(&songs, UdioOutputKind::Image));
        assert!(songs_ready_for_output(&songs, UdioOutputKind::Video));

        let pending = extract_songs_from_feed(&json!({
            "songs": [
                {
                    "id": "song-2",
                    "status": "pending"
                }
            ]
        }))
        .unwrap();
        assert!(!songs_ready_for_output(&pending, UdioOutputKind::Image));
        assert!(!songs_ready_for_output(&pending, UdioOutputKind::Video));
    }

    #[test]
    fn extract_track_ids_reads_track_ids_or_song_ids() {
        let direct = extract_track_ids(&json!({
            "track_ids": ["trk-1", "trk-2"]
        }))
        .unwrap();
        assert_eq!(direct, vec!["trk-1".to_string(), "trk-2".to_string()]);

        let from_songs = extract_track_ids(&json!({
            "songs": [
                { "id": "song-a" },
                { "id": "song-b" }
            ]
        }))
        .unwrap();
        assert_eq!(from_songs, vec!["song-a".to_string(), "song-b".to_string()]);
    }

    #[test]
    fn extract_songs_from_feed_parses_finished_song() {
        let songs = extract_songs_from_feed(&json!({
            "songs": [
                {
                    "id": "song-1",
                    "title": "Night Drive",
                    "song_path": "https://cdn.example.com/song-1.mp3",
                    "image_url": "https://cdn.example.com/song-1.jpg",
                    "finished": true,
                    "created_at": "2026-04-11T00:00:00.000Z"
                }
            ]
        }))
        .unwrap();

        assert_eq!(songs.len(), 1);
        assert_eq!(songs[0].id, "song-1");
        assert_eq!(
            songs[0].audio_url.as_deref(),
            Some("https://cdn.example.com/song-1.mp3")
        );
        assert!(songs_ready(&songs));
    }

    #[test]
    fn image_responses_support_b64_output_mode() {
        let mut req = make_request(json!({
            "prompt": "cover art",
            "response_format": "b64_json",
            "n": 1
        }));
        req.endpoint_kind = EndpointKind::ImagesGenerations;

        assert!(!prefers_url_response(&req).unwrap());
        let response = build_openai_images_response_from_bytes(
            &req,
            "cover art",
            &[("image/png".to_string(), vec![1, 2, 3, 4])],
        );
        assert_eq!(response["data"][0]["mime_type"], "image/png");
        assert!(response["data"][0]["b64_json"].as_str().is_some());
    }

    #[test]
    fn build_video_generation_response_uses_video_asset_shape() {
        let songs = extract_songs_from_feed(&json!({
            "songs": [
                {
                    "id": "song-1",
                    "title": "Night Drive",
                    "song_path": "https://cdn.example.com/song-1.mp3",
                    "video_url": "https://cdn.example.com/song-1.mp4",
                    "finished": true
                }
            ]
        }))
        .unwrap();

        let response = build_video_generation_response(
            UDIO_DEFAULT_MODEL,
            "night drive city lights",
            &songs,
            true,
            None,
        )
        .unwrap();
        assert_eq!(response["object"], "video.generation");
        assert_eq!(response["provider"], "udio");
        assert_eq!(response["data"][0]["kind"], "video");
        assert_eq!(
            response["data"][0]["url"],
            "https://cdn.example.com/song-1.mp4"
        );
    }
}
