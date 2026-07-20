use std::collections::HashMap;

use crate::error::GatewayError;
use crate::protocol::gemini::canvas_program_web_reverse::{
    GeminiCanvasProgramInvokeContract as ProtocolInvokeContract, GeminiCanvasProgramRelayConfig,
};
use crate::protocol::gemini::web_reverse as gemini_web;
use crate::protocol::gemini_canvas;

#[derive(Debug, Clone, PartialEq)]
pub struct GeminiCanvasProgramActionHints {
    pub prompt: Option<String>,
    pub duration_seconds: Option<f64>,
    pub aspect_ratio: Option<String>,
}

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

fn extract_prompt_from_action_input(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if !trimmed.starts_with('{') {
        return Some(trimmed.to_string());
    }
    for marker in ["'prompt'", "\"prompt\""] {
        let Some(marker_index) = raw.find(marker) else {
            continue;
        };
        let after_marker = &raw[marker_index + marker.len()..];
        let Some(colon_index) = after_marker.find(':') else {
            continue;
        };
        let after_colon = after_marker[colon_index + 1..].trim_start();
        let Some(quote) = after_colon
            .chars()
            .next()
            .filter(|ch| *ch == '\'' || *ch == '"')
        else {
            continue;
        };
        let remaining = &after_colon[quote.len_utf8()..];
        if let Some(end_index) = remaining.find(quote) {
            let prompt = remaining[..end_index].trim();
            if !prompt.is_empty() {
                return Some(prompt.to_string());
            }
        }
    }
    None
}

fn extract_raw_action_scalar<'a>(raw: &'a str, markers: &[&str]) -> Option<&'a str> {
    for marker in markers {
        let Some(marker_index) = raw.find(marker) else {
            continue;
        };
        let after_marker = &raw[marker_index + marker.len()..];
        let Some(colon_index) = after_marker.find(':') else {
            continue;
        };
        let after_colon = after_marker[colon_index + 1..].trim_start();
        if after_colon.is_empty() {
            continue;
        }
        if let Some(quote) = after_colon
            .chars()
            .next()
            .filter(|ch| *ch == '\'' || *ch == '"')
        {
            let remaining = &after_colon[quote.len_utf8()..];
            if let Some(end_index) = remaining.find(quote) {
                return Some(remaining[..end_index].trim());
            }
            continue;
        }
        let end_index = after_colon
            .find([',', '}', '\n'])
            .unwrap_or(after_colon.len());
        return Some(after_colon[..end_index].trim());
    }
    None
}

fn extract_duration_seconds_from_action_input(raw: &str) -> Option<f64> {
    extract_raw_action_scalar(
        raw,
        &[
            "'duration_seconds'",
            "\"duration_seconds\"",
            "'durationSeconds'",
            "\"durationSeconds\"",
            "'duration'",
            "\"duration\"",
        ],
    )
    .and_then(|value| value.parse::<f64>().ok())
}

fn extract_aspect_ratio_from_action_input(raw: &str) -> Option<String> {
    extract_raw_action_scalar(
        raw,
        &[
            "'aspect_ratio'",
            "\"aspect_ratio\"",
            "'aspectRatio'",
            "\"aspectRatio\"",
            "'aspect'",
            "\"aspect\"",
        ],
    )
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .map(str::to_string)
}

pub fn preferred_app_endpoint_action_hints(
    operation: gemini_canvas::GeminiCanvasMediaOperation,
    config: &GeminiCanvasProgramRelayConfig,
) -> Option<GeminiCanvasProgramActionHints> {
    let expected_action = match operation {
        gemini_canvas::GeminiCanvasMediaOperation::Music => "music_generation",
        gemini_canvas::GeminiCanvasMediaOperation::Video => "video_generation",
        gemini_canvas::GeminiCanvasMediaOperation::Image => "image_generation",
    };
    let action = config
        .app_endpoint
        .canvas_program_action
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    if action != expected_action {
        return None;
    }
    let action_input = config
        .app_endpoint
        .canvas_program_action_input
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    Some(GeminiCanvasProgramActionHints {
        prompt: extract_prompt_from_action_input(action_input),
        duration_seconds: extract_duration_seconds_from_action_input(action_input),
        aspect_ratio: extract_aspect_ratio_from_action_input(action_input),
    })
}

pub fn preferred_app_endpoint_action_prompt(
    operation: gemini_canvas::GeminiCanvasMediaOperation,
    config: &GeminiCanvasProgramRelayConfig,
) -> Option<String> {
    preferred_app_endpoint_action_hints(operation, config).and_then(|hints| hints.prompt)
}

pub fn preferred_app_endpoint_action_aspect_ratio(
    operation: gemini_canvas::GeminiCanvasMediaOperation,
    config: &GeminiCanvasProgramRelayConfig,
) -> Option<String> {
    preferred_app_endpoint_action_hints(operation, config).and_then(|hints| hints.aspect_ratio)
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

fn current_reqid() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64 % 1_000_000)
        .unwrap_or(1)
}

