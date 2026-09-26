use rquest::{header::HeaderMap, Method};

use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind};
use crate::protocol::suno;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::browser_executor_helpers::{
    build_browser_executor_header_map, missing_browser_executor_field_error, read_json_bool,
    read_json_u64,
};
use crate::upstream::browser_worker_types::SunoBrowserWorkerInput;
use crate::upstream::common::RequestPlan;
use crate::upstream::header_map_helpers::{
    extract_bearer_token, header_map_string, insert_runtime_header,
};
use crate::upstream::headers::build_upstream_headers_with;
use std::time::Duration;

#[derive(Debug)]
pub(crate) struct PreparedSunoBrowserExecutorServiceInput {
    pub(crate) base_url: String,
    pub(crate) headers: HeaderMap,
    pub(crate) request_body: serde_json::Value,
    pub(crate) target_asset_kind: String,
    pub(crate) wait_completion: bool,
    pub(crate) wait_timeout: Duration,
    pub(crate) poll_interval: Duration,
    pub(crate) timeout: Duration,
}

pub(crate) fn unsupported_request_plan_error() -> GatewayError {
    GatewayError::bad_request(
        "Suno adapters currently support reverse-web image, video, and audio-generation passthrough endpoints",
    )
    .with_provider("suno_compatible")
    .with_code("unsupported_suno_endpoint")
}

pub(crate) fn missing_runtime_cookie_error(browser_backed: bool) -> GatewayError {
    let message = if browser_backed {
        "Suno browser-backed requests require runtime Cookie headers from keepalive ensure."
    } else {
        "Suno requests require runtime Cookie headers from keepalive ensure."
    };
    GatewayError::server_error(message)
        .with_provider("suno_compatible")
        .with_code("missing_suno_runtime_cookie")
}

pub(crate) fn missing_runtime_bearer_error(browser_backed: bool) -> GatewayError {
    let message = if browser_backed {
        "Suno browser-backed requests require a runtime Clerk bearer token from keepalive ensure."
    } else {
        "Suno requests require a runtime Clerk bearer token from keepalive ensure."
    };
    GatewayError::server_error(message)
        .with_provider("suno_compatible")
        .with_code("missing_suno_runtime_bearer")
}

pub(crate) fn unsupported_suno_edit_endpoint_error() -> GatewayError {
    GatewayError::bad_request("Suno adapters do not currently support /v1/images/edits.")
        .with_provider("suno_compatible")
        .with_code("unsupported_suno_edit_endpoint")
}

pub(crate) fn unsupported_suno_media_generation_endpoint_error() -> GatewayError {
    GatewayError::bad_request(
        "Suno adapters currently support only /v1/images/generations, /v1/videos/generations, and /v1/music/generations.",
    )
    .with_provider("suno_compatible")
    .with_code("unsupported_suno_endpoint")
}

pub(crate) fn unsupported_suno_image_inputs_error() -> GatewayError {
    GatewayError::bad_request(
        "Suno image generation currently does not support uploaded image or mask inputs.",
    )
    .with_provider("suno_compatible")
    .with_code("unsupported_suno_image_inputs")
}

pub(crate) fn unsupported_suno_video_count_error() -> GatewayError {
    GatewayError::bad_request("Suno video generation currently supports only n=1 requests.")
        .with_provider("suno_compatible")
        .with_code("unsupported_suno_video_count")
}

pub(crate) fn validate_suno_media_request(
    req: &CanonicalRelayRequest,
) -> Result<String, GatewayError> {
    match req.endpoint_kind {
        EndpointKind::ImagesGenerations
        | EndpointKind::MusicGenerations
        | EndpointKind::VideosGenerations => {}
        EndpointKind::ImagesEdits => {
            return Err(unsupported_suno_edit_endpoint_error());
        }
        _ => {
            return Err(unsupported_suno_media_generation_endpoint_error());
        }
    }

    let prompt = suno::prompt_from_request(req)?;
    if req.endpoint_kind == EndpointKind::ImagesGenerations {
        let _ = suno::prefers_url_response(req)?;
        let has_input_images = req.raw_body.get("image").is_some()
            || req.raw_body.get("images").is_some()
            || req.raw_body.get("images[]").is_some()
            || req.raw_body.get("mask").is_some();
        if has_input_images {
            return Err(unsupported_suno_image_inputs_error());
        }
    }
    if req.endpoint_kind == EndpointKind::VideosGenerations && suno::requested_output_count(req) > 1
    {
        return Err(unsupported_suno_video_count_error());
    }

    Ok(prompt)
}

pub(crate) fn build_suno_challenge_check_plan(
    base_url: &str,
    response_kind: EndpointKind,
) -> RequestPlan {
    RequestPlan {
        method: Method::POST,
        url: format!("{}/api/c/check", base_url.trim_end_matches('/')),
        query: Vec::new(),
        body: Some(suno::build_challenge_check_request()),
        response_kind,
    }
}

