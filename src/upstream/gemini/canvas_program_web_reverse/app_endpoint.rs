use std::collections::HashMap;

use crate::error::GatewayError;
use crate::protocol::gemini::canvas_program_web_reverse::{
    GeminiCanvasProgramInvokeContract as ProtocolInvokeContract, GeminiCanvasProgramRelayConfig,
};
use crate::protocol::gemini_canvas;

mod action_input;
mod requests;

pub use action_input::{
    preferred_app_endpoint_action_aspect_ratio, preferred_app_endpoint_action_hints,
    preferred_app_endpoint_action_prompt, GeminiCanvasProgramActionHints,
};
pub use requests::{
    build_program_batchexecute_request_from_invoke_contract,
    build_program_stream_generate_request_from_invoke_contract,
};

#[derive(Debug, Clone, PartialEq)]
pub struct GeminiCanvasProgramAppInvokeContract {
    pub transport_kind: Option<String>,
    pub asset_url: Option<String>,
    pub asset_mime_type: Option<String>,
    pub asset_kind: Option<String>,
    pub ws_url: Option<String>,
    pub api_style: Option<String>,
    pub request_path: Option<String>,
    pub request_envelope_kind: Option<String>,
    pub prompt: Option<String>,
    pub duration_seconds: Option<f64>,
    pub aspect_ratio: Option<String>,
    pub music_ws_url: Option<String>,
    pub video_request_url: Option<String>,
    pub request_url: Option<String>,
    pub request_body: Option<String>,
    pub request_rpc_id: Option<String>,
    pub response_rpc_id: Option<String>,
    pub source_path: Option<String>,
    pub model_hint: Option<String>,
    pub cookie_header: Option<String>,
}

pub fn missing_gemini_canvas_program_app_endpoint_handle_error(provider: &str) -> GatewayError {
    GatewayError::bad_request(
        "Gemini Canvas program-owned app-endpoint lane requires a concrete app handle before invocation.",
    )
    .with_provider(provider)
    .with_code("missing_gemini_canvas_program_app_endpoint_handle")
}

pub fn gemini_canvas_program_direct_http_exhausted_error(provider: &str) -> GatewayError {
    GatewayError::service_unavailable(
        "Gemini Canvas program direct HTTP fetch exhausted all API key transport attempts.",
    )
    .with_provider(provider)
    .with_code("gemini_canvas_program_direct_http_exhausted")
}

pub fn gemini_canvas_program_direct_http_invalid_json_error(
    provider: &str,
    error: &str,
) -> GatewayError {
    GatewayError::server_error(format!(
        "Gemini Canvas program direct HTTP did not return valid JSON: {error}"
    ))
    .with_provider(provider)
    .with_code("gemini_canvas_program_direct_http_invalid_json")
}

pub fn gemini_canvas_program_direct_http_get_invalid_json_error(
    provider: &str,
    error: &str,
) -> GatewayError {
    GatewayError::server_error(format!(
        "Gemini Canvas program direct HTTP GET did not return valid JSON: {error}"
    ))
    .with_provider(provider)
    .with_code("gemini_canvas_program_direct_http_invalid_json")
}

pub fn preferred_app_endpoint_model_name(
    contract: &GeminiCanvasProgramAppInvokeContract,
) -> Option<String> {
    let raw = contract.model_hint.as_deref()?.trim();
    if raw.is_empty() {
        return None;
    }
    let before_suffix = raw.split(';').next().unwrap_or(raw).trim();
    let normalized = before_suffix
        .strip_prefix("models/")
        .unwrap_or(before_suffix)
        .trim();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.to_string())
    }
}

pub fn preferred_app_endpoint_page_url(
    base_url: &str,
    config: &GeminiCanvasProgramRelayConfig,
) -> Option<String> {
    if let Some(url) = config
        .app_endpoint
        .canvas_program_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(url.to_string());
    }
    if let Some(url) = config
        .app_endpoint
        .page_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(url.to_string());
    }
    config
        .app_endpoint
        .app_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|path| format!("{}{}", base_url.trim_end_matches('/'), path))
}

pub fn preferred_app_endpoint_harvest_target_url(
    base_url: &str,
    config: &GeminiCanvasProgramRelayConfig,
) -> String {
    preferred_app_endpoint_page_url(base_url, config).unwrap_or_else(|| {
        format!(
            "{}/share/{}",
            base_url.trim_end_matches('/'),
            config.bootstrap.share_id
        )
    })
}

pub fn build_program_app_endpoint_official_extra_headers(
    locale: &str,
    auth_user: &str,
    page_url: &str,
) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    headers.insert("Accept-Language".to_string(), locale.to_string());
    headers.insert("Referer".to_string(), page_url.to_string());
    if let Ok(parsed) = url::Url::parse(page_url) {
        if let Some(host) = parsed.host_str() {
            let origin = format!("{}://{}", parsed.scheme(), host);
            headers.insert("Origin".to_string(), origin);
        }
    }
    if !auth_user.trim().is_empty() {
        headers.insert("X-Goog-AuthUser".to_string(), auth_user.trim().to_string());
    }
    headers
}

