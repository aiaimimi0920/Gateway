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
    if !base_headers.contains_key(rquest::header::COOKIE) {
        return Err(missing_runtime_cookie_error(browser_backed));
    }
    if !base_headers.contains_key(rquest::header::AUTHORIZATION) {
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
mod tests {
    use std::collections::HashMap;
    use std::time::Duration;

    use crate::protocol::suno;
    use rquest::header::{HeaderMap, HeaderValue};
    use rquest::Method;
    use serde_json::json;

    use super::{
        build_suno_browser_executor_payload, build_suno_browser_worker_input,
        build_suno_challenge_check_plan, build_suno_feed_poll_plan, build_suno_generate_plan,
        build_suno_runtime_headers, missing_runtime_bearer_error, missing_runtime_cookie_error,
        prepare_suno_browser_executor_service_input, prepare_suno_execution_context,
        prepare_suno_execution_plan, prepare_suno_request_context, suno_user_tier,
        unsupported_request_plan_error, unsupported_suno_edit_endpoint_error,
        unsupported_suno_image_inputs_error, unsupported_suno_media_generation_endpoint_error,
        unsupported_suno_video_count_error, validate_suno_media_request,
    };
    use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind, ProtocolFamily};
    use crate::routing::candidate::ProviderAccountPayload;
    use crate::upstream::common::RequestPlan;

    fn make_request(endpoint_kind: EndpointKind) -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind,
            requested_model: Some("chirp-v3-5".to_string()),
            stream: false,
            messages: Vec::new(),
            tools: Vec::new(),
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }

    fn make_payload(adapter: &str, base_url: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
            adapter: adapter.to_string(),
            base_url: base_url.to_string(),
            api_key: "sk-test".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: None,
            headers: HashMap::new(),
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: None,
            chat_completions_path: None,
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            extra_body: None,
            session_auth: None,
            keepalive: None,
        }
    }

    fn assert_suno_request_plan(
        plan: &RequestPlan,
        expected_url: &str,
        expected_response_kind: EndpointKind,
    ) {
        assert_eq!(plan.method, Method::POST);
        assert_eq!(plan.url, expected_url);
        assert_eq!(plan.response_kind, expected_response_kind);
        assert!(plan.query.is_empty());
    }

    #[test]
    fn unsupported_request_plan_error_matches_contract() {
        let error = unsupported_request_plan_error();
        assert_eq!(error.http_status, Some(400));
        assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
        assert_eq!(error.code.as_deref(), Some("unsupported_suno_endpoint"));
    }

    #[test]
    fn plan_suno_chat_endpoint_rejected_locally() {
        let payload = make_payload("suno_compatible", "https://studio-api-prod.suno.com");
        let req = make_request(EndpointKind::ChatCompletions);
        let err = crate::upstream::client::UpstreamClient::build_request_plan(
            &payload,
            &req,
            "chirp-v3-5",
            false,
        )
        .expect_err("suno chat requests should be rejected");
        assert_eq!(err.http_status, Some(400));
        assert_eq!(err.code.as_deref(), Some("unsupported_suno_endpoint"));
    }

    #[test]
    fn build_suno_challenge_check_plan_uses_check_endpoint() {
        let plan = build_suno_challenge_check_plan(
            "https://studio-api-prod.suno.com/",
            EndpointKind::ImagesGenerations,
        );
        assert_suno_request_plan(
            &plan,
            "https://studio-api-prod.suno.com/api/c/check",
            EndpointKind::ImagesGenerations,
        );
        assert_eq!(
            plan.body.as_ref().unwrap(),
            &suno::build_challenge_check_request()
        );
    }

    #[test]
    fn build_suno_generate_plan_uses_v2_web_endpoint() {
        let mut req = make_request(EndpointKind::MusicGenerations);
        req.raw_body = json!({
            "prompt": "lush synthwave chorus"
        });
        let plan = build_suno_generate_plan(
            "https://studio-api-prod.suno.com/",
            &req,
            "chirp-v3-5",
            Some("pro"),
        )
        .expect("suno generate plan");
        assert_suno_request_plan(
            &plan,
            "https://studio-api-prod.suno.com/api/generate/v2-web/",
            EndpointKind::MusicGenerations,
        );
        let body = plan.body.as_ref().expect("expected suno generate body");
        assert_eq!(body["gpt_description_prompt"], "lush synthwave chorus");
        assert_eq!(body["mv"], "chirp-auk-turbo");
        assert_eq!(body["generation_type"], "TEXT");
        assert_eq!(body["metadata"]["user_tier"], "pro");
        assert_eq!(body["metadata"]["web_client_pathname"], "/create");
        assert!(
            body["transaction_uuid"].as_str().is_some(),
            "generate plan should include a transaction uuid"
        );
        assert!(
            body["metadata"]["create_session_token"].as_str().is_some(),
            "generate plan should include a create session token"
        );
    }

    #[test]
    fn build_suno_feed_poll_plan_uses_feed_v3_endpoint() {
        let clip_ids = vec!["clip_a".to_string(), "clip_b".to_string()];
        let plan = build_suno_feed_poll_plan(
            "https://studio-api-prod.suno.com/",
            EndpointKind::VideosGenerations,
            &clip_ids,
        );
        assert_suno_request_plan(
            &plan,
            "https://studio-api-prod.suno.com/api/feed/v3",
            EndpointKind::VideosGenerations,
        );
        assert_eq!(
            plan.body.as_ref().unwrap(),
            &suno::build_feed_poll_request(&clip_ids)
        );
    }

    #[test]
    fn missing_runtime_cookie_error_matches_contracts() {
        let regular = missing_runtime_cookie_error(false);
        assert_eq!(regular.http_status, Some(500));
        assert_eq!(regular.code.as_deref(), Some("missing_suno_runtime_cookie"));
        assert_eq!(
            regular.message.as_str(),
            "Suno requests require runtime Cookie headers from keepalive ensure."
        );

        let browser = missing_runtime_cookie_error(true);
        assert_eq!(browser.http_status, Some(500));
        assert_eq!(browser.code.as_deref(), Some("missing_suno_runtime_cookie"));
        assert_eq!(
            browser.message.as_str(),
            "Suno browser-backed requests require runtime Cookie headers from keepalive ensure."
        );
    }

    #[test]
    fn missing_runtime_bearer_error_matches_contracts() {
        let regular = missing_runtime_bearer_error(false);
        assert_eq!(regular.http_status, Some(500));
        assert_eq!(regular.code.as_deref(), Some("missing_suno_runtime_bearer"));
        assert_eq!(
            regular.message.as_str(),
            "Suno requests require a runtime Clerk bearer token from keepalive ensure."
        );

        let browser = missing_runtime_bearer_error(true);
        assert_eq!(browser.http_status, Some(500));
        assert_eq!(browser.code.as_deref(), Some("missing_suno_runtime_bearer"));
        assert_eq!(
            browser.message.as_str(),
            "Suno browser-backed requests require a runtime Clerk bearer token from keepalive ensure."
        );
    }

    #[test]
    fn unsupported_suno_edit_endpoint_error_matches_contract() {
        let error = unsupported_suno_edit_endpoint_error();
        assert_eq!(error.http_status, Some(400));
        assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
        assert_eq!(
            error.code.as_deref(),
            Some("unsupported_suno_edit_endpoint")
        );
        assert_eq!(
            error.message.as_str(),
            "Suno adapters do not currently support /v1/images/edits."
        );
    }

    #[test]
    fn unsupported_suno_media_generation_endpoint_error_matches_contract() {
        let error = unsupported_suno_media_generation_endpoint_error();
        assert_eq!(error.http_status, Some(400));
        assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
        assert_eq!(error.code.as_deref(), Some("unsupported_suno_endpoint"));
        assert_eq!(
            error.message.as_str(),
            "Suno adapters currently support only /v1/images/generations, /v1/videos/generations, and /v1/music/generations."
        );
    }

    #[test]
    fn unsupported_suno_image_inputs_error_matches_contract() {
        let error = unsupported_suno_image_inputs_error();
        assert_eq!(error.http_status, Some(400));
        assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
        assert_eq!(error.code.as_deref(), Some("unsupported_suno_image_inputs"));
        assert_eq!(
            error.message.as_str(),
            "Suno image generation currently does not support uploaded image or mask inputs."
        );
    }

    #[test]
    fn unsupported_suno_video_count_error_matches_contract() {
        let error = unsupported_suno_video_count_error();
        assert_eq!(error.http_status, Some(400));
        assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
        assert_eq!(error.code.as_deref(), Some("unsupported_suno_video_count"));
        assert_eq!(
            error.message.as_str(),
            "Suno video generation currently supports only n=1 requests."
        );
    }

    #[test]
    fn validate_suno_media_request_rejects_uploaded_image_inputs() {
        let mut req = make_request(EndpointKind::ImagesGenerations);
        req.raw_body = json!({
            "prompt": "cover art",
            "image": "data:image/png;base64,aGVsbG8=",
        });
        let err = validate_suno_media_request(&req)
            .expect_err("uploaded image inputs should be rejected");
        assert_eq!(err.http_status, Some(400));
        assert_eq!(err.code.as_deref(), Some("unsupported_suno_image_inputs"));
    }

    #[test]
    fn validate_suno_media_request_rejects_unsupported_endpoint() {
        let req = make_request(EndpointKind::ChatCompletions);
        let err =
            validate_suno_media_request(&req).expect_err("non-media endpoints should be rejected");
        assert_eq!(err.http_status, Some(400));
        assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
        assert_eq!(err.code.as_deref(), Some("unsupported_suno_endpoint"));
        assert_eq!(
            err.message.as_str(),
            "Suno adapters currently support only /v1/images/generations, /v1/videos/generations, and /v1/music/generations."
        );
    }

    #[test]
    fn validate_suno_media_request_rejects_multi_video_output() {
        let mut req = make_request(EndpointKind::VideosGenerations);
        req.raw_body = json!({
            "prompt": "cinematic stage clip",
            "n": 2,
        });
        let err =
            validate_suno_media_request(&req).expect_err("n>1 video requests should be rejected");
        assert_eq!(err.http_status, Some(400));
        assert_eq!(err.code.as_deref(), Some("unsupported_suno_video_count"));
    }

    #[test]
    fn build_suno_runtime_headers_sets_browser_contract() {
        let mut base_headers = HeaderMap::new();
        base_headers.insert("cookie", HeaderValue::from_static("a=b"));
        let headers = build_suno_runtime_headers(&base_headers);
        assert_eq!(
            headers.get("origin").and_then(|value| value.to_str().ok()),
            Some("https://suno.com")
        );
        assert_eq!(
            headers.get("referer").and_then(|value| value.to_str().ok()),
            Some("https://suno.com/")
        );
        assert_eq!(
            headers
                .get("referring-pathname")
                .and_then(|value| value.to_str().ok()),
            Some("/")
        );
        assert_eq!(
            headers
                .get("referring-origin")
                .and_then(|value| value.to_str().ok()),
            Some("https://suno.com")
        );
        assert!(
            headers.get("device-id").is_some(),
            "runtime headers should synthesize a device id when missing"
        );
        assert!(
            headers.get("browser-token").is_some(),
            "runtime headers should synthesize a browser token"
        );
    }

    #[test]
    fn suno_user_tier_reads_camel_and_snake_case_extra_body_fields() {
        let mut payload = make_payload("suno_compatible", "https://studio-api-prod.suno.com");
        payload.extra_body = Some(HashMap::from([("userTier".to_string(), json!("  pro  "))]));
        assert_eq!(suno_user_tier(&payload).as_deref(), Some("pro"));

        payload.extra_body = Some(HashMap::from([("user_tier".to_string(), json!("free"))]));
        assert_eq!(suno_user_tier(&payload).as_deref(), Some("free"));
    }

    #[test]
    fn prepare_suno_request_context_trims_base_url_and_preserves_headers() {
        let mut payload = make_payload("suno_compatible", "https://studio-api-prod.suno.com/");
        payload.session_auth = Some(crate::credential_runtime::SessionAuthConfig {
            transport: "bearer".to_string(),
            primary_cookie_name: None,
            secondary_cookie_name: None,
            header_name: Some("authorization".to_string()),
            expires_at: None,
        });
        payload
            .headers
            .insert("cookie".to_string(), "a=b".to_string());
        let mut req = make_request(EndpointKind::ImagesGenerations);
        req.raw_body = json!({
            "prompt": "cover art"
        });

        let context = prepare_suno_request_context(&payload, &req, None, false)
            .expect("prepared suno request context");

        assert_eq!(context.prompt, "cover art");
        assert_eq!(context.base_url, "https://studio-api-prod.suno.com");
        assert_eq!(
            context
                .base_headers
                .get("cookie")
                .and_then(|value| value.to_str().ok()),
            Some("a=b")
        );
        assert_eq!(
            context
                .base_headers
                .get("authorization")
                .and_then(|value| value.to_str().ok()),
            Some("Bearer sk-test")
        );
    }

    #[test]
    fn prepare_suno_request_context_requires_bearer_for_browser_backed_mode() {
        let mut payload = make_payload("suno_compatible", "https://studio-api-prod.suno.com/");
        payload
            .headers
            .insert("cookie".to_string(), "a=b".to_string());
        let mut req = make_request(EndpointKind::MusicGenerations);
        req.raw_body = json!({
            "prompt": "neon synth chorus"
        });

        let error = prepare_suno_request_context(&payload, &req, None, true)
            .expect_err("missing bearer should fail");
        assert_eq!(error.http_status, Some(500));
        assert_eq!(error.code.as_deref(), Some("missing_suno_runtime_bearer"));
        assert_eq!(
            error.message.as_str(),
            "Suno browser-backed requests require a runtime Clerk bearer token from keepalive ensure."
        );
    }

    #[test]
    fn prepare_suno_execution_plan_reads_image_contract() {
        let mut req = make_request(EndpointKind::ImagesGenerations);
        req.raw_body = json!({
            "prompt": "cover art",
            "waitCompletion": false,
            "waitTimeoutSecs": 5,
            "pollIntervalMs": 500
        });

        let prepared = prepare_suno_execution_plan(&req, Duration::from_secs(30), false)
            .expect("suno execution plan");

        assert!(prepared.missing_challenge_token);
        assert!(!prepared.wait_completion);
        assert_eq!(prepared.wait_timeout, Duration::from_secs(10));
        assert_eq!(prepared.poll_interval, Duration::from_millis(1_000));
        assert_eq!(prepared.request_timeout, Duration::from_secs(240));
        assert_eq!(prepared.target_asset_kind, "image");
    }

    #[test]
    fn prepare_suno_execution_plan_reads_browser_video_contract() {
        let mut req = make_request(EndpointKind::VideosGenerations);
        req.raw_body = json!({
            "prompt": "cinematic teaser",
            "token": "captcha-ok",
            "wait_timeout_secs": 700,
            "poll_secs": 2
        });

        let prepared = prepare_suno_execution_plan(&req, Duration::from_secs(45), true)
            .expect("suno browser execution plan");

        assert!(!prepared.missing_challenge_token);
        assert!(prepared.wait_completion);
        assert_eq!(prepared.wait_timeout, Duration::from_secs(600));
        assert_eq!(prepared.poll_interval, Duration::from_millis(2_000));
        assert_eq!(prepared.request_timeout, Duration::from_secs(300));
        assert_eq!(prepared.target_asset_kind, "video");
    }

    #[test]
    fn prepare_suno_execution_context_reads_http_contract() {
        let mut payload = make_payload("suno_compatible", "https://studio-api-prod.suno.com/");
        payload.session_auth = Some(crate::credential_runtime::SessionAuthConfig {
            transport: "bearer".to_string(),
            primary_cookie_name: None,
            secondary_cookie_name: None,
            header_name: Some("authorization".to_string()),
            expires_at: None,
        });
        payload
            .headers
            .insert("cookie".to_string(), "a=b".to_string());
        payload.extra_body = Some(HashMap::from([("userTier".to_string(), json!("pro"))]));
        let mut req = make_request(EndpointKind::ImagesGenerations);
        req.raw_body = json!({
            "prompt": "cover art"
        });

        let prepared =
            prepare_suno_execution_context(&payload, &req, None, false, Duration::from_secs(30))
                .expect("suno execution context");

        assert_eq!(prepared.prompt, "cover art");
        assert_eq!(prepared.base_url, "https://studio-api-prod.suno.com");
        assert_eq!(prepared.user_tier.as_deref(), Some("pro"));
        assert_eq!(prepared.execution_plan.target_asset_kind, "image");
        assert_eq!(
            prepared.execution_plan.request_timeout,
            Duration::from_secs(240)
        );
        assert_eq!(
            prepared
                .runtime_headers
                .get("authorization")
                .and_then(|value| value.to_str().ok()),
            Some("Bearer sk-test")
        );
        assert_eq!(
            prepared
                .runtime_headers
                .get("origin")
                .and_then(|value| value.to_str().ok()),
            Some("https://suno.com")
        );
        assert!(prepared.runtime_headers.get("device-id").is_some());
    }

    #[test]
    fn prepare_suno_execution_context_reads_browser_contract() {
        let mut payload = make_payload("suno_compatible", "https://studio-api-prod.suno.com/");
        payload.session_auth = Some(crate::credential_runtime::SessionAuthConfig {
            transport: "bearer".to_string(),
            primary_cookie_name: None,
            secondary_cookie_name: None,
            header_name: Some("authorization".to_string()),
            expires_at: None,
        });
        payload
            .headers
            .insert("cookie".to_string(), "a=b".to_string());
        let mut req = make_request(EndpointKind::VideosGenerations);
        req.raw_body = json!({
            "prompt": "cinematic teaser",
            "token": "captcha-ok"
        });

        let prepared =
            prepare_suno_execution_context(&payload, &req, None, true, Duration::from_secs(45))
                .expect("suno browser execution context");

        assert_eq!(prepared.execution_plan.target_asset_kind, "video");
        assert_eq!(
            prepared.execution_plan.request_timeout,
            Duration::from_secs(300)
        );
        assert!(!prepared.execution_plan.missing_challenge_token);
        assert_eq!(
            prepared
                .runtime_headers
                .get("referer")
                .and_then(|value| value.to_str().ok()),
            Some("https://suno.com/")
        );
        assert!(prepared.runtime_headers.get("browser-token").is_some());
    }

    #[test]
    fn build_suno_browser_executor_payload_preserves_runtime_header_and_wait_contract() {
        let mut runtime_headers = HeaderMap::new();
        runtime_headers.insert("cookie", HeaderValue::from_static("a=b"));
        runtime_headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer token-123"),
        );
        runtime_headers.insert("user-agent", HeaderValue::from_static("suno-agent"));
        runtime_headers.insert("accept-language", HeaderValue::from_static("en-US"));
        runtime_headers.insert("origin", HeaderValue::from_static("https://suno.com"));
        runtime_headers.insert(
            "referer",
            HeaderValue::from_static("https://suno.com/create"),
        );
        runtime_headers.insert("device-id", HeaderValue::from_static("device-1"));
        runtime_headers.insert("browser-token", HeaderValue::from_static("browser-1"));
        runtime_headers.insert("referring-pathname", HeaderValue::from_static("/create"));
        runtime_headers.insert(
            "referring-origin",
            HeaderValue::from_static("https://suno.com"),
        );

        let mut req = make_request(EndpointKind::VideosGenerations);
        req.raw_body = json!({
            "prompt": "cinematic stage clip",
            "wait_completion": false,
        });

        let payload = build_suno_browser_executor_payload(
            "https://studio-api-prod.suno.com",
            &runtime_headers,
            &req,
            "video",
            std::time::Duration::from_secs(150),
            std::time::Duration::from_secs(3),
            std::time::Duration::from_secs(300),
            Some("C:/browser/chrome.exe".to_string()),
        );

        assert_eq!(payload["baseUrl"], "https://studio-api-prod.suno.com");
        assert_eq!(payload["cookieHeader"], "a=b");
        assert_eq!(payload["authToken"], "token-123");
        assert_eq!(payload["requestBody"]["prompt"], "cinematic stage clip");
        assert_eq!(payload["targetAssetKind"], "video");
        assert_eq!(payload["waitCompletion"], false);
        assert_eq!(payload["waitTimeoutMs"], 150_000u64);
        assert_eq!(payload["pollIntervalMs"], 3_000u64);
        assert_eq!(payload["timeoutMs"], 300_000u64);
        assert_eq!(payload["browserExecutablePath"], "C:/browser/chrome.exe");
        assert_eq!(payload["userAgent"], "suno-agent");
        assert_eq!(payload["acceptLanguage"], "en-US");
        assert_eq!(payload["origin"], "https://suno.com");
        assert_eq!(payload["referer"], "https://suno.com/create");
        assert_eq!(payload["deviceId"], "device-1");
        assert_eq!(payload["browserToken"], "browser-1");
        assert_eq!(payload["referringPathname"], "/create");
        assert_eq!(payload["referringOrigin"], "https://suno.com");
    }

    #[test]
    fn build_suno_browser_worker_input_preserves_runtime_header_and_wait_contract() {
        let mut runtime_headers = HeaderMap::new();
        runtime_headers.insert("cookie", HeaderValue::from_static("a=b"));
        runtime_headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer token-123"),
        );
        runtime_headers.insert("user-agent", HeaderValue::from_static("suno-agent"));
        runtime_headers.insert("accept-language", HeaderValue::from_static("en-US"));
        runtime_headers.insert("origin", HeaderValue::from_static("https://suno.com"));
        runtime_headers.insert(
            "referer",
            HeaderValue::from_static("https://suno.com/create"),
        );
        runtime_headers.insert("device-id", HeaderValue::from_static("device-1"));
        runtime_headers.insert("browser-token", HeaderValue::from_static("browser-1"));
        runtime_headers.insert("referring-pathname", HeaderValue::from_static("/create"));
        runtime_headers.insert(
            "referring-origin",
            HeaderValue::from_static("https://suno.com"),
        );
        let request_body = json!({ "prompt": "cinematic stage clip" });

        let input = build_suno_browser_worker_input(
            "https://studio-api-prod.suno.com",
            &runtime_headers,
            &request_body,
            "video",
            false,
            std::time::Duration::from_secs(150),
            std::time::Duration::from_secs(3),
            std::time::Duration::from_secs(300),
            Some("C:/browser/chrome.exe".to_string()),
        );
        let payload = serde_json::to_value(&input).expect("suno worker input");

        assert_eq!(payload["baseUrl"], "https://studio-api-prod.suno.com");
        assert_eq!(payload["cookieHeader"], "a=b");
        assert_eq!(payload["authToken"], "token-123");
        assert_eq!(payload["requestBody"]["prompt"], "cinematic stage clip");
        assert_eq!(payload["targetAssetKind"], "video");
        assert_eq!(payload["waitCompletion"], false);
        assert_eq!(payload["waitTimeoutMs"], 150_000u64);
        assert_eq!(payload["pollIntervalMs"], 3_000u64);
        assert_eq!(payload["timeoutMs"], 300_000u64);
        assert_eq!(payload["browserExecutablePath"], "C:/browser/chrome.exe");
        assert_eq!(payload["userAgent"], "suno-agent");
        assert_eq!(payload["acceptLanguage"], "en-US");
        assert_eq!(payload["origin"], "https://suno.com");
        assert_eq!(payload["referer"], "https://suno.com/create");
        assert_eq!(payload["deviceId"], "device-1");
        assert_eq!(payload["browserToken"], "browser-1");
        assert_eq!(payload["referringPathname"], "/create");
        assert_eq!(payload["referringOrigin"], "https://suno.com");
    }

    #[test]
    fn prepare_suno_browser_executor_service_input_preserves_runtime_and_wait_contract() {
        let input = json!({
            "baseUrl": "https://studio-api-prod.suno.com",
            "cookieHeader": "a=b",
            "authToken": "token-123",
            "requestBody": { "prompt": "cinematic stage clip" },
            "targetAssetKind": "video",
            "waitCompletion": false,
            "waitTimeoutMs": 150_000u64,
            "pollIntervalMs": 3_000u64,
            "timeoutMs": 300_000u64,
            "userAgent": "suno-agent",
            "acceptLanguage": "en-US",
            "origin": "https://suno.com",
            "referer": "https://suno.com/create",
            "deviceId": "device-1",
            "browserToken": "browser-1",
            "referringPathname": "/create",
            "referringOrigin": "https://suno.com"
        });

        let prepared =
            prepare_suno_browser_executor_service_input(&input).expect("suno service input");

        assert_eq!(prepared.base_url, "https://studio-api-prod.suno.com");
        assert_eq!(prepared.request_body["prompt"], "cinematic stage clip");
        assert_eq!(prepared.target_asset_kind, "video");
        assert!(!prepared.wait_completion);
        assert_eq!(prepared.wait_timeout, std::time::Duration::from_secs(150));
        assert_eq!(prepared.poll_interval, std::time::Duration::from_secs(3));
        assert_eq!(prepared.timeout, std::time::Duration::from_secs(300));
        assert_eq!(
            crate::upstream::header_map_helpers::header_map_string(
                &prepared.headers,
                "authorization"
            )
            .as_deref(),
            Some("Bearer token-123")
        );
        assert_eq!(
            crate::upstream::header_map_helpers::header_map_string(&prepared.headers, "cookie")
                .as_deref(),
            Some("a=b")
        );
        assert_eq!(
            crate::upstream::header_map_helpers::header_map_string(
                &prepared.headers,
                "browser-token"
            )
            .as_deref(),
            Some("browser-1")
        );
    }

    #[test]
    fn prepare_suno_browser_executor_service_input_requires_request_body_contract() {
        let input = json!({
            "baseUrl": "https://studio-api-prod.suno.com"
        });

        let error = prepare_suno_browser_executor_service_input(&input)
            .expect_err("missing request body should fail");
        assert_eq!(
            error.code.as_deref(),
            Some("browser_executor_missing_request_body")
        );
        assert_eq!(error.http_status, Some(400));
    }
}