pub(crate) fn build_suno_generate_plan(
    base_url: &str,
    req: &CanonicalRelayRequest,
    model: &str,
    user_tier: Option<&str>,
) -> Result<RequestPlan, GatewayError> {
    Ok(RequestPlan {
        method: Method::POST,
        url: format!("{}/api/generate/v2-web/", base_url.trim_end_matches('/')),
        query: Vec::new(),
        body: Some(suno::build_generate_request(req, model, user_tier)?),
        response_kind: req.endpoint_kind,
    })
}

pub(crate) fn build_suno_feed_poll_plan(
    base_url: &str,
    response_kind: EndpointKind,
    clip_ids: &[String],
) -> RequestPlan {
    RequestPlan {
        method: Method::POST,
        url: format!("{}/api/feed/v3", base_url.trim_end_matches('/')),
        query: Vec::new(),
        body: Some(suno::build_feed_poll_request(clip_ids)),
        response_kind,
    }
}

pub(crate) fn build_suno_runtime_headers(base_headers: &HeaderMap) -> HeaderMap {
    let mut headers = base_headers.clone();
    let device_id = header_map_string(&headers, "device-id")
        .map(|value| value.trim_matches('"').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    insert_runtime_header(&mut headers, "device-id", &device_id);
    insert_runtime_header(&mut headers, "referring-pathname", "/");
    insert_runtime_header(&mut headers, "referring-origin", "https://suno.com");
    insert_runtime_header(&mut headers, "origin", "https://suno.com");
    insert_runtime_header(&mut headers, "referer", "https://suno.com/");
    insert_runtime_header(
        &mut headers,
        "browser-token",
        &suno::build_browser_token_header_value(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64,
        ),
    );
    headers
}

pub(crate) fn suno_user_tier(payload: &ProviderAccountPayload) -> Option<String> {
    payload.extra_body.as_ref().and_then(|extra_body| {
        extra_body
            .get("userTier")
            .or_else(|| extra_body.get("user_tier"))
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedSunoExecutionPlan {
    pub(crate) missing_challenge_token: bool,
    pub(crate) wait_completion: bool,
    pub(crate) wait_timeout: Duration,
    pub(crate) poll_interval: Duration,
    pub(crate) request_timeout: Duration,
    pub(crate) target_asset_kind: &'static str,
}

pub(crate) fn prepare_suno_execution_plan(
    req: &CanonicalRelayRequest,
    default_timeout: Duration,
    browser_backed: bool,
) -> Result<PreparedSunoExecutionPlan, GatewayError> {
    let target_asset_kind = match req.endpoint_kind {
        EndpointKind::ImagesGenerations => "image",
        EndpointKind::MusicGenerations => "audio",
        EndpointKind::VideosGenerations => "video",
        EndpointKind::ImagesEdits => {
            return Err(unsupported_suno_edit_endpoint_error());
        }
        _ => {
            return Err(unsupported_suno_media_generation_endpoint_error());
        }
    };
    let request_timeout =
        default_timeout.max(Duration::from_secs(if browser_backed { 300 } else { 240 }));

    Ok(PreparedSunoExecutionPlan {
        missing_challenge_token: !suno::has_challenge_token(req),
        wait_completion: suno::wait_for_completion(req),
        wait_timeout: Duration::from_secs(suno::wait_timeout_secs(req)),
        poll_interval: Duration::from_millis(suno::poll_interval_ms(req)),
        request_timeout,
        target_asset_kind,
    })
}

#[derive(Debug, Clone)]
pub(crate) struct SunoPreparedRequestContext {
    pub(crate) prompt: String,
    pub(crate) base_url: String,
    pub(crate) base_headers: HeaderMap,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedSunoExecutionContext {
    pub(crate) prompt: String,
    pub(crate) base_url: String,
    pub(crate) runtime_headers: HeaderMap,
    pub(crate) user_tier: Option<String>,
    pub(crate) execution_plan: PreparedSunoExecutionPlan,
}

pub(crate) fn prepare_suno_request_context(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    extra_headers: Option<&std::collections::HashMap<String, String>>,
    browser_backed: bool,
) -> Result<SunoPreparedRequestContext, GatewayError> {
    let prompt = validate_suno_media_request(req)?;
    let base_headers = build_upstream_headers_with(payload, extra_headers);
    if !browser_backed && !base_headers.contains_key(rquest::header::COOKIE) {
        return Err(missing_runtime_cookie_error(browser_backed));
    }
    if !browser_backed && !base_headers.contains_key(rquest::header::AUTHORIZATION) {
        return Err(missing_runtime_bearer_error(browser_backed));
    }

    Ok(SunoPreparedRequestContext {
        prompt,
        base_url: payload.base_url.trim_end_matches('/').to_string(),
        base_headers,
    })
}

pub(crate) fn prepare_suno_execution_context(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    extra_headers: Option<&std::collections::HashMap<String, String>>,
    browser_backed: bool,
    default_timeout: Duration,
) -> Result<PreparedSunoExecutionContext, GatewayError> {
    let prepared = prepare_suno_request_context(payload, req, extra_headers, browser_backed)?;
    let runtime_headers = build_suno_runtime_headers(&prepared.base_headers);
    let execution_plan = prepare_suno_execution_plan(req, default_timeout, browser_backed)?;

    Ok(PreparedSunoExecutionContext {
        prompt: prepared.prompt,
        base_url: prepared.base_url,
        runtime_headers,
        user_tier: suno_user_tier(payload),
        execution_plan,
    })
}

pub(crate) fn build_suno_browser_executor_payload(
    base_url: &str,
    runtime_headers: &HeaderMap,
    req: &CanonicalRelayRequest,
    target_asset_kind: &str,
    wait_timeout: Duration,
    poll_interval: Duration,
    request_timeout: Duration,
    browser_executable_path: Option<String>,
) -> serde_json::Value {
    serde_json::json!({
        "baseUrl": base_url,
        "cookieHeader": header_map_string(runtime_headers, "cookie"),
        "authToken": extract_bearer_token(runtime_headers),
        "requestBody": req.raw_body,
        "targetAssetKind": target_asset_kind,
        "waitCompletion": suno::wait_for_completion(req),
        "waitTimeoutMs": wait_timeout.as_millis().min(u128::from(u64::MAX)) as u64,
        "pollIntervalMs": poll_interval.as_millis().min(u128::from(u64::MAX)) as u64,
        "timeoutMs": request_timeout.as_millis().min(u128::from(u64::MAX)) as u64,
        "browserExecutablePath": browser_executable_path,
        "userAgent": header_map_string(runtime_headers, "user-agent"),
        "acceptLanguage": header_map_string(runtime_headers, "accept-language"),
        "origin": header_map_string(runtime_headers, "origin"),
        "referer": header_map_string(runtime_headers, "referer"),
        "deviceId": header_map_string(runtime_headers, "device-id"),
        "browserToken": header_map_string(runtime_headers, "browser-token"),
        "referringPathname": header_map_string(runtime_headers, "referring-pathname"),
        "referringOrigin": header_map_string(runtime_headers, "referring-origin"),
    })
}

pub(crate) fn build_suno_browser_worker_input<'a>(
    base_url: &'a str,
    headers: &HeaderMap,
    request_body: &'a serde_json::Value,
    target_asset_kind: &'a str,
    wait_completion: bool,
    wait_timeout: Duration,
    poll_interval: Duration,
    timeout: Duration,
    browser_executable_path: Option<String>,
) -> SunoBrowserWorkerInput<'a> {
    SunoBrowserWorkerInput {
        base_url,
        cookie_header: header_map_string(headers, "cookie"),
        auth_token: extract_bearer_token(headers),
        request_body,
        target_asset_kind,
        wait_completion,
        wait_timeout_ms: wait_timeout.as_millis().min(u128::from(u64::MAX)) as u64,
        poll_interval_ms: poll_interval.as_millis().min(u128::from(u64::MAX)) as u64,
        timeout_ms: timeout.as_millis().min(u128::from(u64::MAX)) as u64,
        browser_executable_path,
        user_agent: header_map_string(headers, "user-agent"),
        accept_language: header_map_string(headers, "accept-language"),
        origin: header_map_string(headers, "origin"),
        referer: header_map_string(headers, "referer"),
        device_id: header_map_string(headers, "device-id"),
        browser_token: header_map_string(headers, "browser-token"),
        referring_pathname: header_map_string(headers, "referring-pathname"),
        referring_origin: header_map_string(headers, "referring-origin"),
    }
}

pub(crate) fn prepare_suno_browser_executor_service_input(
    input: &serde_json::Value,
) -> Result<PreparedSunoBrowserExecutorServiceInput, GatewayError> {
    let base_url = input
        .get("baseUrl")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            missing_browser_executor_field_error(
                "Suno",
                "baseUrl",
                "browser_executor_missing_base_url",
            )
        })?;
    let request_body = input.get("requestBody").cloned().ok_or_else(|| {
        missing_browser_executor_field_error(
            "Suno",
            "requestBody",
            "browser_executor_missing_request_body",
        )
    })?;

    Ok(PreparedSunoBrowserExecutorServiceInput {
        base_url,
        headers: build_browser_executor_header_map(input),
        request_body,
        target_asset_kind: input
            .get("targetAssetKind")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("audio")
            .to_string(),
        wait_completion: read_json_bool(input, "waitCompletion").unwrap_or(true),
        wait_timeout: Duration::from_millis(
            read_json_u64(input, "waitTimeoutMs").unwrap_or(150_000),
        ),
        poll_interval: Duration::from_millis(
            read_json_u64(input, "pollIntervalMs").unwrap_or(3_000),
        ),
        timeout: Duration::from_millis(read_json_u64(input, "timeoutMs").unwrap_or(300_000)),
    })
}
#[cfg(test)]
mod tests;