pub fn preferred_app_endpoint_invoke_base_url(
    fallback_api_base_url: &str,
    config: &GeminiCanvasProgramRelayConfig,
) -> String {
    config
        .app_endpoint
        .invoke_base_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| fallback_api_base_url.trim_end_matches('/').to_string())
}

pub fn preferred_app_endpoint_music_ws_url(
    fallback_ws_url: &str,
    config: &GeminiCanvasProgramRelayConfig,
) -> String {
    config
        .app_endpoint
        .music_ws_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| fallback_ws_url.to_string())
}

pub fn preferred_app_endpoint_video_request_url(
    fallback_api_base_url: &str,
    upstream_model: &str,
    config: &GeminiCanvasProgramRelayConfig,
) -> String {
    if let Some(url) = config
        .app_endpoint
        .video_invoke_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if url.starts_with("http://") || url.starts_with("https://") {
            return url.to_string();
        }
        let prefix = preferred_app_endpoint_invoke_base_url(fallback_api_base_url, config);
        return format!(
            "{}{}",
            prefix.trim_end_matches('/'),
            if url.starts_with('/') {
                url.to_string()
            } else {
                format!("/{url}")
            }
        );
    }
    format!(
        "{}/models/{}:predictLongRunning",
        preferred_app_endpoint_invoke_base_url(fallback_api_base_url, config).trim_end_matches('/'),
        upstream_model
    )
}

pub fn preferred_app_endpoint_invoke_contract(
    operation: gemini_canvas::GeminiCanvasMediaOperation,
    fallback_api_base_url: &str,
    upstream_model: Option<&str>,
    config: &GeminiCanvasProgramRelayConfig,
) -> GeminiCanvasProgramAppInvokeContract {
    let action_hints = preferred_app_endpoint_action_hints(operation, config);
    let operation_name = match operation {
        gemini_canvas::GeminiCanvasMediaOperation::Music => "music",
        gemini_canvas::GeminiCanvasMediaOperation::Video => "video",
        gemini_canvas::GeminiCanvasMediaOperation::Image => "image",
    };
    let protocol_contract = config
        .app_endpoint
        .canvas_program_invoke_contract
        .as_ref()
        .filter(|contract| {
            contract
                .operation
                .as_deref()
                .map(|value| value == operation_name)
                .unwrap_or(true)
        });
    let asset_hint = preferred_media_asset_hint_from_protocol(operation, protocol_contract);
    GeminiCanvasProgramAppInvokeContract {
        transport_kind: protocol_contract.and_then(|contract| contract.transport_kind.clone()),
        asset_url: asset_hint.as_ref().and_then(|hint| hint.url.clone()),
        asset_mime_type: asset_hint.as_ref().and_then(|hint| hint.mime_type.clone()),
        asset_kind: asset_hint.as_ref().and_then(|hint| hint.kind.clone()),
        ws_url: protocol_contract.and_then(|contract| contract.ws_url.clone()),
        api_style: protocol_contract.and_then(|contract| contract.api_style.clone()),
        request_path: protocol_contract.and_then(|contract| contract.request_path.clone()),
        request_envelope_kind: protocol_contract
            .and_then(|contract| contract.request_envelope_kind.clone()),
        prompt: protocol_contract
            .and_then(|contract| contract.prompt.clone())
            .or_else(|| action_hints.as_ref().and_then(|hints| hints.prompt.clone())),
        duration_seconds: protocol_contract
            .and_then(|contract| contract.duration_seconds)
            .or_else(|| {
                action_hints
                    .as_ref()
                    .and_then(|hints| hints.duration_seconds)
            }),
        aspect_ratio: protocol_contract
            .and_then(|contract| contract.aspect_ratio.clone())
            .or_else(|| action_hints.and_then(|hints| hints.aspect_ratio)),
        music_ws_url: preferred_music_ws_url_from_protocol(
            operation,
            fallback_api_base_url,
            protocol_contract,
            config,
        ),
        video_request_url: preferred_video_request_url_from_protocol(
            operation,
            fallback_api_base_url,
            upstream_model,
            protocol_contract,
            config,
        ),
        request_url: protocol_contract.and_then(|contract| contract.request_url.clone()),
        request_body: protocol_contract.and_then(|contract| contract.request_body.clone()),
        request_rpc_id: protocol_contract.and_then(|contract| contract.request_rpc_id.clone()),
        response_rpc_id: protocol_contract.and_then(|contract| contract.response_rpc_id.clone()),
        source_path: protocol_contract.and_then(|contract| contract.source_path.clone()),
        model_hint: protocol_contract.and_then(|contract| contract.model_hint.clone()),
        cookie_header: protocol_contract.and_then(|contract| contract.cookie_header.clone()),
    }
}

#[derive(Debug, Clone)]
struct PreferredMediaAssetHint {
    url: Option<String>,
    mime_type: Option<String>,
    kind: Option<String>,
}

