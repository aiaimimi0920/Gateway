//! Suno model mapping, prompts, output preferences and polling options.

use super::field_values::{read_optional_string, read_optional_u64};
use super::DEFAULT_POLL_INTERVAL_MS;
use super::DEFAULT_WAIT_TIMEOUT_SECS;
use super::SUNO_DEFAULT_MODEL;
use super::SUNO_DEFAULT_UPSTREAM_MODEL;
use super::SUNO_LEGACY_MODEL;
use super::SUNO_LEGACY_UPSTREAM_MODEL;
use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use serde_json::Map;
use serde_json::Value;

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

pub(super) fn prompt_from_request_body(body_obj: &Map<String, Value>) -> Option<String> {
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
