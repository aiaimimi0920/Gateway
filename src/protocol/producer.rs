use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use futures::StreamExt;
use serde_json::{json, Map, Value};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{classify_network_error, GatewayError};
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
};
use crate::protocol::sse_parse::{parse_sse_line, SseFrame, SseParseState};
use crate::upstream::common::RequestPlan;

pub const PRODUCER_DEFAULT_MODEL: &str = "producer:standard";
pub const PRODUCER_IMAGE_DEFAULT_MODEL: &str = "producer:image";
pub const PRODUCER_VIDEO_DEFAULT_MODEL: &str = "producer:music-video";
const PRODUCER_PUBLIC_ASSET_BASE_URL: &str =
    "https://storage.googleapis.com/producer-app-public/assets";

pub fn unsupported_request_plan_error() -> GatewayError {
    GatewayError::bad_request(
        "Producer.ai adapters currently support reverse-web image, music, and video-generation passthrough endpoints",
    )
    .with_provider("producer_compatible")
    .with_code("unsupported_producer_endpoint")
}

pub fn unsupported_media_endpoint_error() -> GatewayError {
    GatewayError::bad_request(
        "Producer.ai adapters currently support only /v1/images/generations, /v1/music/generations, and /v1/videos/generations.",
    )
    .with_provider("producer_compatible")
    .with_code("unsupported_producer_endpoint")
}

pub fn missing_browser_worker_result_error() -> GatewayError {
    GatewayError::server_error("Producer browser worker reported success without a result payload.")
        .with_provider("producer_compatible")
        .with_code("producer_browser_worker_missing_result")
}

pub fn empty_browser_worker_output_error(stderr: &str) -> GatewayError {
    GatewayError::server_error(format!(
        "Producer browser worker did not return JSON output. stderr: {}",
        if stderr.is_empty() { "<empty>" } else { stderr }
    ))
    .with_provider("producer_compatible")
    .with_code("producer_browser_worker_empty_output")
}

pub fn browser_worker_output_parse_error(error: &str, stdout: &str) -> GatewayError {
    GatewayError::server_error(format!(
        "Failed to parse Producer browser worker output: {error}. stdout: {stdout}"
    ))
    .with_provider("producer_compatible")
    .with_code("producer_browser_worker_output_parse_failed")
}

pub fn browser_worker_wait_failed_error(error: &str) -> GatewayError {
    GatewayError::server_error(format!(
        "Producer browser worker failed before producing output: {error}"
    ))
    .with_provider("producer_compatible")
    .with_code("producer_browser_worker_wait_failed")
}

pub fn browser_worker_timeout_error() -> GatewayError {
    GatewayError::server_error("Producer browser worker timed out before producing output.")
        .with_provider("producer_compatible")
        .with_code("producer_browser_worker_timeout")
}

pub fn browser_worker_stdin_error(error: &str) -> GatewayError {
    GatewayError::server_error(format!(
        "Failed to write Producer browser worker input: {error}"
    ))
    .with_provider("producer_compatible")
    .with_code("producer_browser_worker_stdin_failed")
}

pub fn browser_worker_spawn_failed_error(
    script_path: &std::path::Path,
    error: &str,
) -> GatewayError {
    GatewayError::server_error(format!(
        "Failed to launch Producer browser worker at {}: {error}",
        script_path.display()
    ))
    .with_provider("producer_compatible")
    .with_code("producer_browser_worker_spawn_failed")
}

pub fn browser_worker_input_serialize_error(error: &str) -> GatewayError {
    GatewayError::server_error(format!(
        "Failed to serialize Producer browser worker input: {error}"
    ))
    .with_provider("producer_compatible")
    .with_code("producer_browser_worker_input_serialize_failed")
}

pub fn should_use_image_generations(body: &Value) -> bool {
    let Some(body_obj) = body.as_object() else {
        return false;
    };

    if let Some(model) = body_obj
        .get("model")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return is_image_model(model);
    }

    extract_image_type(body_obj)
        .map(|value| value.eq_ignore_ascii_case("clip"))
        .unwrap_or(false)
}

pub fn normalize_image_generations(body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    let body_obj = body.as_object().ok_or_else(|| {
        GatewayError::bad_request("Image generation request body must be a JSON object")
    })?;

    if body_obj
        .get("stream")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
    {
        return Err(GatewayError::bad_request(
            "Image generation endpoints do not support `stream: true`",
        )
        .with_code("image_streaming_not_supported"));
    }

    let response_format = image_response_format(body_obj)?;
    if response_format != "url" {
        return Err(GatewayError::bad_request(
            "Producer.ai image generation currently supports only `response_format: \"url\"`.",
        )
        .with_code("unsupported_producer_image_response_format"));
    }

    let output_count = requested_output_count(body_obj);
    if output_count > 1 {
        return Err(GatewayError::bad_request(
            "Producer.ai image generation currently supports only `n = 1` requests.",
        )
        .with_code("unsupported_producer_image_count"));
    }

    let prompt = extract_image_prompt(body_obj).ok_or_else(|| {
        GatewayError::bad_request("Image generation requests require `prompt` or `input`.")
            .with_code("missing_image_prompt")
    })?;
    let image_type = extract_image_type(body_obj).unwrap_or_else(|| "clip".to_string());
    let requested_model = body_obj
        .get("model")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| Some(PRODUCER_IMAGE_DEFAULT_MODEL.to_string()));
    let explicit_session_key = extract_explicit_session_key(body_obj);

    Ok(CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ImagesGenerations,
        requested_model,
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: format!("type: {image_type}\n{prompt}"),
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
        explicit_session_key,
        extra: std::collections::HashMap::new(),
    })
}

pub fn normalize_music_generations(body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    let body_obj = body.as_object().ok_or_else(|| {
        GatewayError::bad_request("Music generation request body must be a JSON object")
    })?;

    if body_obj
        .get("stream")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
    {
        return Err(GatewayError::bad_request(
            "Music generation endpoints do not support `stream: true`",
        )
        .with_code("music_streaming_not_supported"));
    }

    let prompt = extract_prompt(body_obj).ok_or_else(|| {
        GatewayError::bad_request(
            "Music generation requests require one of: prompt, input, lyrics, or parts[].content",
        )
        .with_code("missing_music_prompt")
    })?;

    // Producer music generation keeps model unspecified when the caller omits it.
    let requested_model = body_obj
        .get("model")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let explicit_session_key = extract_explicit_session_key(body_obj);

    Ok(CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::MusicGenerations,
        requested_model,
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text { text: prompt }],
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

pub fn should_use_video_generations(body: &Value) -> bool {
    let Some(body_obj) = body.as_object() else {
        return false;
    };

    if let Some(model) = body_obj
        .get("model")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return is_video_model(model);
    }

    has_any_field(
        body_obj,
        &[
            "clip_id",
            "clipId",
            "song_id",
            "songId",
            "style_image_url",
            "styleImageUrl",
            "likeness_image_url",
            "likenessImageUrl",
            "subject_image_url",
            "subjectImageUrl",
            "render_lyrics",
            "display_lyrics",
            "renderLyrics",
            "displayLyrics",
            "duration_s",
            "durationSeconds",
        ],
    )
}

