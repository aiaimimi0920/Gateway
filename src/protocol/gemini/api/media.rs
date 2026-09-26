//! Official media model selection and request normalization.

use base64::Engine;
use serde_json::{json, Value};

use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::gemini_canvas as legacy;

use super::request::pack_request;

mod audio;
mod responses;

pub use audio::{build_audio_binary_response, extract_audio_from_generate_content_response};
pub use responses::{
    build_music_generation_accepted_response, build_music_generation_response,
    build_openai_images_response_from_bytes, build_video_generation_accepted_response,
    build_video_generation_response,
};

const DEFAULT_ASPECT_RATIO: &str = "1:1";
const DEFAULT_VIDEO_ASPECT_RATIO: &str = "16:9";

pub fn resolve_official_image_model(model: &str) -> Result<&'static str, GatewayError> {
    match model {
        legacy::GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL | legacy::GEMINI_25_FLASH_IMAGE_MODEL => {
            Ok(legacy::GEMINI_CANVAS_OFFICIAL_IMAGE_MODEL)
        }
        legacy::GEMINI_31_FLASH_IMAGE_PREVIEW_MODEL => {
            Ok(legacy::GEMINI_CANVAS_OFFICIAL_IMAGE_MODEL_PREVIEW)
        }
        _ => Err(GatewayError::bad_request(format!(
            "Unsupported Gemini official image model '{model}'."
        ))
        .with_code("unsupported_gemini_official_image_model")),
    }
}

pub fn resolve_official_music_model(model: &str) -> Result<&'static str, GatewayError> {
    match model {
        legacy::GEMINI_CANVAS_MUSIC_PREVIEW_MODEL => Ok(legacy::GEMINI_CANVAS_OFFICIAL_MUSIC_MODEL),
        _ => Err(GatewayError::bad_request(format!(
            "Unsupported Gemini official music model '{model}'."
        ))
        .with_code("unsupported_gemini_official_music_model")),
    }
}

pub fn resolve_official_video_model(model: &str) -> Result<&'static str, GatewayError> {
    match model {
        legacy::GEMINI_CANVAS_VIDEO_PREVIEW_MODEL => Ok(legacy::GEMINI_CANVAS_OFFICIAL_VIDEO_MODEL),
        _ => Err(GatewayError::bad_request(format!(
            "Unsupported Gemini official video model '{model}'."
        ))
        .with_code("unsupported_gemini_official_video_model")),
    }
}

pub fn requested_output_count(req: &CanonicalRelayRequest) -> usize {
    req.raw_body
        .get("n")
        .and_then(Value::as_u64)
        .map(|value| value.max(1) as usize)
        .unwrap_or(1)
}

pub fn prefers_url_response(req: &CanonicalRelayRequest) -> Result<bool, GatewayError> {
    let Some(value) = req.raw_body.get("response_format") else {
        return Ok(false);
    };
    let Some(format) = value.as_str() else {
        return Err(
            GatewayError::bad_request("response_format must be a string when provided.")
                .with_code("invalid_gemini_official_image_response_format"),
        );
    };

    match format {
        "url" => Ok(true),
        "b64_json" => Ok(false),
        _ => Err(GatewayError::bad_request(
            "Gemini official image endpoints currently support response_format=url or b64_json.",
        )
        .with_code("unsupported_gemini_official_image_response_format")),
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
        "1024x1536" | "2:3" => "2:3".to_string(),
        "1536x1024" | "3:2" => "3:2".to_string(),
        "1024x1365" | "3:4" => "3:4".to_string(),
        "1365x1024" | "4:3" => "4:3".to_string(),
        "1024x1280" | "4:5" => "4:5".to_string(),
        "1280x1024" | "5:4" => "5:4".to_string(),
        "auto" => "auto".to_string(),
        _ if is_ratio_string(&size) => size,
        _ => DEFAULT_ASPECT_RATIO.to_string(),
    }
}

pub fn video_aspect_ratio_from_request(req: &CanonicalRelayRequest) -> String {
    if read_optional_string(&req.raw_body, &["size", "aspect_ratio"]).is_some() {
        return aspect_ratio_from_request(req);
    }
    DEFAULT_VIDEO_ASPECT_RATIO.to_string()
}

pub fn build_text_request_body(req: &CanonicalRelayRequest, model: &str) -> Value {
    let mut body = pack_request(req, model, false);
    strip_common_generation_fields(&mut body);
    body
}

