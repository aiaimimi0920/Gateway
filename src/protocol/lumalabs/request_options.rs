//! LumaLabs request prompts, output preferences and action selection.

use super::runtime::LumalabsRuntime;
use super::DEFAULT_ASPECT_RATIO;
use super::DEFAULT_IMAGE_OUTPUT_FORMAT;
use super::DEFAULT_STYLE;
use super::{default_action_type_for_operation, LumalabsMediaOperation};
use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use serde_json::Value;

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

pub(super) fn read_optional_string(value: &Value, keys: &[&str]) -> Option<String> {
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