fn preferred_media_asset_hint_from_protocol(
    operation: gemini_canvas::GeminiCanvasMediaOperation,
    protocol_contract: Option<&ProtocolInvokeContract>,
) -> Option<PreferredMediaAssetHint> {
    let contract = protocol_contract?;
    if let Some(target) = preferred_media_asset_candidate_from_fields(
        operation,
        contract.target.as_deref(),
        contract.target_mime_type.as_deref(),
        None,
    ) {
        return Some(target);
    }

    contract.target_candidates.iter().find_map(|candidate| {
        preferred_media_asset_candidate_from_fields(
            operation,
            candidate.url.as_deref(),
            candidate.mime_type.as_deref(),
            candidate.kind.as_deref(),
        )
    })
}

fn preferred_media_asset_candidate_from_fields(
    operation: gemini_canvas::GeminiCanvasMediaOperation,
    url: Option<&str>,
    mime_type: Option<&str>,
    kind: Option<&str>,
) -> Option<PreferredMediaAssetHint> {
    let url = url.map(str::trim).filter(|value| !value.is_empty())?;
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return None;
    }
    let mime_type = mime_type.map(str::trim).filter(|value| !value.is_empty());
    let kind = kind.map(str::trim).filter(|value| !value.is_empty());
    if !is_likely_media_asset_target(operation, url, mime_type, kind) {
        return None;
    }
    Some(PreferredMediaAssetHint {
        url: Some(url.to_string()),
        mime_type: mime_type.map(str::to_string),
        kind: kind.map(str::to_string),
    })
}

fn is_likely_media_asset_target(
    operation: gemini_canvas::GeminiCanvasMediaOperation,
    url: &str,
    mime_type: Option<&str>,
    kind: Option<&str>,
) -> bool {
    let lowered_url = url.trim().to_ascii_lowercase();
    if lowered_url.is_empty() {
        return false;
    }
    let lowered_mime = mime_type.map(|value| value.to_ascii_lowercase());
    let lowered_kind = kind.map(|value| value.to_ascii_lowercase());
    match operation {
        gemini_canvas::GeminiCanvasMediaOperation::Music => {
            lowered_url.contains("contribution.usercontent.google.com/download")
                || lowered_mime
                    .as_deref()
                    .is_some_and(|value| value.starts_with("audio/") || value.starts_with("video/"))
                || lowered_kind
                    .as_deref()
                    .is_some_and(|value| value == "audio" || value == "video")
        }
        gemini_canvas::GeminiCanvasMediaOperation::Video => {
            lowered_url.contains("contribution.usercontent.google.com/download")
                || lowered_mime
                    .as_deref()
                    .is_some_and(|value| value.starts_with("video/"))
                || lowered_kind
                    .as_deref()
                    .is_some_and(|value| value == "video")
        }
        gemini_canvas::GeminiCanvasMediaOperation::Image => false,
    }
}

fn preferred_music_ws_url_from_protocol(
    operation: gemini_canvas::GeminiCanvasMediaOperation,
    fallback_ws_url: &str,
    protocol_contract: Option<&ProtocolInvokeContract>,
    config: &GeminiCanvasProgramRelayConfig,
) -> Option<String> {
    if !matches!(operation, gemini_canvas::GeminiCanvasMediaOperation::Music) {
        return None;
    }
    protocol_contract
        .and_then(|contract| {
            if contract
                .transport_kind
                .as_deref()
                .is_some_and(|value| value == "canvas_program_ws_candidate")
            {
                return None;
            }
            contract
                .target
                .as_deref()
                .filter(|value| value.starts_with("ws://") || value.starts_with("wss://"))
        })
        .map(str::to_string)
        .or_else(|| {
            config
                .app_endpoint
                .music_ws_url
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|_| preferred_app_endpoint_music_ws_url(fallback_ws_url, config))
        })
}

fn preferred_video_request_url_from_protocol(
    operation: gemini_canvas::GeminiCanvasMediaOperation,
    fallback_api_base_url: &str,
    upstream_model: Option<&str>,
    protocol_contract: Option<&ProtocolInvokeContract>,
    config: &GeminiCanvasProgramRelayConfig,
) -> Option<String> {
    if !matches!(operation, gemini_canvas::GeminiCanvasMediaOperation::Video) {
        return None;
    }
    protocol_contract
        .and_then(|contract| {
            if contract
                .transport_kind
                .as_deref()
                .is_some_and(|value| value == "canvas_program_ws_candidate")
            {
                return None;
            }
            contract
                .target
                .as_deref()
                .filter(|value| is_likely_video_invoke_target(value))
                .map(str::to_string)
        })
        .or_else(|| {
            let has_explicit_video_path = config
                .app_endpoint
                .video_invoke_path
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_some();
            let has_explicit_invoke_base = config
                .app_endpoint
                .invoke_base_url
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_some();
            (has_explicit_video_path || has_explicit_invoke_base).then(|| {
                preferred_app_endpoint_video_request_url(
                    fallback_api_base_url,
                    upstream_model.unwrap_or_default(),
                    config,
                )
            })
        })
}

fn is_likely_video_invoke_target(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.contains("predictLongRunning") || trimmed.contains(":predictLongRunning") {
        return true;
    }
    if trimmed.contains("/download?") || trimmed.contains("contribution.usercontent.google.com") {
        return false;
    }
    false
}