pub fn normalize_video_generations(body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    let body_obj = body.as_object().ok_or_else(|| {
        GatewayError::bad_request("Video generation request body must be a JSON object")
    })?;

    if body_obj
        .get("stream")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
    {
        return Err(GatewayError::bad_request(
            "Video generation endpoints do not support `stream: true`",
        )
        .with_code("video_streaming_not_supported"));
    }

    let clip_id = extract_clip_id(body_obj).ok_or_else(|| {
        GatewayError::bad_request(
            "Producer.ai music video requests require clip_id (or song_id/songId).",
        )
        .with_code("missing_video_clip_id")
    })?;

    let prompt = extract_video_prompt(body_obj);
    let requested_model = body_obj
        .get("model")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| Some(PRODUCER_VIDEO_DEFAULT_MODEL.to_string()));
    let explicit_session_key = extract_explicit_session_key(body_obj);

    let mut content = vec![ContentPart::Text {
        text: format!("clip_id: {clip_id}"),
    }];
    if let Some(prompt) = prompt {
        content.push(ContentPart::Text { text: prompt });
    }

    Ok(CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::VideosGenerations,
        requested_model,
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content,
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

pub fn build_send_message_request(
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<Value, GatewayError> {
    let body_obj = req.raw_body.as_object().ok_or_else(|| {
        GatewayError::bad_request("Producer.ai request body must be a JSON object")
    })?;

    let mut body = body_obj.clone();
    body.remove("model");
    body.remove("stream");
    body.remove("prompt");
    body.remove("input");
    body.remove("lyrics");
    body.remove("user");

    if !body.contains_key("parts") {
        let prompt = extract_prompt(body_obj).ok_or_else(|| {
            GatewayError::bad_request(
                "Producer.ai music requests require one of: prompt, input, lyrics, or parts[].content",
            )
            .with_code("missing_music_prompt")
        })?;
        body.insert(
            "parts".to_string(),
            Value::Array(vec![json!({
                "part_kind": "user-prompt",
                "content": prompt,
            })]),
        );
    }

    if !body.contains_key("client_context") {
        body.insert(
            "client_context".to_string(),
            json!({
                "current_song_id": null,
                "song_queue": [],
                "project_id": body_obj.get("project_id").cloned().unwrap_or(Value::Null),
                "selected_model": Value::String(model.to_string()),
                "lyrics_id_map": {},
                "ghostwriter_version": "standard",
            }),
        );
    }

    body.entry("model_name".to_string())
        .or_insert_with(|| Value::String(model.to_string()));
    body.entry("mode".to_string())
        .or_insert_with(|| Value::String("standard".to_string()));

    Ok(Value::Object(body))
}

pub fn build_send_message_plan(
    payload: &crate::routing::candidate::ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<RequestPlan, GatewayError> {
    Ok(RequestPlan {
        method: rquest::Method::POST,
        url: build_conversation_url(&payload.base_url),
        query: Vec::new(),
        body: Some(build_send_message_request(req, model)?),
        response_kind: req.endpoint_kind,
    })
}

pub fn build_conversation_url(base_url: &str) -> String {
    format!("{}/__api/conversation", base_url.trim_end_matches('/'))
}

pub fn build_message_stream_url(base_url: &str, job_id: &str) -> String {
    format!(
        "{}/__api/messages/{}/stream",
        base_url.trim_end_matches('/'),
        job_id.trim()
    )
}

pub fn build_session_url(base_url: &str, conversation_id: &str) -> String {
    format!(
        "{}/session/{}",
        base_url.trim_end_matches('/'),
        conversation_id.trim()
    )
}

pub fn build_video_status_url(base_url: &str, job_id: &str) -> String {
    format!(
        "{}{}",
        base_url.trim_end_matches('/'),
        build_video_status_path(job_id)
    )
}

pub fn build_video_status_path(job_id: &str) -> String {
    format!("/__api/music-video/{}/status", job_id.trim())
}

pub fn build_video_detail_path(job_id: &str) -> String {
    format!("/__api/music-video/get/{}", job_id.trim())
}

pub fn build_image_generation_url(base_url: &str) -> String {
    format!("{}/__api/generate/image", base_url.trim_end_matches('/'))
}

pub fn build_clips_library_url(base_url: &str) -> String {
    format!("{}/__api/clips/auth-user", base_url.trim_end_matches('/'))
}

pub fn build_video_library_path() -> &'static str {
    "/library/videos"
}

pub fn build_conversation_request_body(
    prompt: &str,
    conversation_id: Option<&str>,
    client_context: &Value,
    model_name: &str,
) -> Value {
    let mut body = json!({
        "parts": [{ "content": prompt, "part_kind": "user-prompt" }],
        "client_context": client_context,
        "model_name": model_name,
        "mode": "standard",
    });
    if let Some(conversation_id) = conversation_id {
        body["conversation_id"] = Value::String(conversation_id.to_string());
    }
    body
}

pub fn build_image_generation_request(
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<Value, GatewayError> {
    let body_obj = req.raw_body.as_object().ok_or_else(|| {
        GatewayError::bad_request("Producer.ai image request body must be a JSON object")
    })?;

    let prompt = extract_image_prompt(body_obj).ok_or_else(|| {
        GatewayError::bad_request("Producer.ai image requests require `prompt` or `input`.")
            .with_code("missing_image_prompt")
    })?;
    let image_type = extract_image_type(body_obj).unwrap_or_else(|| "clip".to_string());

    let mut body = body_obj.clone();
    body.remove("model");
    body.remove("stream");
    body.remove("response_format");
    body.remove("n");
    body.remove("user");
    body.remove("image_type");
    body.remove("imageType");
    body.remove("input");
    body.insert("prompt".to_string(), Value::String(prompt));
    body.insert("type".to_string(), Value::String(image_type));
    body.entry("model_name".to_string())
        .or_insert_with(|| Value::String(model.to_string()));

    Ok(Value::Object(body))
}

pub fn build_image_generation_response(
    body: &Value,
    req: &CanonicalRelayRequest,
    model: &str,
    auth_token: Option<&str>,
) -> Result<Value, GatewayError> {
    let body_obj = body.as_object().ok_or_else(|| {
        GatewayError::server_error("Producer.ai image response body must be a JSON object")
            .with_provider("producer_compatible")
            .with_code("producer_invalid_image_response")
    })?;
    let image_id = extract_image_id(body_obj)?;
    let image_type = request_image_type(req).unwrap_or_else(|| "clip".to_string());
    let image_url = extract_image_url(body)
        .or_else(|| derive_image_url_from_auth(auth_token, &image_type, &image_id));

    let created = body_obj
        .get("created")
        .cloned()
        .unwrap_or_else(|| json!(current_unix_timestamp()));

    let mut item = Map::new();
    item.insert("image_id".to_string(), Value::String(image_id.clone()));
    item.insert("type".to_string(), Value::String(image_type.clone()));
    if let Some(url) = image_url.clone() {
        item.insert("url".to_string(), Value::String(url));
    }

    let mut response = Map::new();
    response.insert("created".to_string(), created);
    response.insert("data".to_string(), Value::Array(vec![Value::Object(item)]));
    response.insert(
        "object".to_string(),
        Value::String("image.generation".to_string()),
    );
    response.insert(
        "provider".to_string(),
        Value::String("producer.ai".to_string()),
    );
    response.insert("model".to_string(), Value::String(model.to_string()));
    response.insert("image_id".to_string(), Value::String(image_id));
    response.insert("type".to_string(), Value::String(image_type));
    if image_url.is_none() {
        response.insert("upstream_response".to_string(), body.clone());
    }

    Ok(Value::Object(response))
}

pub fn build_video_tool_call_request(
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<Value, GatewayError> {
    let body_obj = req.raw_body.as_object().ok_or_else(|| {
        GatewayError::bad_request("Producer.ai video request body must be a JSON object")
    })?;

    let conversation_id = read_string_fields(body_obj, &["conversation_id", "conversationId"]);
    let project_id = body_obj.get("project_id").cloned().unwrap_or(Value::Null);
    let clip_id = extract_clip_id(body_obj).ok_or_else(|| {
        GatewayError::bad_request(
            "Producer.ai music video requests require clip_id (or song_id/songId).",
        )
        .with_code("missing_video_clip_id")
    })?;
    let prompt = extract_video_prompt(body_obj);

    let mut args = body_obj
        .get("args")
        .and_then(|value| value.as_object())
        .cloned()
        .unwrap_or_default();

    args.entry("clip_id".to_string())
        .or_insert_with(|| Value::String(clip_id.clone()));

    if let Some(prompt) = prompt {
        args.entry("user_message".to_string())
            .or_insert_with(|| Value::String(prompt));
    }

    maybe_insert_string(
        &mut args,
        "style_image_url",
        read_string_fields(
            body_obj,
            &[
                "style_image_url",
                "styleImageUrl",
                "styleImage",
                "style_reference_image_url",
                "styleReferenceImageUrl",
            ],
        ),
    );
    maybe_insert_string(
        &mut args,
        "likeness_image_url",
        read_string_fields(
            body_obj,
            &[
                "likeness_image_url",
                "likenessImageUrl",
                "subject_image_url",
                "subjectImageUrl",
                "subject_image",
                "subjectImage",
            ],
        ),
    );
    maybe_insert_string(
        &mut args,
        "aspect_ratio",
        read_string_fields(body_obj, &["aspect_ratio", "aspectRatio", "size"]),
    );
    maybe_insert_string(
        &mut args,
        "resolution",
        read_string_fields(body_obj, &["resolution"]),
    );
    maybe_insert_bool(
        &mut args,
        "render_lyrics",
        read_bool_fields(
            body_obj,
            &[
                "render_lyrics",
                "display_lyrics",
                "renderLyrics",
                "displayLyrics",
            ],
        ),
    );
    maybe_insert_number(
        &mut args,
        "duration_s",
        read_number_fields(body_obj, &["duration_s", "durationSeconds", "duration"]),
    );

    args.entry("aspect_ratio".to_string())
        .or_insert_with(|| Value::String("9:16".to_string()));
    args.entry("resolution".to_string())
        .or_insert_with(|| Value::String("720p".to_string()));
    args.entry("render_lyrics".to_string())
        .or_insert(Value::Bool(true));

    let mut body = Map::new();
    if let Some(conversation_id) = conversation_id {
        body.insert(
            "conversation_id".to_string(),
            Value::String(conversation_id),
        );
    }
    body.insert(
        "part".to_string(),
        json!({
            "tool_name": "video__create_music_video",
            "args": Value::Object(args),
        }),
    );
    body.insert(
        "client_context".to_string(),
        body_obj.get("client_context").cloned().unwrap_or_else(|| {
            json!({
                "current_song_id": clip_id,
                "song_queue": [clip_id],
                "project_id": project_id,
                "selected_model": model,
                "lyrics_id_map": {},
                "ghostwriter_version": "standard",
            })
        }),
    );

    Ok(Value::Object(body))
}

pub fn extract_video_clip_id(body: &Value) -> Result<String, GatewayError> {
    let body_obj = body.as_object().ok_or_else(|| {
        GatewayError::bad_request("Producer.ai video request body must be a JSON object")
    })?;
    extract_clip_id(body_obj).ok_or_else(|| {
        GatewayError::bad_request(
            "Producer.ai music video requests require clip_id (or song_id/songId).",
        )
        .with_code("missing_video_clip_id")
    })
}

pub fn build_video_bootstrap_prompt(base_url: &str, clip_id: &str) -> String {
    format!(
        "Let's make a music video with the song {}/song/{}",
        base_url.trim_end_matches('/'),
        clip_id.trim()
    )
}

pub fn build_video_client_context(
    body: &Value,
    selected_model: &str,
) -> Result<Value, GatewayError> {
    let body_obj = body.as_object().ok_or_else(|| {
        GatewayError::bad_request("Producer.ai video request body must be a JSON object")
    })?;
    let clip_id = extract_video_clip_id(body)?;
    let project_id = body_obj.get("project_id").cloned().unwrap_or(Value::Null);
    Ok(body_obj.get("client_context").cloned().unwrap_or_else(|| {
        json!({
            "current_song_id": clip_id,
            "song_queue": [{ "id": clip_id }],
            "project_id": project_id,
            "selected_model": selected_model,
            "lyrics_id_map": {},
            "ghostwriter_version": "standard",
        })
    }))
}

pub fn build_video_proposal_prompt(body: &Value) -> Result<String, GatewayError> {
    let body_obj = body.as_object().ok_or_else(|| {
        GatewayError::bad_request("Producer.ai video request body must be a JSON object")
    })?;
    let creative_prompt = extract_video_prompt(body_obj).ok_or_else(|| {
        GatewayError::bad_request(
            "Producer.ai music video requests require one of: prompt, input, user_message, or args.user_message.",
        )
        .with_code("missing_video_prompt")
    })?;
    let aspect_ratio = read_string_fields(body_obj, &["aspect_ratio", "aspectRatio", "size"])
        .unwrap_or_else(|| "16:9".to_string());
    let style_image_url = read_string_fields(
        body_obj,
        &[
            "style_image_url",
            "styleImageUrl",
            "style_reference_image_url",
            "styleReferenceImageUrl",
        ],
    );
    let subject_image_url = read_string_fields(
        body_obj,
        &[
            "likeness_image_url",
            "likenessImageUrl",
            "subject_image_url",
            "subjectImageUrl",
        ],
    );
    let duration_seconds =
        read_number_fields(body_obj, &["duration_s", "durationSeconds", "duration"]).map(|value| {
            value
                .as_f64()
                .map(|entry| {
                    if entry.fract() == 0.0 {
                        format!("{entry:.0}")
                    } else {
                        format!("{entry}")
                    }
                })
                .unwrap_or_else(|| value.to_string())
        });
    let render_lyrics = read_bool_fields(
        body_obj,
        &[
            "render_lyrics",
            "renderLyrics",
            "display_lyrics",
            "displayLyrics",
        ],
    )
    .unwrap_or(false);

    Ok([
        Some("Please propose the music video.".to_string()),
        Some(format!("Vision: {creative_prompt}")),
        Some(format!("Use {aspect_ratio}.")),
        Some(match style_image_url {
            Some(url) => format!("Style reference image: {url}."),
            None => "Style reference image: none.".to_string(),
        }),
        Some(match subject_image_url {
            Some(url) => format!("Subject image: {url}."),
            None => "Subject image: none, generate one.".to_string(),
        }),
        Some(format!(
            "Lyrics on screen: {}.",
            if render_lyrics { "yes" } else { "no" }
        )),
        Some(match duration_seconds {
            Some(seconds) => format!("Duration: use about {seconds} seconds."),
            None => "Duration: choose the strongest section under 60 seconds.".to_string(),
        }),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" "))
}

pub fn choose_video_confirm_prompt(body: &Value, summary: &Value) -> Result<String, GatewayError> {
    let body_obj = body.as_object().ok_or_else(|| {
        GatewayError::bad_request("Producer.ai video request body must be a JSON object")
    })?;
    if let Some(explicit) = read_string_fields(body_obj, &["confirm_prompt", "confirmPrompt"]) {
        return Ok(explicit);
    }

    let proposed_inputs = summary
        .get("tool_calls")
        .and_then(|value| value.as_array())
        .and_then(|entries| {
            entries.iter().find_map(|entry| {
                let object = entry.as_object()?;
                let tool_name = object.get("tool_name")?.as_str()?.trim();
                if tool_name != "video__propose_music_video" {
                    return None;
                }
                object
                    .get("args")
                    .and_then(|value| value.as_object())
                    .and_then(|args| args.get("inputs"))
                    .and_then(|value| value.as_object())
                    .cloned()
            })
        });

    let start_seconds = proposed_inputs
        .as_ref()
        .and_then(|value| read_number_fields(value, &["start_s", "startSeconds", "start"]))
        .or_else(|| read_number_fields(body_obj, &["start_s", "startSeconds", "start"]));
    let duration_seconds = proposed_inputs
        .as_ref()
        .and_then(|value| read_number_fields(value, &["duration_s", "durationSeconds", "duration"]))
        .or_else(|| read_number_fields(body_obj, &["duration_s", "durationSeconds", "duration"]));
    let aspect_ratio = proposed_inputs
        .as_ref()
        .and_then(|value| read_string_fields(value, &["aspect_ratio", "aspectRatio", "size"]))
        .or_else(|| read_string_fields(body_obj, &["aspect_ratio", "aspectRatio", "size"]));
    let resolution = proposed_inputs
        .as_ref()
        .and_then(|value| read_string_fields(value, &["resolution"]))
        .or_else(|| read_string_fields(body_obj, &["resolution"]));

    if start_seconds.is_some() || duration_seconds.is_some() || aspect_ratio.is_some() {
        let mut lines = vec!["Create this exact proposed music video now.".to_string()];
        if let Some(value) = start_seconds {
            lines.push(format!(
                "Keep the current start time at {}s.",
                format_prompt_number(&value)
            ));
        }
        if let Some(value) = duration_seconds {
            lines.push(format!(
                "Keep the duration at {}s.",
                format_prompt_number(&value)
            ));
        }
        if let Some(value) = aspect_ratio {
            lines.push(format!("Keep the aspect ratio at {value}."));
        }
        if let Some(value) = resolution {
            lines.push(format!("Keep the resolution at {value}."));
        }
        lines.push(
            "Do not ask follow-up questions or change the selected song section.".to_string(),
        );
        return Ok(lines.join(" "));
    }

    let mut candidate_texts = Vec::new();
    if let Some(entries) = summary
        .get("suggestions")
        .and_then(|value| value.as_array())
    {
        for entry in entries {
            if let Some(text) = entry
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                candidate_texts.push(text.to_string());
            }
        }
    }
    if let Some(entries) = summary
        .get("message_texts")
        .and_then(|value| value.as_array())
    {
        for entry in entries {
            if let Some(text) = entry
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                candidate_texts.push(text.to_string());
            }
        }
    }

    let preferred = candidate_texts
        .iter()
        .find(|entry| {
            let lower = entry.to_ascii_lowercase();
            lower.contains("create the video")
                || lower.contains("create music video")
                || lower.contains("create video")
        })
        .cloned()
        .or_else(|| {
            candidate_texts
                .iter()
                .find(|entry| entry.to_ascii_lowercase().contains("render"))
                .cloned()
        })
        .or_else(|| {
            candidate_texts
                .iter()
                .find(|entry| entry.to_ascii_lowercase().contains("create"))
                .cloned()
        })
        .or_else(|| {
            candidate_texts
                .iter()
                .find(|entry| entry.to_ascii_lowercase().contains("start"))
                .cloned()
        });

    Ok(preferred.unwrap_or_else(|| "Create the video".to_string()))
}

pub fn summarize_conversation_stream_text(raw_text: &str) -> Result<Value, GatewayError> {
    let mut parser_state = SseParseState::new();
    let mut conversation_id: Option<String> = None;
    let mut tool_calls = Vec::new();
    let mut tool_returns = Vec::new();
    let mut retry_prompts = Vec::new();
    let mut suggestions = Vec::new();
    let mut message_texts = Vec::new();
    let mut final_event_seen = false;

    for raw_line in raw_text.split('\n') {
        let line = raw_line.trim_end_matches('\r');
        if let Some(frame) = parse_sse_line(line, &mut parser_state) {
            handle_conversation_summary_frame(
                &frame,
                &mut conversation_id,
                &mut tool_calls,
                &mut tool_returns,
                &mut retry_prompts,
                &mut suggestions,
                &mut message_texts,
                &mut final_event_seen,
            )?;
        }
    }

    if let Some(frame) = parse_sse_line("", &mut parser_state) {
        handle_conversation_summary_frame(
            &frame,
            &mut conversation_id,
            &mut tool_calls,
            &mut tool_returns,
            &mut retry_prompts,
            &mut suggestions,
            &mut message_texts,
            &mut final_event_seen,
        )?;
    }

    Ok(json!({
        "conversation_id": conversation_id,
        "tool_calls": tool_calls,
        "tool_returns": tool_returns,
        "retry_prompts": retry_prompts,
        "suggestions": suggestions,
        "message_texts": message_texts,
        "final_event_seen": final_event_seen,
    }))
}

pub fn extract_job_id(body: &Value) -> Result<String, GatewayError> {
    body.get("job_id")
        .or_else(|| body.get("jobId"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| {
            GatewayError::server_error("Producer.ai send-message response missing job_id")
                .with_provider("producer_compatible")
                .with_code("producer_missing_job_id")
        })
}

pub fn extract_stream_job_id(body: &Value) -> Result<String, GatewayError> {
    extract_job_id(body).map_err(|error| error.with_code("producer_missing_stream_job_id"))
}

pub async fn accumulate_job_stream(
    response: rquest::Response,
    model: &str,
    job_id: &str,
) -> Result<Value, GatewayError> {
    let provider = "producer_compatible";
    let mut raw_text = String::new();
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| classify_network_error(&error, Some(provider)))?;
        raw_text.push_str(&String::from_utf8_lossy(&chunk));
    }

    parse_producer_music_stream_text(&raw_text, model, job_id)
}

pub fn parse_producer_music_stream_text(
    raw_text: &str,
    model: &str,
    job_id: &str,
) -> Result<Value, GatewayError> {
    let mut parser_state = SseParseState::new();
    let mut events = Vec::new();
    let mut parts = Vec::new();
    let mut suggestions = Vec::new();
    let mut conversation_id: Option<String> = None;
    let mut generated_title: Option<String> = None;
    let mut completed = false;
    let mut final_event_seen = false;

    for raw_line in raw_text.split('\n') {
        let line = raw_line.trim_end_matches('\r');
        if let Some(frame) = parse_sse_line(line, &mut parser_state) {
            handle_sse_frame(
                &frame,
                &mut events,
                &mut parts,
                &mut suggestions,
                &mut conversation_id,
                &mut generated_title,
                &mut completed,
                &mut final_event_seen,
            )?;
        }
    }

    if let Some(frame) = parse_sse_line("", &mut parser_state) {
        handle_sse_frame(
            &frame,
            &mut events,
            &mut parts,
            &mut suggestions,
            &mut conversation_id,
            &mut generated_title,
            &mut completed,
            &mut final_event_seen,
        )?;
    }

    Ok(json!({
        "object": "music.generation",
        "provider": "producer.ai",
        "model": model,
        "job_id": job_id,
        "conversation_id": conversation_id,
        "generated_title": generated_title,
        "completed": completed,
        "final_event_seen": final_event_seen,
        "parts": parts,
        "suggestions": suggestions,
        "events": events,
    }))
}

pub async fn accumulate_tool_call_stream(
    response: rquest::Response,
    model: &str,
    stream_job_id: &str,
    tool_name: &str,
) -> Result<Value, GatewayError> {
    let provider = "producer_compatible";
    let mut buffer = String::new();
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| classify_network_error(&error, Some(provider)))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));
    }

    parse_tool_call_stream_text(&buffer, model, stream_job_id, tool_name)
}