pub fn build_tts_request_body(req: &CanonicalRelayRequest, model: &str) -> Value {
    let mut body = build_text_request_body(req, model);
    let request_voice = req
        .raw_body
        .get("voice")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let generation_config = body
        .as_object_mut()
        .map(|map| map.entry("generationConfig").or_insert_with(|| json!({})))
        .and_then(Value::as_object_mut);
    if let Some(config) = generation_config {
        config.insert("responseModalities".to_string(), json!(["AUDIO"]));
        if let Some(voice_name) = request_voice {
            config.insert(
                "speechConfig".to_string(),
                json!({
                    "voiceConfig": {
                        "prebuiltVoiceConfig": {
                            "voiceName": voice_name,
                        }
                    }
                }),
            );
        }
    } else if let Some(voice_name) = request_voice {
        body["generationConfig"] = json!({
            "responseModalities": ["AUDIO"],
            "speechConfig": {
                "voiceConfig": {
                    "prebuiltVoiceConfig": {
                        "voiceName": voice_name,
                    }
                }
            }
        });
    }
    body
}

pub fn build_image_request_body(req: &CanonicalRelayRequest, model: &str) -> Value {
    let mut body = pack_request(req, model, false);
    rewrite_data_url_file_parts_to_inline_data(&mut body);
    strip_common_generation_fields(&mut body);
    if let Some(config) = body
        .as_object_mut()
        .map(|map| map.entry("generationConfig").or_insert_with(|| json!({})))
        .and_then(Value::as_object_mut)
    {
        config.insert("responseModalities".to_string(), json!(["IMAGE"]));
        config.entry("imageConfig").or_insert_with(|| json!({}));
        if let Some(image_config) = config.get_mut("imageConfig").and_then(Value::as_object_mut) {
            image_config.insert(
                "aspectRatio".to_string(),
                json!(aspect_ratio_from_request(req)),
            );
        }
    }
    body
}

pub fn build_music_client_content(prompt: &str) -> Value {
    json!({
        "weightedPrompts": [{
            "text": prompt,
            "weight": 1.0
        }]
    })
}

pub fn build_music_generation_config(req: &CanonicalRelayRequest) -> Value {
    let mut config = serde_json::Map::new();
    if let Some(duration_seconds) = req
        .raw_body
        .get("duration")
        .or_else(|| req.raw_body.get("duration_s"))
        .or_else(|| req.raw_body.get("durationSeconds"))
        .and_then(Value::as_f64)
    {
        config.insert("durationSeconds".to_string(), json!(duration_seconds));
    }
    Value::Object(config)
}

pub fn extract_inline_image_from_generate_content_response(
    body: &Value,
) -> Result<legacy::GeminiCanvasImage, GatewayError> {
    let Some(candidates) = body.get("candidates").and_then(Value::as_array) else {
        return Err(GatewayError::server_error(
            "Gemini official image response did not contain candidates.",
        )
        .with_code("gemini_official_missing_image_candidates"));
    };

    for candidate in candidates {
        if let Some(parts) = candidate
            .get("content")
            .and_then(|content| content.get("parts"))
            .and_then(Value::as_array)
        {
            for part in parts {
                if let Some(inline_data) =
                    part.get("inlineData").or_else(|| part.get("inline_data"))
                {
                    let mime_type = inline_data
                        .get("mimeType")
                        .or_else(|| inline_data.get("mime_type"))
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .unwrap_or("image/png")
                        .to_string();
                    if !mime_type.starts_with("image/") {
                        continue;
                    }
                    let raw = inline_data
                        .get("data")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            GatewayError::server_error(
                                "Gemini official image response inlineData was missing base64 image bytes.",
                            )
                            .with_code("gemini_official_missing_image_data")
                        })?;
                    let bytes = base64::engine::general_purpose::STANDARD
                        .decode(raw)
                        .map_err(|error| {
                            GatewayError::server_error(format!(
                                "Failed to decode Gemini official image payload: {error}"
                            ))
                            .with_code("gemini_official_invalid_image_base64")
                        })?;
                    return Ok(legacy::GeminiCanvasImage { mime_type, bytes });
                }
            }
        }
    }

    Err(GatewayError::server_error(
        "Gemini official image response did not include an inlineData image part.",
    )
    .with_code("gemini_official_image_part_not_found"))
}

