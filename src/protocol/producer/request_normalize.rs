use serde_json::{Map, Value};

use super::{
    extract_clip_id, extract_image_prompt, extract_image_type, extract_prompt,
    extract_video_prompt, read_string_fields, PRODUCER_IMAGE_DEFAULT_MODEL,
    PRODUCER_VIDEO_DEFAULT_MODEL,
};
use crate::error::GatewayError;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
};

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

    Ok(canonical_request(
        body,
        EndpointKind::ImagesGenerations,
        requested_model,
        vec![ContentPart::Text {
            text: format!("type: {image_type}\n{prompt}"),
        }],
        explicit_session_key,
    ))
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

    Ok(canonical_request(
        body,
        EndpointKind::MusicGenerations,
        requested_model,
        vec![ContentPart::Text { text: prompt }],
        explicit_session_key,
    ))
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

    Ok(canonical_request(
        body,
        EndpointKind::VideosGenerations,
        requested_model,
        content,
        explicit_session_key,
    ))
}

fn canonical_request(
    raw_body: Value,
    endpoint_kind: EndpointKind,
    requested_model: Option<String>,
    content: Vec<ContentPart>,
    explicit_session_key: Option<String>,
) -> CanonicalRelayRequest {
    CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind,
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
        raw_body,
        previous_response_id: None,
        explicit_session_key,
        extra: std::collections::HashMap::new(),
    }
}

fn extract_explicit_session_key(map: &Map<String, Value>) -> Option<String> {
    read_string_fields(map, &["conversation_id", "session_id", "thread_id", "user"])
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