fn handle_sse_frame(
    frame: &SseFrame,
    events: &mut Vec<Value>,
    parts: &mut Vec<Value>,
    suggestions: &mut Vec<Value>,
    conversation_id: &mut Option<String>,
    generated_title: &mut Option<String>,
    completed: &mut bool,
    final_event_seen: &mut bool,
) -> Result<(), GatewayError> {
    let event_name = frame.event_name.clone().unwrap_or_default();
    let data = parse_json_or_string(&frame.data);

    events.push(json!({
        "event": if event_name.is_empty() { Value::Null } else { Value::String(event_name.clone()) },
        "data": data.clone(),
    }));

    match event_name.as_str() {
        "conversation_id" => {
            if let Some(id) = data
                .as_object()
                .and_then(|value| value.get("id"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                *conversation_id = Some(id.to_string());
            }
        }
        "part" => parts.push(data),
        "suggestion" => suggestions.push(data),
        "generated-title" => {
            if let Some(title) = data
                .as_object()
                .and_then(|value| value.get("title"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                *generated_title = Some(title.to_string());
            }
        }
        "complete" => *completed = true,
        "final" => *final_event_seen = true,
        "error" => {
            let message = data
                .as_object()
                .and_then(|value| value.get("message"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("Producer.ai stream returned an error event");
            return Err(GatewayError::server_error(message)
                .with_provider("producer_compatible")
                .with_code("producer_stream_error_event"));
        }
        _ => {}
    }

    Ok(())
}

fn parse_json_or_string(value: &str) -> Value {
    if value.trim().is_empty() {
        Value::Null
    } else if let Ok(parsed) = serde_json::from_str::<Value>(value) {
        parsed
    } else {
        Value::String(value.to_string())
    }
}

fn parse_tool_call_stream_text(
    raw_text: &str,
    model: &str,
    stream_job_id: &str,
    tool_name: &str,
) -> Result<Value, GatewayError> {
    let mut parser_state = SseParseState::new();
    let mut events = Vec::new();
    let mut messages = Vec::new();
    let mut tool_returns = Vec::new();
    let mut video_urls = Vec::new();
    let mut conversation_id: Option<String> = None;
    let mut tool_return_job_id: Option<String> = None;
    let mut final_event_seen = false;

    for raw_line in raw_text.split('\n') {
        let line = raw_line.trim_end_matches('\r');
        if let Some(frame) = parse_sse_line(line, &mut parser_state) {
            handle_tool_call_sse_frame(
                &frame,
                tool_name,
                &mut events,
                &mut messages,
                &mut tool_returns,
                &mut video_urls,
                &mut conversation_id,
                &mut tool_return_job_id,
                &mut final_event_seen,
            )?;
        }
    }

    if let Some(frame) = parse_sse_line("", &mut parser_state) {
        handle_tool_call_sse_frame(
            &frame,
            tool_name,
            &mut events,
            &mut messages,
            &mut tool_returns,
            &mut video_urls,
            &mut conversation_id,
            &mut tool_return_job_id,
            &mut final_event_seen,
        )?;
    }

    let job_id = tool_return_job_id
        .clone()
        .unwrap_or_else(|| stream_job_id.to_string());
    let progress_url = conversation_id
        .as_ref()
        .map(|id| build_session_url("https://www.producer.ai", id));

    Ok(json!({
        "object": "video.generation",
        "provider": "producer.ai",
        "model": model,
        "tool_name": tool_name,
        "job_id": job_id,
        "stream_job_id": stream_job_id,
        "tool_return_job_id": tool_return_job_id,
        "conversation_id": conversation_id,
        "progress_url": progress_url,
        "status_path": build_video_status_path(&job_id),
        "detail_path": build_video_detail_path(&job_id),
        "library_path": "/__api/music-videos/get",
        "video_urls": video_urls,
        "tool_returns": tool_returns,
        "messages": messages,
        "final_event_seen": final_event_seen,
        "events": events,
    }))
}

#[allow(clippy::too_many_arguments)]
pub fn build_video_generation_response(
    base_url: &str,
    request_body: &Value,
    model: &str,
    clip_id: &str,
    conversation_id: &str,
    bootstrap_job_id: &str,
    creative_job_id: &str,
    confirmation_job_id: Option<&str>,
    video_job_id: &str,
    final_url: Option<&str>,
    final_status: &str,
    status_payload: &Value,
    creative_summary: &Value,
    confirmation_summary: Option<&Value>,
) -> Value {
    let completed = final_status == "completed" && final_url.is_some();
    let prompt = build_video_proposal_prompt(request_body).unwrap_or_else(|_| model.to_string());

    json!({
        "object": "video.generation",
        "provider": "producer.ai",
        "created": SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        "accepted": !completed,
        "completed": completed,
        "model": model,
        "prompt": prompt,
        "clip_id": clip_id,
        "conversation_id": conversation_id,
        "bootstrap_job_id": bootstrap_job_id,
        "creative_job_id": creative_job_id,
        "confirmation_job_id": confirmation_job_id,
        "job_id": video_job_id,
        "progress_url": build_session_url(base_url, conversation_id),
        "status_path": build_video_status_path(video_job_id),
        "detail_path": build_video_detail_path(video_job_id),
        "library_path": build_video_library_path(),
        "data": [{
            "kind": "video",
            "url": final_url,
            "mime_type": "video/mp4",
            "preview_url": status_payload
                .get("preview")
                .and_then(|value| value.get("video"))
                .cloned()
                .unwrap_or(Value::Null),
            "aspect_ratio": request_body
                .get("aspect_ratio")
                .or_else(|| request_body.get("aspectRatio"))
                .cloned()
                .unwrap_or(Value::Null),
            "resolution": request_body.get("resolution").cloned().unwrap_or(Value::Null),
            "duration_seconds": request_body
                .get("duration_s")
                .or_else(|| request_body.get("durationSeconds"))
                .or_else(|| request_body.get("duration"))
                .cloned()
                .unwrap_or(Value::Null),
        }],
        "state": final_status,
        "status": status_payload,
        "creative_stream": creative_summary,
        "confirmation_stream": confirmation_summary,
    })
}

fn handle_tool_call_sse_frame(
    frame: &SseFrame,
    tool_name: &str,
    events: &mut Vec<Value>,
    messages: &mut Vec<Value>,
    tool_returns: &mut Vec<Value>,
    video_urls: &mut Vec<String>,
    conversation_id: &mut Option<String>,
    tool_return_job_id: &mut Option<String>,
    final_event_seen: &mut bool,
) -> Result<(), GatewayError> {
    let event_name = frame.event_name.clone().unwrap_or_default();
    let data = parse_json_or_string(&frame.data);

    events.push(json!({
        "event": if event_name.is_empty() { Value::Null } else { Value::String(event_name.clone()) },
        "data": data.clone(),
    }));

    match event_name.as_str() {
        "conversation_id" => {
            if let Some(id) = data
                .as_object()
                .and_then(|value| value.get("id"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                *conversation_id = Some(id.to_string());
            }
        }
        "message" => {
            messages.push(data.clone());
            for content in extract_tool_return_contents(&data, tool_name) {
                if let Some(job_id) = content
                    .get("job_id")
                    .or_else(|| content.get("jobId"))
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    *tool_return_job_id = Some(job_id.to_string());
                }
                if let Some(url) = content
                    .get("url")
                    .or_else(|| content.get("final_video_url"))
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    video_urls.push(url.to_string());
                }
                tool_returns.push(Value::Object(content));
            }
        }
        "final" => *final_event_seen = true,
        "error" => {
            let message = data
                .as_object()
                .and_then(|value| value.get("message"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("Producer.ai tool-call stream returned an error event");
            return Err(GatewayError::server_error(message)
                .with_provider("producer_compatible")
                .with_code("producer_tool_call_stream_error_event"));
        }
        _ => {}
    }

    Ok(())
}

fn handle_conversation_summary_frame(
    frame: &SseFrame,
    conversation_id: &mut Option<String>,
    tool_calls: &mut Vec<Value>,
    tool_returns: &mut Vec<Value>,
    retry_prompts: &mut Vec<String>,
    suggestions: &mut Vec<String>,
    message_texts: &mut Vec<String>,
    final_event_seen: &mut bool,
) -> Result<(), GatewayError> {
    let event_name = frame.event_name.clone().unwrap_or_default();
    let data = parse_json_or_string(&frame.data);

    match event_name.as_str() {
        "conversation_id" => {
            if let Some(id) = data
                .as_object()
                .and_then(|value| value.get("id"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                *conversation_id = Some(id.to_string());
            }
        }
        "part" => {
            let Some(part) = data
                .as_object()
                .and_then(|value| value.get("part"))
                .and_then(|value| value.as_object())
            else {
                return Ok(());
            };
            let part_kind = part
                .get("part_kind")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .unwrap_or_default();
            match part_kind {
                "tool-call" => tool_calls.push(Value::Object(part.clone())),
                "tool-return" => tool_returns.push(Value::Object(part.clone())),
                "retry-prompt" => {
                    if let Some(text) = part
                        .get("content")
                        .and_then(|value| value.as_str())
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                    {
                        retry_prompts.push(text.to_string());
                    }
                }
                "text" => {
                    if let Some(text) = part
                        .get("content")
                        .and_then(|value| value.as_str())
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                    {
                        message_texts.push(text.to_string());
                    }
                }
                _ => {}
            }
        }
        "suggestion" => {
            if let Some(parts) = data
                .as_object()
                .and_then(|value| value.get("parts"))
                .and_then(|value| value.as_array())
            {
                for part in parts {
                    let Some(part) = part.as_object() else {
                        continue;
                    };
                    let part_kind = part
                        .get("part_kind")
                        .and_then(|value| value.as_str())
                        .map(str::trim)
                        .unwrap_or_default();
                    if part_kind == "text" {
                        if let Some(text) = part
                            .get("content")
                            .and_then(|value| value.as_str())
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                        {
                            message_texts.push(text.to_string());
                        }
                    }
                    if part_kind == "tool-call"
                        && part
                            .get("tool_name")
                            .and_then(|value| value.as_str())
                            .map(str::trim)
                            == Some("synthetic__suggest_actions")
                    {
                        if let Some(args) = part.get("args").and_then(|value| value.as_object()) {
                            for value in args.values() {
                                if let Some(text) = value
                                    .as_str()
                                    .map(str::trim)
                                    .filter(|entry| !entry.is_empty())
                                {
                                    suggestions.push(text.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
        "final" => *final_event_seen = true,
        "error" => {
            let message = data
                .as_object()
                .and_then(|value| value.get("message"))
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("Producer.ai conversation stream returned an error event");
            return Err(GatewayError::server_error(message)
                .with_provider("producer_compatible")
                .with_code("producer_stream_error_event"));
        }
        _ => {}
    }

    Ok(())
}

fn format_prompt_number(value: &Value) -> String {
    value
        .as_f64()
        .map(|entry| {
            if entry.fract() == 0.0 {
                format!("{entry:.0}")
            } else {
                format!("{entry}")
            }
        })
        .unwrap_or_else(|| value.to_string())
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
            Value::Object(part_map) => match part_map.get("content") {
                Some(Value::String(text)) => {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        texts.push(trimmed.to_string());
                    }
                }
                Some(Value::Array(values)) => {
                    for value in values {
                        if let Some(text) = value
                            .as_str()
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                        {
                            texts.push(text.to_string());
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    if texts.is_empty() {
        None
    } else {
        Some(texts.join("\n"))
    }
}

fn extract_image_prompt(map: &Map<String, Value>) -> Option<String> {
    for field in ["prompt", "input"] {
        if let Some(text) = map
            .get(field)
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(text.to_string());
        }
    }
    None
}

fn extract_image_type(map: &Map<String, Value>) -> Option<String> {
    read_string_fields(map, &["type", "image_type", "imageType"])
}

fn extract_video_prompt(map: &Map<String, Value>) -> Option<String> {
    for field in ["user_message", "userMessage", "prompt", "input"] {
        if let Some(text) = map
            .get(field)
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(text.to_string());
        }
    }

    map.get("args")
        .and_then(|value| value.as_object())
        .and_then(|args| {
            args.get("user_message")
                .or_else(|| args.get("userMessage"))
                .and_then(|value| value.as_str())
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn extract_image_id(map: &Map<String, Value>) -> Result<String, GatewayError> {
    map.get("image_id")
        .or_else(|| map.get("imageId"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| {
            GatewayError::server_error("Producer.ai image response missing image_id")
                .with_provider("producer_compatible")
                .with_code("producer_missing_image_id")
        })
}

fn extract_clip_id(map: &Map<String, Value>) -> Option<String> {
    read_string_fields(map, &["clip_id", "clipId", "song_id", "songId"]).or_else(|| {
        map.get("args")
            .and_then(|value| value.as_object())
            .and_then(|args| read_string_fields(args, &["clip_id", "clipId", "song_id", "songId"]))
    })
}

fn extract_explicit_session_key(map: &Map<String, Value>) -> Option<String> {
    read_string_fields(map, &["conversation_id", "session_id", "thread_id", "user"])
}

fn request_image_type(req: &CanonicalRelayRequest) -> Option<String> {
    req.raw_body.as_object().and_then(extract_image_type)
}

fn extract_image_url(body: &Value) -> Option<String> {
    if let Some(url) = body
        .as_object()
        .and_then(|value| {
            value
                .get("image_url")
                .or_else(|| value.get("url"))
                .and_then(|value| value.as_str())
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(url.to_string());
    }

    body.get("data")
        .and_then(|value| value.as_array())
        .and_then(|items| items.first())
        .and_then(|value| value.as_object())
        .and_then(|item| {
            item.get("url")
                .or_else(|| item.get("image_url"))
                .and_then(|value| value.as_str())
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn extract_tool_return_contents(data: &Value, tool_name: &str) -> Vec<Map<String, Value>> {
    let Some(parts) = data
        .as_object()
        .and_then(|value| value.get("parts"))
        .and_then(|value| value.as_array())
    else {
        return Vec::new();
    };

    parts
        .iter()
        .filter_map(|part| {
            let part_map = part.as_object()?;
            if part_map
                .get("part_kind")
                .and_then(|value| value.as_str())
                .map(str::trim)
                != Some("tool-return")
            {
                return None;
            }

            let matches_tool = part_map
                .get("tool_name")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .map(|value| value == tool_name)
                .unwrap_or(true);
            if !matches_tool {
                return None;
            }

            part_map
                .get("content")
                .and_then(|value| value.as_object())
                .cloned()
        })
        .collect()
}

fn has_any_field(map: &Map<String, Value>, fields: &[&str]) -> bool {
    fields.iter().any(|field| {
        map.get(*field).is_some()
            || map
                .get("args")
                .and_then(|value| value.as_object())
                .and_then(|args| args.get(*field))
                .is_some()
    })
}

fn is_video_model(model: &str) -> bool {
    let normalized = model.trim().to_lowercase();
    normalized == PRODUCER_VIDEO_DEFAULT_MODEL
        || normalized == "producer:video"
        || normalized == "producer-video"
        || (normalized.starts_with("producer") && normalized.contains("video"))
}

fn is_image_model(model: &str) -> bool {
    let normalized = model.trim().to_lowercase();
    normalized == PRODUCER_IMAGE_DEFAULT_MODEL
        || normalized == "producer:image"
        || normalized == "producer-image"
        || (normalized.starts_with("producer") && normalized.contains("image"))
}

fn image_response_format(map: &Map<String, Value>) -> Result<String, GatewayError> {
    Ok(
        read_string_fields(map, &["response_format", "responseFormat"])
            .unwrap_or_else(|| "url".to_string())
            .to_lowercase(),
    )
}

fn requested_output_count(map: &Map<String, Value>) -> u64 {
    map.get("n")
        .and_then(|value| value.as_u64())
        .or_else(|| {
            map.get("n")
                .and_then(|value| value.as_str())
                .and_then(|value| value.trim().parse::<u64>().ok())
        })
        .unwrap_or(1)
}

fn derive_image_url_from_auth(
    auth_token: Option<&str>,
    image_type: &str,
    image_id: &str,
) -> Option<String> {
    let token = auth_token?.trim();
    if token.is_empty() {
        return None;
    }
    decode_auth_claims(token)?;
    let legacy_bucket = producer_image_bucket(image_type)?;
    if let ProducerImageBucket::UserScoped(bucket_name) = legacy_bucket {
        let _ = bucket_name;
    }

    // Flow Music keeps Supabase only as its auth issuer; generated image
    // assets now live in the public producer-app-public GCS bucket.
    Some(format!(
        "{PRODUCER_PUBLIC_ASSET_BASE_URL}/{}.jpg",
        image_id.trim()
    ))
}

fn decode_auth_claims(token: &str) -> Option<Value> {
    let mut segments = token.split('.');
    let _header = segments.next()?;
    let payload = segments.next()?;
    let decoded = URL_SAFE_NO_PAD.decode(payload.as_bytes()).ok()?;
    serde_json::from_slice(&decoded).ok()
}

fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
}

enum ProducerImageBucket {
    UserScoped(&'static str),
    Space,
}

fn producer_image_bucket(image_type: &str) -> Option<ProducerImageBucket> {
    match image_type.trim().to_ascii_lowercase().as_str() {
        "clip" | "song" => Some(ProducerImageBucket::UserScoped("clips")),
        "playlist" => Some(ProducerImageBucket::UserScoped("playlists")),
        "space" => Some(ProducerImageBucket::Space),
        _ => None,
    }
}

fn read_string_fields(map: &Map<String, Value>, fields: &[&str]) -> Option<String> {
    fields.iter().find_map(|field| {
        map.get(*field)
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn read_bool_fields(map: &Map<String, Value>, fields: &[&str]) -> Option<bool> {
    fields
        .iter()
        .find_map(|field| map.get(*field).and_then(|value| value.as_bool()))
}

fn read_number_fields(map: &Map<String, Value>, fields: &[&str]) -> Option<Value> {
    fields.iter().find_map(|field| {
        map.get(*field).and_then(|value| {
            if value.is_number() {
                Some(value.clone())
            } else {
                value
                    .as_str()
                    .and_then(|raw| raw.trim().parse::<f64>().ok())
                    .and_then(serde_json::Number::from_f64)
                    .map(Value::Number)
            }
        })
    })
}

fn maybe_insert_string(args: &mut Map<String, Value>, key: &str, value: Option<String>) {
    if let Some(value) = value {
        args.entry(key.to_string())
            .or_insert_with(|| Value::String(value));
    }
}

fn maybe_insert_bool(args: &mut Map<String, Value>, key: &str, value: Option<bool>) {
    if let Some(value) = value {
        args.entry(key.to_string()).or_insert(Value::Bool(value));
    }
}

fn maybe_insert_number(args: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    if let Some(value) = value {
        args.entry(key.to_string()).or_insert(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind, ProtocolFamily};
    use crate::routing::candidate::ProviderAccountPayload;
    use crate::upstream::client::UpstreamClient;
    use rquest::Method;
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

    fn make_request(protocol: ProtocolFamily, endpoint: EndpointKind) -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: protocol,
            endpoint_kind: endpoint,
            requested_model: Some("test-model".to_string()),
            stream: false,
            messages: vec![],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }

    fn assert_request_plan(
        plan: &RequestPlan,
        expected_url: &str,
        expected_response_kind: EndpointKind,
    ) {
        assert_eq!(plan.method, Method::POST);
        assert_eq!(plan.url, expected_url);
        assert_eq!(plan.response_kind, expected_response_kind);
        assert!(plan.query.is_empty());
    }

    #[test]
    fn producer_unsupported_request_plan_error_matches_contract() {
        let err = unsupported_request_plan_error();
        assert_eq!(err.http_status, Some(400));
        assert_eq!(err.provider_name.as_deref(), Some("producer_compatible"));
        assert_eq!(err.code.as_deref(), Some("unsupported_producer_endpoint"));
    }

    #[test]
    fn plan_producer_chat_endpoint_rejected_locally() {
        let payload = make_payload("producer_compatible", "https://www.producer.ai");
        let req = make_request(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
        let err = UpstreamClient::build_request_plan(&payload, &req, PRODUCER_DEFAULT_MODEL, false)
            .expect_err("producer chat requests should be rejected");
        assert_eq!(err.http_status, Some(400));
        assert_eq!(err.code.as_deref(), Some("unsupported_producer_endpoint"));
    }

    #[test]
    fn producer_unsupported_media_endpoint_error_matches_contract() {
        let err = unsupported_media_endpoint_error();
        assert_eq!(err.http_status, Some(400));
        assert_eq!(err.provider_name.as_deref(), Some("producer_compatible"));
        assert_eq!(err.code.as_deref(), Some("unsupported_producer_endpoint"));
    }

    #[test]
    fn normalize_image_generations_defaults_clip_type_and_model() {
        let req = normalize_image_generations(json!({
            "prompt": "retro chrome mascot"
        }))
        .unwrap();

        assert_eq!(req.endpoint_kind, EndpointKind::ImagesGenerations);
        assert_eq!(
            req.requested_model.as_deref(),
            Some(PRODUCER_IMAGE_DEFAULT_MODEL)
        );
        assert_eq!(
            req.messages[0].text_content(),
            "type: clip\nretro chrome mascot"
        );
    }

    #[test]
    fn normalize_image_generations_rejects_b64_response_format() {
        let err = normalize_image_generations(json!({
            "prompt": "retro chrome mascot",
            "response_format": "b64_json"
        }))
        .expect_err("unsupported Producer image formats should fail");

        assert_eq!(err.http_status, Some(400));
        assert_eq!(
            err.code.as_deref(),
            Some("unsupported_producer_image_response_format")
        );
    }

    #[test]
    fn build_image_generation_request_uses_site_native_type_field() {
        let req = normalize_image_generations(json!({
            "model": "producer:image",
            "prompt": "holographic album cover",
            "imageType": "clip",
            "response_format": "url"
        }))
        .unwrap();

        let body = build_image_generation_request(&req, PRODUCER_IMAGE_DEFAULT_MODEL).unwrap();
        assert_eq!(body["prompt"], "holographic album cover");
        assert_eq!(body["type"], "clip");
        assert_eq!(body["model_name"], PRODUCER_IMAGE_DEFAULT_MODEL);
        assert!(body.get("response_format").is_none());
    }

    #[test]
    fn build_image_generation_response_derives_public_url_from_session_jwt() {
        let claims = URL_SAFE_NO_PAD.encode(
            json!({
                "iss": "https://demo-project.supabase.co/auth/v1",
                "sub": "user-123"
            })
            .to_string(),
        );
        let token = format!("e30.{claims}.sig");
        let req = normalize_image_generations(json!({
            "prompt": "holographic album cover",
            "type": "clip"
        }))
        .unwrap();

        let response = build_image_generation_response(
            &json!({
                "image_id": "img-42"
            }),
            &req,
            PRODUCER_IMAGE_DEFAULT_MODEL,
            Some(&token),
        )
        .unwrap();

        assert_eq!(response["object"], "image.generation");
        assert_eq!(response["data"][0]["image_id"], "img-42");
        assert_eq!(
            response["data"][0]["url"],
            "https://storage.googleapis.com/producer-app-public/assets/img-42.jpg"
        );
    }

    #[test]
    fn missing_browser_worker_result_error_matches_contract() {
        let err = missing_browser_worker_result_error();
        assert_eq!(err.http_status, Some(500));
        assert_eq!(
            err.code.as_deref(),
            Some("producer_browser_worker_missing_result")
        );
        assert_eq!(
            err.message.as_str(),
            "Producer browser worker reported success without a result payload."
        );
    }

    #[test]
    fn empty_browser_worker_output_error_formats_stderr_fallback() {
        let with_stderr = empty_browser_worker_output_error("permission denied");
        assert_eq!(with_stderr.http_status, Some(500));
        assert_eq!(
            with_stderr.code.as_deref(),
            Some("producer_browser_worker_empty_output")
        );
        assert_eq!(
            with_stderr.message.as_str(),
            "Producer browser worker did not return JSON output. stderr: permission denied"
        );

        let empty = empty_browser_worker_output_error("");
        assert_eq!(
            empty.message.as_str(),
            "Producer browser worker did not return JSON output. stderr: <empty>"
        );
    }

    #[test]
    fn browser_worker_output_parse_error_formats_error_and_stdout() {
        let err = browser_worker_output_parse_error("expected value", "{\"oops\":");
        assert_eq!(err.http_status, Some(500));
        assert_eq!(
            err.code.as_deref(),
            Some("producer_browser_worker_output_parse_failed")
        );
        assert_eq!(
            err.message.as_str(),
            "Failed to parse Producer browser worker output: expected value. stdout: {\"oops\":"
        );
    }

    #[test]
    fn browser_worker_wait_failed_error_formats_cause() {
        let err = browser_worker_wait_failed_error("The pipe has been ended");
        assert_eq!(err.http_status, Some(500));
        assert_eq!(
            err.code.as_deref(),
            Some("producer_browser_worker_wait_failed")
        );
        assert_eq!(
            err.message.as_str(),
            "Producer browser worker failed before producing output: The pipe has been ended"
        );
    }

    #[test]
    fn browser_worker_timeout_error_matches_contract() {
        let err = browser_worker_timeout_error();
        assert_eq!(err.http_status, Some(500));
        assert_eq!(err.code.as_deref(), Some("producer_browser_worker_timeout"));
        assert_eq!(
            err.message.as_str(),
            "Producer browser worker timed out before producing output."
        );
    }

    #[test]
    fn browser_worker_stdin_error_formats_cause() {
        let err = browser_worker_stdin_error("broken pipe");
        assert_eq!(err.http_status, Some(500));
        assert_eq!(
            err.code.as_deref(),
            Some("producer_browser_worker_stdin_failed")
        );
        assert_eq!(
            err.message.as_str(),
            "Failed to write Producer browser worker input: broken pipe"
        );
    }

    #[test]
    fn browser_worker_spawn_failed_error_formats_script_path_and_cause() {
        let err = browser_worker_spawn_failed_error(
            std::path::Path::new("C:/tmp/producer-browser-worker.mjs"),
            "The system cannot find the file specified. (os error 2)",
        );
        assert_eq!(err.http_status, Some(500));
        assert_eq!(
            err.code.as_deref(),
            Some("producer_browser_worker_spawn_failed")
        );
        assert_eq!(
            err.message.as_str(),
            "Failed to launch Producer browser worker at C:/tmp/producer-browser-worker.mjs: The system cannot find the file specified. (os error 2)"
        );
    }

    #[test]
    fn browser_worker_input_serialize_error_formats_cause() {
        let err = browser_worker_input_serialize_error("missing field `prompt`");
        assert_eq!(err.http_status, Some(500));
        assert_eq!(
            err.code.as_deref(),
            Some("producer_browser_worker_input_serialize_failed")
        );
        assert_eq!(
            err.message.as_str(),
            "Failed to serialize Producer browser worker input: missing field `prompt`"
        );
    }

    #[test]
    fn gemini_business_image_model_is_not_claimed_by_producer_router() {
        assert!(!should_use_image_generations(&json!({
            "model": "gemini-2.5-flash-image-preview",
            "prompt": "launch poster"
        })));
    }

    #[test]
    fn normalize_music_generations_leaves_model_unspecified_when_absent() {
        let req = normalize_music_generations(json!({
            "prompt": "dreamy synthpop with airy vocals"
        }))
        .unwrap();

        assert_eq!(req.endpoint_kind, EndpointKind::MusicGenerations);
        assert!(req.requested_model.is_none());
        assert_eq!(
            req.messages[0].text_content(),
            "dreamy synthpop with airy vocals"
        );
    }

    #[test]
    fn build_send_message_request_uses_prompt_when_parts_missing() {
        let req = normalize_music_generations(json!({
            "prompt": "lo-fi piano with rain ambience"
        }))
        .unwrap();

        let body = build_send_message_request(&req, PRODUCER_DEFAULT_MODEL).unwrap();
        assert_eq!(body["model_name"], PRODUCER_DEFAULT_MODEL);
        assert_eq!(body["mode"], "standard");
        assert_eq!(body["parts"][0]["part_kind"], "user-prompt");
        assert_eq!(
            body["parts"][0]["content"],
            "lo-fi piano with rain ambience"
        );
    }

    #[test]
    fn build_send_message_request_preserves_explicit_parts() {
        let req = normalize_music_generations(json!({
            "model": "producer:standard",
            "parts": [
                {
                    "part_kind": "user-prompt",
                    "content": "cinematic trailer drums"
                }
            ]
        }))
        .unwrap();

        let body = build_send_message_request(&req, PRODUCER_DEFAULT_MODEL).unwrap();
        assert_eq!(body["parts"][0]["content"], "cinematic trailer drums");
    }

    #[test]
    fn build_send_message_plan_uses_conversation_endpoint() {
        let payload = make_payload("producer_compatible", "https://producer.ai/");
        let req = normalize_music_generations(json!({
            "prompt": "lo-fi piano with rain ambience"
        }))
        .unwrap();

        let plan = build_send_message_plan(&payload, &req, PRODUCER_DEFAULT_MODEL)
            .expect("producer send message plan");
        assert_request_plan(
            &plan,
            "https://producer.ai/__api/conversation",
            EndpointKind::MusicGenerations,
        );
        assert_eq!(
            plan.body.as_ref().expect("producer send message plan body"),
            &build_send_message_request(&req, PRODUCER_DEFAULT_MODEL)
                .expect("producer send message body"),
        );
    }

    #[test]
    fn build_conversation_url_trims_trailing_slash() {
        assert_eq!(
            build_conversation_url("https://producer.ai/"),
            "https://producer.ai/__api/conversation"
        );
        assert_eq!(
            build_conversation_url("https://producer.ai"),
            "https://producer.ai/__api/conversation"
        );
    }

    #[test]
    fn build_image_generation_url_trims_trailing_slash() {
        assert_eq!(
            build_image_generation_url("https://producer.ai/"),
            "https://producer.ai/__api/generate/image"
        );
        assert_eq!(
            build_image_generation_url("https://producer.ai"),
            "https://producer.ai/__api/generate/image"
        );
    }

    #[test]
    fn build_clips_library_url_trims_trailing_slash() {
        assert_eq!(
            build_clips_library_url("https://www.flowmusic.app/"),
            "https://www.flowmusic.app/__api/clips/auth-user"
        );
    }

    #[test]
    fn build_message_stream_url_uses_job_id_path() {
        assert_eq!(
            build_message_stream_url("https://producer.ai/", "job-123"),
            "https://producer.ai/__api/messages/job-123/stream"
        );
    }

    #[test]
    fn build_session_url_uses_conversation_id_path() {
        assert_eq!(
            build_session_url("https://producer.ai/", "conv-123"),
            "https://producer.ai/session/conv-123"
        );
        assert_eq!(
            build_session_url("https://producer.ai", "conv-123"),
            "https://producer.ai/session/conv-123"
        );
    }

    #[test]
    fn build_video_status_url_uses_job_id_path() {
        assert_eq!(
            build_video_status_url("https://producer.ai/", "job-123"),
            "https://producer.ai/__api/music-video/job-123/status"
        );
    }

    #[test]
    fn build_video_status_path_uses_job_id_path() {
        assert_eq!(
            build_video_status_path("job-123"),
            "/__api/music-video/job-123/status"
        );
    }

    #[test]
    fn build_video_detail_path_uses_job_id_path() {
        assert_eq!(
            build_video_detail_path("job-123"),
            "/__api/music-video/get/job-123"
        );
    }

    #[test]
    fn build_video_library_path_matches_canonical_route() {
        assert_eq!(build_video_library_path(), "/library/videos");
    }

    #[test]
    fn build_conversation_request_body_uses_prompt_and_standard_mode() {
        let body = build_conversation_request_body(
            "cinematic trailer drums",
            None,
            &json!({ "project_id": "proj-1" }),
            PRODUCER_DEFAULT_MODEL,
        );
        assert_eq!(body["parts"][0]["part_kind"], "user-prompt");
        assert_eq!(body["parts"][0]["content"], "cinematic trailer drums");
        assert_eq!(body["client_context"]["project_id"], "proj-1");
        assert_eq!(body["model_name"], PRODUCER_DEFAULT_MODEL);
        assert_eq!(body["mode"], "standard");
        assert!(body.get("conversation_id").is_none());
    }

    #[test]
    fn build_conversation_request_body_preserves_conversation_id_when_present() {
        let body = build_conversation_request_body(
            "follow-up idea",
            Some("conv-123"),
            &json!({ "project_id": "proj-2" }),
            PRODUCER_DEFAULT_MODEL,
        );
        assert_eq!(body["conversation_id"], "conv-123");
    }

    #[test]
    fn extract_job_id_reads_camel_or_snake_case() {
        assert_eq!(
            extract_job_id(&json!({ "job_id": "job_1" })).unwrap(),
            "job_1"
        );
        assert_eq!(
            extract_job_id(&json!({ "jobId": "job_2" })).unwrap(),
            "job_2"
        );
    }

    #[test]
    fn normalize_video_generations_defaults_model_and_song_alias() {
        let req = normalize_video_generations(json!({
            "songId": "song-123",
            "user_message": "anime cyberpunk performance video"
        }))
        .unwrap();

        assert_eq!(req.endpoint_kind, EndpointKind::VideosGenerations);
        assert_eq!(
            req.requested_model.as_deref(),
            Some(PRODUCER_VIDEO_DEFAULT_MODEL)
        );
        assert_eq!(
            req.messages[0].text_content(),
            "clip_id: song-123\nanime cyberpunk performance video"
        );
    }

    #[test]
    fn build_video_tool_call_request_uses_real_web_field_names() {
        let req = normalize_video_generations(json!({
            "clip_id": "clip-9",
            "prompt": "dreamy stage lights",
            "subjectImageUrl": "https://example.com/subject.png",
            "style_image_url": "https://example.com/style.png",
            "displayLyrics": false,
            "durationSeconds": 42,
            "conversation_id": "conv-video-1"
        }))
        .unwrap();

        let body = build_video_tool_call_request(&req, PRODUCER_VIDEO_DEFAULT_MODEL).unwrap();
        assert_eq!(body["conversation_id"], "conv-video-1");
        assert_eq!(body["part"]["tool_name"], "video__create_music_video");
        assert_eq!(body["part"]["args"]["clip_id"], "clip-9");
        assert_eq!(body["part"]["args"]["user_message"], "dreamy stage lights");
        assert_eq!(
            body["part"]["args"]["likeness_image_url"],
            "https://example.com/subject.png"
        );
        assert_eq!(
            body["part"]["args"]["style_image_url"],
            "https://example.com/style.png"
        );
        assert_eq!(body["part"]["args"]["render_lyrics"], false);
        assert_eq!(body["part"]["args"]["duration_s"], 42);
        assert_eq!(body["part"]["args"]["aspect_ratio"], "9:16");
        assert_eq!(body["part"]["args"]["resolution"], "720p");
        assert_eq!(body["client_context"]["current_song_id"], "clip-9");
    }

    #[test]
    fn build_video_proposal_prompt_matches_http_orchestration_shape() {
        let prompt = build_video_proposal_prompt(&json!({
            "clip_id": "clip-9",
            "prompt": "chrome silhouettes in neon rain",
            "aspect_ratio": "9:16",
            "duration_s": 60,
            "render_lyrics": false,
        }))
        .unwrap();
        assert!(prompt.contains("Please propose the music video."));
        assert!(prompt.contains("Vision: chrome silhouettes in neon rain"));
        assert!(prompt.contains("Use 9:16."));
        assert!(prompt.contains("Lyrics on screen: no."));
        assert!(prompt.contains("Duration: use about 60 seconds."));
    }

    #[test]
    fn choose_video_confirm_prompt_prefers_proposed_inputs() {
        let prompt = choose_video_confirm_prompt(
            &json!({
                "clip_id": "clip-9",
                "aspect_ratio": "9:16",
                "duration_s": 60,
                "resolution": "720p",
            }),
            &json!({
                "tool_calls": [{
                    "tool_name": "video__propose_music_video",
                    "args": {
                        "inputs": {
                            "start_s": 0,
                            "duration_s": 60,
                            "aspect_ratio": "9:16",
                            "resolution": "720p"
                        }
                    }
                }]
            }),
        )
        .unwrap();
        assert!(prompt.contains("Create this exact proposed music video now."));
        assert!(prompt.contains("Keep the current start time at 0s."));
        assert!(prompt.contains("Keep the duration at 60s."));
        assert!(prompt.contains("Keep the aspect ratio at 9:16."));
        assert!(prompt.contains("Keep the resolution at 720p."));
    }

    #[test]
    fn summarize_conversation_stream_text_extracts_tools_and_suggestions() {
        let summary = summarize_conversation_stream_text(
            [
                "event: conversation_id",
                "data: {\"id\":\"conv-video-1\"}",
                "",
                "event: part",
                "data: {\"part\":{\"part_kind\":\"tool-call\",\"tool_name\":\"video__propose_music_video\",\"args\":{\"inputs\":{\"start_s\":0}}}}",
                "",
                "event: part",
                "data: {\"part\":{\"part_kind\":\"tool-return\",\"tool_name\":\"video__create_music_video\",\"content\":{\"job_id\":\"mv-job-1\"}}}",
                "",
                "event: suggestion",
                "data: {\"parts\":[{\"part_kind\":\"tool-call\",\"tool_name\":\"synthetic__suggest_actions\",\"args\":{\"action1\":\"Start the render\"}}]}",
                "",
                "event: final",
                "data: {}",
                "",
            ]
            .join("\n")
            .as_str(),
        )
        .unwrap();

        assert_eq!(summary["conversation_id"], "conv-video-1");
        assert_eq!(
            summary["tool_calls"][0]["tool_name"],
            "video__propose_music_video"
        );
        assert_eq!(
            summary["tool_returns"][0]["tool_name"],
            "video__create_music_video"
        );
        assert_eq!(summary["tool_returns"][0]["content"]["job_id"], "mv-job-1");
        assert_eq!(summary["suggestions"][0], "Start the render");
        assert_eq!(summary["final_event_seen"], true);
    }

    #[test]
    fn parse_producer_music_stream_text_extracts_completed_contract() {
        let parsed = parse_producer_music_stream_text(
            [
                "event: conversation_id",
                "data: {\"id\":\"conv-music-1\"}",
                "",
                "event: part",
                "data: {\"kind\":\"lyric\",\"text\":\"first line\"}",
                "",
                "event: suggestion",
                "data: {\"label\":\"save to favorites\"}",
                "",
                "event: generated-title",
                "data: {\"title\":\"Neon Dreams\"}",
                "",
                "event: complete",
                "data: {}",
                "",
                "event: final",
                "data: {}",
                "",
            ]
            .join("\n")
            .as_str(),
            PRODUCER_DEFAULT_MODEL,
            "music-job-1",
        )
        .expect("music stream should parse");

        assert_eq!(parsed["object"], "music.generation");
        assert_eq!(parsed["provider"], "producer.ai");
        assert_eq!(parsed["model"], PRODUCER_DEFAULT_MODEL);
        assert_eq!(parsed["job_id"], "music-job-1");
        assert_eq!(parsed["conversation_id"], "conv-music-1");
        assert_eq!(parsed["generated_title"], "Neon Dreams");
        assert_eq!(parsed["completed"], true);
        assert_eq!(parsed["final_event_seen"], true);
        assert_eq!(parsed["parts"][0]["text"], "first line");
        assert_eq!(parsed["suggestions"][0]["label"], "save to favorites");
    }

    #[test]
    fn parse_producer_music_stream_text_rejects_error_event_contract() {
        let error = parse_producer_music_stream_text(
            [
                "event: error",
                "data: {\"message\":\"captcha required\"}",
                "",
            ]
            .join("\n")
            .as_str(),
            PRODUCER_DEFAULT_MODEL,
            "music-job-2",
        )
        .expect_err("error event should fail");

        assert_eq!(error.http_status, Some(500));
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
        assert_eq!(error.code.as_deref(), Some("producer_stream_error_event"));
        assert_eq!(error.message, "captcha required");
    }

    #[test]
    fn gemini_canvas_video_model_is_not_claimed_by_producer_router() {
        assert!(!should_use_video_generations(&json!({
            "model": "gemini-canvas-video-preview",
            "prompt": "launch video"
        })));
    }

    #[test]
    fn parse_tool_call_stream_text_extracts_music_video_job_id() {
        let parsed = parse_tool_call_stream_text(
            [
                "event: conversation_id",
                "data: {\"id\":\"conv-video-1\"}",
                "",
                "event: message",
                "data: {\"kind\":\"request\",\"parts\":[{\"part_kind\":\"tool-return\",\"tool_name\":\"video__create_music_video\",\"content\":{\"job_id\":\"mv-job-77\"}}]}",
                "",
                "event: final",
                "data: {}",
                "",
            ]
            .join("\n")
            .as_str(),
            PRODUCER_VIDEO_DEFAULT_MODEL,
            "stream-job-1",
            "video__create_music_video",
        )
        .unwrap();

        assert_eq!(parsed["object"], "video.generation");
        assert_eq!(parsed["job_id"], "mv-job-77");
        assert_eq!(parsed["stream_job_id"], "stream-job-1");
        assert_eq!(parsed["conversation_id"], "conv-video-1");
        assert_eq!(parsed["status_path"], "/__api/music-video/mv-job-77/status");
    }

    #[test]
    fn build_video_generation_response_preserves_completed_contract() {
        let body = build_video_generation_response(
            "https://www.producer.ai",
            &json!({
                "prompt": "launch trailer",
                "aspect_ratio": "16:9",
                "resolution": "1080p",
                "duration_s": 12
            }),
            PRODUCER_VIDEO_DEFAULT_MODEL,
            "clip-1",
            "conv-video-1",
            "bootstrap-job-1",
            "creative-job-1",
            Some("confirm-job-1"),
            "video-job-1",
            Some("https://cdn.example.com/music-video/video-job-1/final.mp4"),
            "completed",
            &json!({
                "status": "completed",
                "preview": { "video": "https://cdn.example.com/preview.mp4" }
            }),
            &json!({ "tool_calls": [] }),
            Some(&json!({ "tool_returns": [] })),
        );

        assert_eq!(body["object"], "video.generation");
        assert_eq!(body["accepted"], false);
        assert_eq!(body["completed"], true);
        assert_eq!(body["model"], PRODUCER_VIDEO_DEFAULT_MODEL);
        assert_eq!(
            body["prompt"],
            "Please propose the music video. Vision: launch trailer Use 16:9. Style reference image: none. Subject image: none, generate one. Lyrics on screen: no. Duration: use about 12 seconds."
        );
        assert_eq!(body["clip_id"], "clip-1");
        assert_eq!(body["conversation_id"], "conv-video-1");
        assert_eq!(body["bootstrap_job_id"], "bootstrap-job-1");
        assert_eq!(body["creative_job_id"], "creative-job-1");
        assert_eq!(body["confirmation_job_id"], "confirm-job-1");
        assert_eq!(body["job_id"], "video-job-1");
        assert_eq!(
            body["progress_url"],
            "https://www.producer.ai/session/conv-video-1"
        );
        assert_eq!(body["status_path"], "/__api/music-video/video-job-1/status");
        assert_eq!(body["detail_path"], "/__api/music-video/get/video-job-1");
        assert_eq!(body["library_path"], "/library/videos");
        assert_eq!(
            body["data"][0]["url"],
            "https://cdn.example.com/music-video/video-job-1/final.mp4"
        );
        assert_eq!(
            body["data"][0]["preview_url"],
            "https://cdn.example.com/preview.mp4"
        );
        assert_eq!(body["data"][0]["aspect_ratio"], "16:9");
        assert_eq!(body["data"][0]["resolution"], "1080p");
        assert_eq!(body["data"][0]["duration_seconds"], 12);
        assert_eq!(body["state"], "completed");
    }

    #[test]
    fn build_video_generation_response_preserves_pending_without_confirmation_contract() {
        let body = build_video_generation_response(
            "https://www.producer.ai",
            &json!({
                "resolution": "720p",
                "durationSeconds": 8
            }),
            PRODUCER_VIDEO_DEFAULT_MODEL,
            "clip-2",
            "conv-video-2",
            "bootstrap-job-2",
            "creative-job-2",
            None,
            "video-job-2",
            None,
            "processing",
            &json!({
                "state": { "status": "processing" }
            }),
            &json!({ "events": [] }),
            None,
        );

        assert_eq!(body["accepted"], true);
        assert_eq!(body["completed"], false);
        assert_eq!(body["confirmation_job_id"], serde_json::Value::Null);
        assert_eq!(body["data"][0]["url"], serde_json::Value::Null);
        assert_eq!(body["data"][0]["preview_url"], serde_json::Value::Null);
        assert_eq!(body["data"][0]["resolution"], "720p");
        assert_eq!(body["data"][0]["duration_seconds"], 8);
        assert_eq!(body["prompt"], PRODUCER_VIDEO_DEFAULT_MODEL);
        assert_eq!(body["state"], "processing");
        assert_eq!(body["confirmation_stream"], serde_json::Value::Null);
    }
}