fn strip_common_generation_fields(body: &mut Value) {
    if let Some(map) = body.as_object_mut() {
        map.remove("model");
        map.remove("stream");
        map.remove("response_format");
        map.remove("voice");
    }
}

fn parse_base64_data_url(data_url: &str) -> Option<(String, String)> {
    let trimmed = data_url.trim();
    if !trimmed.starts_with("data:") {
        return None;
    }
    let (meta, data) = trimmed.split_once(',')?;
    if !meta.to_ascii_lowercase().contains(";base64") {
        return None;
    }
    let mime_type = meta
        .trim_start_matches("data:")
        .split(';')
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let encoded = data.trim();
    if encoded.is_empty() {
        return None;
    }
    Some((mime_type.to_string(), encoded.to_string()))
}

fn rewrite_data_url_file_parts_to_inline_data(body: &mut Value) {
    let Some(contents) = body.get_mut("contents").and_then(Value::as_array_mut) else {
        return;
    };

    for content in contents {
        let Some(parts) = content.get_mut("parts").and_then(Value::as_array_mut) else {
            continue;
        };
        for part in parts {
            let Some(file_uri) = part
                .get("fileData")
                .or_else(|| part.get("file_data"))
                .and_then(|value| {
                    value
                        .get("fileUri")
                        .or_else(|| value.get("file_uri"))
                        .and_then(Value::as_str)
                })
            else {
                continue;
            };
            let Some((mime_type, data)) = parse_base64_data_url(file_uri) else {
                continue;
            };
            *part = json!({
                "inlineData": {
                    "mimeType": mime_type,
                    "data": data,
                }
            });
        }
    }
}

fn read_optional_string(value: &Value, keys: &[&str]) -> Option<String> {
    let map = value.as_object()?;
    for key in keys {
        if let Some(value) = map.get(*key).and_then(Value::as_str) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::{
        CanonicalMessage, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
    };
    use serde_json::json;

    fn make_request(raw_body: Value) -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::VideosGenerations,
            requested_model: Some("gemini-canvas-video-preview".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "video".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
            }],
            tools: Vec::new(),
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body,
            previous_response_id: None,
            explicit_session_key: None,
            extra: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn official_video_defaults_to_sixteen_by_nine_when_size_unspecified() {
        let req = make_request(json!({ "prompt": "launch video" }));
        assert_eq!(video_aspect_ratio_from_request(&req), "16:9");
    }

    #[test]
    fn official_video_honors_explicit_aspect_ratio() {
        let req = make_request(json!({ "prompt": "launch video", "size": "9:16" }));
        assert_eq!(video_aspect_ratio_from_request(&req), "9:16");
    }

    #[test]
    fn build_video_generation_accepted_response_marks_pending_state() {
        let response = build_video_generation_accepted_response(
            "gemini-canvas-video-preview",
            "pending video",
            Some("c_123"),
            Some("r_456"),
            Some("/app/abc"),
            Some("job-1"),
            Some("video_placeholder"),
        );
        assert_eq!(response["accepted"], json!(true));
        assert_eq!(response["completed"], json!(false));
        assert_eq!(response["conversation_id"], json!("c_123"));
        assert_eq!(response["response_id"], json!("r_456"));
        assert_eq!(response["app_path"], json!("/app/abc"));
        assert_eq!(response["job_id"], json!("job-1"));
        assert_eq!(response["data"][0]["status"], json!("pending"));
        assert_eq!(response["data"][0]["kind"], json!("video"));
    }

    #[test]
    fn build_music_generation_accepted_response_marks_pending_state() {
        let response = build_music_generation_accepted_response(
            "gemini-canvas-music-preview",
            "pending music",
            Some("c_789"),
            Some("r_321"),
            Some("/app/music"),
            Some(30.0),
            Some("music_generation"),
        );
        assert_eq!(response["accepted"], json!(true));
        assert_eq!(response["completed"], json!(false));
        assert_eq!(response["conversation_id"], json!("c_789"));
        assert_eq!(response["response_id"], json!("r_321"));
        assert_eq!(response["duration_seconds"], json!(30.0));
        assert_eq!(response["data"][0]["status"], json!("pending"));
        assert_eq!(response["data"][0]["kind"], json!("audio"));
    }
}
