//! Udio prompts, output counts/preferences and polling options.

use super::field_values::{read_number_fields, read_optional_u64};
use super::DEFAULT_POLL_INTERVAL_MS;
use super::DEFAULT_WAIT_TIMEOUT_SECS;
use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use serde_json::Map;
use serde_json::Value;

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