fn parse_form_urlencoded_pairs(input: &str) -> Vec<(String, String)> {
    url::form_urlencoded::parse(input.as_bytes())
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect()
}

fn serialize_form_urlencoded_pairs(pairs: &[(String, String)]) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in pairs {
        serializer.append_pair(key, value);
    }
    serializer.finish()
}

fn upsert_pair(pairs: &mut Vec<(String, String)>, key: &str, value: String) {
    if let Some(entry) = pairs.iter_mut().find(|(entry_key, _)| entry_key == key) {
        entry.1 = value;
    } else {
        pairs.push((key.to_string(), value));
    }
}

fn replace_contract_prompt(
    form: &mut Vec<(String, String)>,
    original_prompt: Option<&str>,
    requested_prompt: Option<&str>,
) {
    let Some(original_prompt) = original_prompt
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let Some(requested_prompt) = requested_prompt
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    if requested_prompt == original_prompt {
        return;
    }
    for (_, value) in form.iter_mut() {
        if value.contains(original_prompt) {
            *value = value.replacen(original_prompt, requested_prompt, 1);
        }
    }
}

pub fn build_program_batchexecute_request_from_invoke_contract(
    contract: &GeminiCanvasProgramAppInvokeContract,
    bootstrap: &gemini_web::GeminiWebBootstrap,
    locale: &str,
    requested_prompt: Option<&str>,
) -> Result<Option<gemini_web::GeminiWebRequest>, GatewayError> {
    let Some(request_url) = contract
        .request_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let Some(request_body) = contract
        .request_body
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let parsed_url = url::Url::parse(request_url).map_err(|error| {
        GatewayError::server_error(format!(
            "parse Gemini Canvas program batchexecute requestUrl failed: {error}"
        ))
        .with_provider("gemini_canvas_program_web_reverse_compatible")
        .with_code("gemini_canvas_program_batchexecute_request_url_invalid")
    })?;
    let mut query: Vec<(String, String)> = parsed_url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    let mut form = parse_form_urlencoded_pairs(request_body);

    if let Some(build_label) = bootstrap.build_label.as_deref() {
        upsert_pair(&mut query, "bl", build_label.trim().to_string());
    }
    if let Some(session_id) = bootstrap.session_id.as_deref() {
        upsert_pair(&mut query, "f.sid", session_id.trim().to_string());
    }
    upsert_pair(&mut query, "hl", locale.trim().to_string());
    upsert_pair(&mut query, "_reqid", current_reqid().to_string());
    upsert_pair(&mut query, "rt", "c".to_string());

    if let Some(source_path) = contract
        .source_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        upsert_pair(&mut query, "source-path", source_path.to_string());
    }

    if let Some(access_token) = bootstrap.access_token.as_deref() {
        upsert_pair(&mut form, "at", access_token.trim().to_string());
    }
    replace_contract_prompt(&mut form, contract.prompt.as_deref(), requested_prompt);

    Ok(Some(gemini_web::GeminiWebRequest { query, form }))
}

pub fn build_program_stream_generate_request_from_invoke_contract(
    contract: &GeminiCanvasProgramAppInvokeContract,
    requested_prompt: Option<&str>,
) -> Result<Option<gemini_canvas::GeminiCanvasTextStreamGenerateTemplate>, GatewayError> {
    let Some(request_url) = contract
        .request_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    if !request_url.contains("/StreamGenerate") {
        return Ok(None);
    }
    let Some(request_body) = contract
        .request_body
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let parsed_url = url::Url::parse(request_url).map_err(|error| {
        GatewayError::server_error(format!(
            "parse Gemini Canvas program StreamGenerate requestUrl failed: {error}"
        ))
        .with_provider("gemini_canvas_program_web_reverse_compatible")
        .with_code("gemini_canvas_program_stream_generate_request_url_invalid")
    })?;
    let query: Vec<(String, String)> = parsed_url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    let mut form = parse_form_urlencoded_pairs(request_body);
    replace_contract_prompt(&mut form, contract.prompt.as_deref(), requested_prompt);
    let mut template_url = parsed_url.clone();
    template_url.set_query(None);
    template_url.set_fragment(None);
    let mut headers = HashMap::new();
    headers.insert(
        "content-type".to_string(),
        "application/x-www-form-urlencoded;charset=UTF-8".to_string(),
    );
    headers.insert("accept".to_string(), "*/*".to_string());
    Ok(Some(
        gemini_canvas::GeminiCanvasTextStreamGenerateTemplate {
            url: template_url.to_string(),
            query,
            form: form.clone(),
            raw_post_data: serialize_form_urlencoded_pairs(&form),
            headers,
        },
    ))
}
