use std::collections::HashMap;

use serde_json::Value;
use url::Url;

use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::gemini_canvas as legacy;
use crate::routing::candidate::ProviderAccountPayload;

const DEFAULT_ASPECT_RATIO: &str = "1:1";

pub fn runtime_from_payload(
    payload: &ProviderAccountPayload,
) -> Result<legacy::GeminiCanvasRuntime, GatewayError> {
    let runtime_state_object_key = payload
        .runtime_state_object_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| {
            GatewayError::bad_request(
                "Gemini Canvas credentials require runtime_state_object_key browser-state material.",
            )
            .with_code("missing_gemini_canvas_runtime_state")
        })?;

    let share_id = payload
        .extra_body
        .as_ref()
        .and_then(|extra| read_optional_hash_string(extra, &["shareId", "share_id"]))
        .or_else(|| {
            payload.extra_body.as_ref().and_then(|extra| {
                read_optional_hash_string(extra, &["shareUrl", "share_url"])
                    .and_then(|value| legacy::share_id_from_share_url(&value))
            })
        })
        .unwrap_or_else(|| legacy::GEMINI_CANVAS_DEFAULT_SHARE_ID.to_string());
    let api_base_url = payload
        .extra_body
        .as_ref()
        .and_then(|extra| read_optional_hash_string(extra, &["apiBaseUrl", "api_base_url"]))
        .unwrap_or_else(|| legacy::GEMINI_CANVAS_DEFAULT_API_BASE_URL.to_string());

    Ok(legacy::GeminiCanvasRuntime {
        runtime_state_object_key,
        share_id,
        api_base_url,
    })
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
                .with_code("invalid_image_response_format"),
        );
    };

    match format {
        "url" => Ok(true),
        "b64_json" => Ok(false),
        _ => Err(GatewayError::bad_request(
            "Gemini Canvas image endpoints currently support response_format=url or b64_json.",
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

pub fn locale_from_payload(payload: &ProviderAccountPayload) -> String {
    payload
        .headers
        .get("accept-language")
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("en-US")
        .to_string()
}

pub fn build_text_fetch_url(runtime: &legacy::GeminiCanvasRuntime, model: &str) -> String {
    format!(
        "{}/models/{}:generateContent",
        runtime.api_base_url.trim_end_matches('/'),
        model
    )
}

pub fn direct_http_image_api_base_url(runtime: &legacy::GeminiCanvasRuntime) -> String {
    let trimmed = runtime.api_base_url.trim();
    if trimmed.is_empty() {
        return legacy::GEMINI_CANVAS_DIRECT_HTTP_IMAGE_API_BASE_URL.to_string();
    }
    let Some(parsed) = Url::parse(trimmed).ok() else {
        return trimmed.trim_end_matches('/').to_string();
    };
    let Some(host) = parsed.host_str() else {
        return trimmed.trim_end_matches('/').to_string();
    };
    if host.eq_ignore_ascii_case("generativelanguage.googleapis.com") {
        return legacy::GEMINI_CANVAS_DIRECT_HTTP_IMAGE_API_BASE_URL.to_string();
    }
    trimmed.trim_end_matches('/').to_string()
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

fn read_optional_hash_string(map: &HashMap<String, Value>, keys: &[&str]) -> Option<String> {
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
