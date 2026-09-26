use serde_json::{json, Map, Value};

use crate::error::GatewayError;
mod browser_worker_errors;
#[cfg(test)]
mod browser_worker_errors_tests;
mod conversation_summary;
#[cfg(test)]
mod conversation_summary_tests;
mod endpoints;
#[cfg(test)]
mod endpoints_tests;
mod image_response;
mod music_stream;
#[cfg(test)]
mod music_stream_tests;
mod request_body;
#[cfg(test)]
mod request_body_tests;
mod request_normalize;
#[cfg(test)]
mod request_normalize_tests;
#[cfg(test)]
mod request_plan_tests;
#[cfg(test)]
mod response_tests;
mod tool_call_stream;
#[cfg(test)]
mod tool_call_stream_tests;
mod video_response;
#[cfg(test)]
mod video_response_tests;

pub use browser_worker_errors::{
    browser_worker_input_serialize_error, browser_worker_nonzero_exit_error,
    browser_worker_output_buffer_allocation_error, browser_worker_output_parse_error,
    browser_worker_output_too_large_error, browser_worker_spawn_failed_error,
    browser_worker_stdin_error, browser_worker_timeout_error, browser_worker_wait_failed_error,
    empty_browser_worker_output_error, missing_browser_worker_result_error,
};
pub use conversation_summary::summarize_conversation_stream_text;
pub use endpoints::{
    build_clips_library_url, build_conversation_url, build_image_generation_url,
    build_message_stream_url, build_session_url, build_video_detail_path, build_video_library_path,
    build_video_status_path, build_video_status_url,
};
pub use image_response::build_image_generation_response;
pub use music_stream::{
    accumulate_job_stream, extract_job_id, extract_stream_job_id, parse_producer_music_stream_text,
};
pub use request_body::{
    build_conversation_request_body, build_image_generation_request, build_send_message_plan,
    build_send_message_request, build_video_tool_call_request,
};
pub use request_normalize::{
    normalize_image_generations, normalize_music_generations, normalize_video_generations,
    should_use_image_generations, should_use_video_generations,
};
pub use tool_call_stream::accumulate_tool_call_stream;
#[cfg(test)]
use tool_call_stream::parse_tool_call_stream_text;
pub use video_response::{
    build_video_generation_response, build_video_proposal_prompt, choose_video_confirm_prompt,
};

pub const PRODUCER_DEFAULT_MODEL: &str = "producer:standard";
pub const PRODUCER_IMAGE_DEFAULT_MODEL: &str = "producer:image";
pub const PRODUCER_VIDEO_DEFAULT_MODEL: &str = "producer:music-video";

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

fn parse_json_or_string(value: &str) -> Value {
    if value.trim().is_empty() {
        Value::Null
    } else if let Ok(parsed) = serde_json::from_str::<Value>(value) {
        parsed
    } else {
        Value::String(value.to_string())
    }
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

fn extract_clip_id(map: &Map<String, Value>) -> Option<String> {
    read_string_fields(map, &["clip_id", "clipId", "song_id", "songId"]).or_else(|| {
        map.get("args")
            .and_then(|value| value.as_object())
            .and_then(|args| read_string_fields(args, &["clip_id", "clipId", "song_id", "songId"]))
    })
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
