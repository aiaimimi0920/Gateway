use serde_json::{json, Value};

use crate::error::{classify_upstream_error, GatewayError};
use crate::upstream::browser_worker_types::{SunoBrowserWorkerResult, SunoBrowserWorkerSuccess};
use crate::upstream::response_preview_helpers::truncate_response_preview;

pub(crate) fn parse_suno_json_response(
    body_text: &str,
    message: &str,
    code: &'static str,
) -> Result<Value, GatewayError> {
    serde_json::from_str(body_text).map_err(|error| {
        let detail = if body_text.trim().is_empty() {
            "<empty body>".to_string()
        } else {
            truncate_response_preview(body_text, 200).to_string()
        };
        GatewayError::server_error(format!("{message} {error}; body={detail}"))
            .with_provider("suno_compatible")
            .with_code(code)
    })
}

pub(crate) fn parse_suno_challenge_probe_body(body_text: &str) -> Result<Value, GatewayError> {
    parse_suno_json_response(
        body_text,
        "Suno challenge probe response body was not valid JSON.",
        "suno_invalid_challenge_probe_body",
    )
}

pub(crate) fn parse_suno_challenge_probe_http_response(
    status: u16,
    headers: &rquest::header::HeaderMap,
    body_text: &str,
    missing_challenge_token: bool,
) -> Result<Value, GatewayError> {
    ensure_successful_suno_upstream_status(status, headers, body_text, missing_challenge_token)?;
    parse_suno_challenge_probe_body(body_text)
}

pub(crate) fn parse_suno_challenge_probe_verified_response(
    status: u16,
    headers: &rquest::header::HeaderMap,
    body_text: &str,
    missing_challenge_token: bool,
) -> Result<Value, GatewayError> {
    let body = parse_suno_challenge_probe_http_response(
        status,
        headers,
        body_text,
        missing_challenge_token,
    )?;
    if crate::protocol::suno::challenge_required(&body)? && missing_challenge_token {
        return Err(suno_challenge_required_error());
    }
    Ok(body)
}

pub(crate) fn parse_suno_generation_body(body_text: &str) -> Result<Value, GatewayError> {
    parse_suno_json_response(
        body_text,
        "Suno generation response body was not valid JSON.",
        "suno_invalid_generation_body",
    )
}

pub(crate) fn parse_suno_generation_http_response(
    status: u16,
    headers: &rquest::header::HeaderMap,
    body_text: &str,
    missing_challenge_token: bool,
) -> Result<Value, GatewayError> {
    ensure_successful_suno_upstream_status(status, headers, body_text, missing_challenge_token)?;
    parse_suno_generation_body(body_text)
}

pub(crate) fn parse_suno_generation_http_clips_response(
    status: u16,
    headers: &rquest::header::HeaderMap,
    body_text: &str,
    missing_challenge_token: bool,
) -> Result<(Value, Vec<crate::protocol::suno::SunoClip>), GatewayError> {
    let body =
        parse_suno_generation_http_response(status, headers, body_text, missing_challenge_token)?;
    let clips = crate::protocol::suno::extract_clips_from_feed(&body)?;
    Ok((body, clips))
}

pub(crate) fn parse_suno_generation_http_poll_seed(
    status: u16,
    headers: &rquest::header::HeaderMap,
    body_text: &str,
    missing_challenge_token: bool,
) -> Result<(Vec<crate::protocol::suno::SunoClip>, Vec<String>), GatewayError> {
    let (body, clips) = parse_suno_generation_http_clips_response(
        status,
        headers,
        body_text,
        missing_challenge_token,
    )?;
    let clip_ids = crate::protocol::suno::extract_clip_ids(&body)?;
    Ok((clips, clip_ids))
}

pub(crate) fn parse_suno_feed_body(body_text: &str) -> Result<Value, GatewayError> {
    parse_suno_json_response(
        body_text,
        "Suno feed poll response body was not valid JSON.",
        "suno_invalid_feed_body",
    )
}

pub(crate) fn parse_suno_feed_http_response(
    status: u16,
    headers: &rquest::header::HeaderMap,
    body_text: &str,
    missing_challenge_token: bool,
) -> Result<Value, GatewayError> {
    ensure_successful_suno_upstream_status(status, headers, body_text, missing_challenge_token)?;
    parse_suno_feed_body(body_text)
}

pub(crate) fn parse_suno_feed_http_clips_response(
    status: u16,
    headers: &rquest::header::HeaderMap,
    body_text: &str,
    missing_challenge_token: bool,
) -> Result<Vec<crate::protocol::suno::SunoClip>, GatewayError> {
    let body = parse_suno_feed_http_response(status, headers, body_text, missing_challenge_token)?;
    crate::protocol::suno::extract_clips_from_feed(&body)
}

pub(crate) fn parse_suno_remote_browser_worker_success(
    result: Value,
) -> Result<SunoBrowserWorkerSuccess, GatewayError> {
    serde_json::from_value::<SunoBrowserWorkerSuccess>(result).map_err(|error| {
        crate::protocol::suno::remote_browser_worker_result_parse_error(error.to_string().as_str())
    })
}

pub(crate) fn parse_suno_remote_browser_worker_verified_result(
    result: Value,
) -> Result<SunoBrowserWorkerSuccess, GatewayError> {
    let worker = parse_suno_remote_browser_worker_success(result)?;
    if worker.clips.is_empty() {
        return Err(crate::protocol::suno::missing_browser_worker_clips_error());
    }
    Ok(worker)
}

pub(crate) fn parse_suno_browser_worker_output(
    stdout: &str,
    stderr: &str,
) -> Result<SunoBrowserWorkerResult, GatewayError> {
    if stdout.trim().is_empty() {
        return Err(crate::protocol::suno::empty_browser_worker_output_error(
            stderr,
        ));
    }

    serde_json::from_str::<SunoBrowserWorkerResult>(stdout).map_err(|error| {
        crate::protocol::suno::browser_worker_output_parse_error(error.to_string().as_str(), stdout)
    })
}

pub(crate) fn parse_suno_browser_worker_verified_output(
    stdout: &str,
    stderr: &str,
    provider: &str,
) -> Result<SunoBrowserWorkerSuccess, GatewayError> {
    let result = parse_suno_browser_worker_output(stdout, stderr)?;
    resolve_suno_browser_worker_result(result, provider)
}

pub(crate) fn extract_suno_browser_worker_success(
    result: SunoBrowserWorkerResult,
) -> Result<SunoBrowserWorkerSuccess, GatewayError> {
    let payload = result
        .result
        .ok_or_else(crate::protocol::suno::missing_browser_worker_result_error)?;
    if payload.clips.is_empty() {
        return Err(crate::protocol::suno::missing_browser_worker_clips_error());
    }
    Ok(payload)
}

pub(crate) fn resolve_suno_browser_worker_result(
    result: SunoBrowserWorkerResult,
    provider: &str,
) -> Result<SunoBrowserWorkerSuccess, GatewayError> {
    if result.ok {
        return extract_suno_browser_worker_success(result);
    }

    Err(classify_suno_browser_worker_failure(result, provider))
}

pub(crate) fn build_suno_browser_executor_service_result(
    result: &SunoBrowserWorkerSuccess,
) -> serde_json::Value {
    json!({
        "clips": result.clips,
        "completed": result.completed,
        "message": result.message,
    })
}

pub(crate) fn resolve_suno_image_generation_plan(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    prompt: &str,
    clips: &[crate::protocol::suno::SunoClip],
) -> Result<(Vec<String>, Option<Value>), GatewayError> {
    let image_urls = crate::protocol::suno::image_urls_for_requested_count(req, clips)?;
    let response = if crate::protocol::suno::prefers_url_response(req)? {
        Some(crate::protocol::suno::build_openai_images_response_from_urls(req, prompt, clips)?)
    } else {
        None
    };
    Ok((image_urls, response))
}

pub(crate) fn build_suno_non_image_generation_response(
    endpoint_kind: crate::protocol::canonical::EndpointKind,
    model: &str,
    prompt: &str,
    clips: &[crate::protocol::suno::SunoClip],
    completed: bool,
    message: Option<&str>,
) -> Result<Value, GatewayError> {
    match endpoint_kind {
        crate::protocol::canonical::EndpointKind::VideosGenerations => {
            crate::protocol::suno::build_video_generation_response(
                model, prompt, clips, completed, message,
            )
        }
        crate::protocol::canonical::EndpointKind::MusicGenerations => {
            Ok(crate::protocol::suno::build_music_generation_response(
                model, prompt, clips, completed, message,
            ))
        }
        _ => Err(crate::protocol::suno::unsupported_media_endpoint_error()),
    }
}

pub(crate) fn classify_suno_browser_worker_failure(
    result: SunoBrowserWorkerResult,
    provider: &str,
) -> GatewayError {
    let status = result
        .error
        .as_ref()
        .and_then(|entry| entry.status)
        .or(result.status)
        .unwrap_or(500);
    let body_text = result
        .error
        .as_ref()
        .and_then(|entry| entry.body.as_deref())
        .unwrap_or_default();
    let mut gateway_error = classify_upstream_error(status, body_text, Some(provider));
    if let Some(error) = result.error {
        if let Some(code) = error.code {
            gateway_error.code = Some(code);
        }
        if let Some(message) = error.message {
            gateway_error.message = message;
        }
        if gateway_error.http_status.is_none() {
            gateway_error.http_status = error.status.or(Some(status));
        }
    }
    gateway_error
}

pub(crate) fn suno_challenge_required_error() -> GatewayError {
    GatewayError::server_error(
        "Suno requires an active browser challenge token before generation can continue.",
    )
    .with_provider("suno_compatible")
    .with_code("suno_challenge_required")
}

pub(crate) fn classify_suno_upstream_error(
    status: u16,
    headers: &rquest::header::HeaderMap,
    body_text: &str,
    missing_challenge_token: bool,
) -> GatewayError {
    let provider = "suno_compatible";
    if status == 401 {
        return GatewayError::server_error(
            "Suno session is not authenticated. Capture a fresh browser session cookie before routing requests through the gateway.",
        )
        .with_provider(provider)
        .with_code("suno_session_unauthorized");
    }

    let content_type = headers
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let body_lower = body_text.to_ascii_lowercase();
    let looks_like_html = content_type.contains("text/html")
        || body_lower.contains("<!doctype html")
        || body_lower.contains("<html");
    let looks_like_challenge = body_lower.contains("captcha")
        || body_lower.contains("turnstile")
        || body_lower.contains("challenge")
        || body_lower.contains("cloudflare");
    let body_is_empty = body_text.trim().is_empty();

    if missing_challenge_token
        && matches!(status, 400 | 403 | 422 | 429)
        && (body_is_empty || looks_like_html || looks_like_challenge)
    {
        return suno_challenge_required_error();
    }

    classify_upstream_error(status, body_text, Some(provider))
}

pub(crate) fn classify_suno_media_fetch_error(
    status: u16,
    headers: &rquest::header::HeaderMap,
    body_text: &str,
) -> GatewayError {
    classify_suno_upstream_error(status, headers, body_text, false)
}

pub(crate) fn ensure_successful_suno_media_fetch_status(
    status: u16,
    headers: &rquest::header::HeaderMap,
    body_text: &str,
) -> Result<(), GatewayError> {
    if (200..300).contains(&status) {
        return Ok(());
    }

    Err(classify_suno_media_fetch_error(status, headers, body_text))
}

pub(crate) fn ensure_successful_suno_upstream_status(
    status: u16,
    headers: &rquest::header::HeaderMap,
    body_text: &str,
    missing_challenge_token: bool,
) -> Result<(), GatewayError> {
    if (200..300).contains(&status) {
        return Ok(());
    }

    Err(classify_suno_upstream_error(
        status,
        headers,
        body_text,
        missing_challenge_token,
    ))
}

pub(crate) fn resolve_suno_downloaded_image_mime_type(
    headers: &rquest::header::HeaderMap,
    url: &str,
) -> String {
    headers
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .filter(|value| value.starts_with("image/"))
        .map(ToString::to_string)
        .unwrap_or_else(|| crate::protocol::suno::infer_mime_type_from_url(url, "image/png"))
}

pub(crate) fn materialize_suno_downloaded_image(
    headers: &rquest::header::HeaderMap,
    url: &str,
    bytes: &[u8],
) -> (String, Vec<u8>) {
    (
        resolve_suno_downloaded_image_mime_type(headers, url),
        bytes.to_vec(),
    )
}

pub(crate) fn build_suno_downloaded_images_response(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    prompt: &str,
    images: &[(String, Vec<u8>)],
) -> Result<Value, GatewayError> {
    crate::protocol::suno::build_openai_images_response_from_bytes(req, prompt, images)
}

#[cfg(test)]
mod tests {
    use super::{
        build_suno_browser_executor_service_result, build_suno_downloaded_images_response,
        build_suno_non_image_generation_response, classify_suno_browser_worker_failure,
        classify_suno_media_fetch_error, classify_suno_upstream_error,
        ensure_successful_suno_media_fetch_status, ensure_successful_suno_upstream_status,
        extract_suno_browser_worker_success, materialize_suno_downloaded_image,
        parse_suno_browser_worker_output, parse_suno_browser_worker_verified_output,
        parse_suno_challenge_probe_body, parse_suno_challenge_probe_http_response,
        parse_suno_challenge_probe_verified_response, parse_suno_feed_body,
        parse_suno_feed_http_clips_response, parse_suno_feed_http_response,
        parse_suno_generation_body, parse_suno_generation_http_clips_response,
        parse_suno_generation_http_poll_seed, parse_suno_generation_http_response,
        parse_suno_remote_browser_worker_success, parse_suno_remote_browser_worker_verified_result,
        resolve_suno_browser_worker_result, resolve_suno_downloaded_image_mime_type,
        resolve_suno_image_generation_plan, suno_challenge_required_error,
    };
    use crate::upstream::browser_worker_types::SunoBrowserWorkerResult;
    use serde_json::json;

    fn image_generation_request(
        raw_body: serde_json::Value,
    ) -> crate::protocol::canonical::CanonicalRelayRequest {
        crate::protocol::canonical::CanonicalRelayRequest {
            protocol_family: crate::protocol::canonical::ProtocolFamily::OpenAi,
            endpoint_kind: crate::protocol::canonical::EndpointKind::ImagesGenerations,
            requested_model: None,
            stream: false,
            messages: Vec::new(),
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
    fn suno_challenge_required_error_matches_contract() {
        let error = suno_challenge_required_error();
        assert_eq!(error.http_status, Some(500));
        assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
        assert_eq!(error.code.as_deref(), Some("suno_challenge_required"));
        assert_eq!(
            error.message.as_str(),
            "Suno requires an active browser challenge token before generation can continue."
        );
    }

    #[test]
    fn classify_suno_empty_422_as_challenge_when_token_missing() {
        let headers = rquest::header::HeaderMap::new();
        let err = classify_suno_upstream_error(422, &headers, "", true);
        assert_eq!(err.code.as_deref(), Some("suno_challenge_required"));
        assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
    }

    #[test]
    fn classify_suno_401_as_session_unauthorized() {
        let headers = rquest::header::HeaderMap::new();
        let err =
            classify_suno_upstream_error(401, &headers, "{\"detail\":\"Unauthorized\"}", false);
        assert_eq!(err.code.as_deref(), Some("suno_session_unauthorized"));
        assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
    }

    #[test]
    fn parse_suno_challenge_probe_body_reports_invalid_json_contract() {
        let err =
            parse_suno_challenge_probe_body("not-json").expect_err("invalid JSON should fail");
        assert_eq!(
            err.code.as_deref(),
            Some("suno_invalid_challenge_probe_body")
        );
        assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
        assert!(err
            .message
            .starts_with("Suno challenge probe response body was not valid JSON."));
    }

    #[test]
    fn parse_suno_challenge_probe_http_response_reads_success_contract() {
        let headers = rquest::header::HeaderMap::new();

        let body = parse_suno_challenge_probe_http_response(
            200,
            &headers,
            "{\"requiresChallenge\":false}",
            false,
        )
        .expect("valid challenge response should parse");

        assert_eq!(body["requiresChallenge"], false);
    }

    #[test]
    fn parse_suno_challenge_probe_http_response_preserves_failure_contract() {
        let headers = rquest::header::HeaderMap::new();

        let error = parse_suno_challenge_probe_http_response(
            401,
            &headers,
            "{\"detail\":\"Unauthorized\"}",
            false,
        )
        .expect_err("non-2xx response should fail");

        assert_eq!(error.code.as_deref(), Some("suno_session_unauthorized"));
        assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
    }

    #[test]
    fn parse_suno_challenge_probe_verified_response_reads_success_contract() {
        let headers = rquest::header::HeaderMap::new();

        let body = parse_suno_challenge_probe_verified_response(
            200,
            &headers,
            "{\"required\":false}",
            true,
        )
        .expect("valid challenge probe should pass");

        assert_eq!(body["required"], false);
    }

    #[test]
    fn parse_suno_challenge_probe_verified_response_preserves_required_contract() {
        let headers = rquest::header::HeaderMap::new();

        let error = parse_suno_challenge_probe_verified_response(
            200,
            &headers,
            "{\"required\":true}",
            true,
        )
        .expect_err("required challenge without token should fail");

        assert_eq!(error.code.as_deref(), Some("suno_challenge_required"));
        assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
    }

    #[test]
    fn parse_suno_generation_body_reports_invalid_json_contract() {
        let err = parse_suno_generation_body("not-json").expect_err("invalid JSON should fail");
        assert_eq!(err.code.as_deref(), Some("suno_invalid_generation_body"));
        assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
        assert!(err
            .message
            .starts_with("Suno generation response body was not valid JSON."));
    }

    #[test]
    fn parse_suno_generation_http_response_reads_success_contract() {
        let headers = rquest::header::HeaderMap::new();

        let body = parse_suno_generation_http_response(200, &headers, "{\"clips\":[]}", false)
            .expect("valid generation response should parse");

        assert!(body["clips"].is_array());
    }

    #[test]
    fn parse_suno_generation_http_response_preserves_failure_contract() {
        let headers = rquest::header::HeaderMap::new();

        let error = parse_suno_generation_http_response(
            401,
            &headers,
            "{\"detail\":\"Unauthorized\"}",
            false,
        )
        .expect_err("non-2xx response should fail");

        assert_eq!(error.code.as_deref(), Some("suno_session_unauthorized"));
        assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
    }

    #[test]
    fn parse_suno_generation_http_clips_response_reads_success_contract() {
        let headers = rquest::header::HeaderMap::new();

        let (body, clips) = parse_suno_generation_http_clips_response(
            200,
            &headers,
            "{\"clips\":[{\"id\":\"clip-1\",\"status\":\"complete\"}]}",
            false,
        )
        .expect("valid generation clips response should parse");

        assert!(body["clips"].is_array());
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].id, "clip-1");
    }

    #[test]
    fn parse_suno_generation_http_clips_response_preserves_failure_contract() {
        let headers = rquest::header::HeaderMap::new();

        let error = parse_suno_generation_http_clips_response(
            401,
            &headers,
            "{\"detail\":\"Unauthorized\"}",
            false,
        )
        .expect_err("non-2xx response should fail");

        assert_eq!(error.code.as_deref(), Some("suno_session_unauthorized"));
        assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
    }

    #[test]
    fn parse_suno_generation_http_poll_seed_reads_success_contract() {
        let headers = rquest::header::HeaderMap::new();

        let (clips, clip_ids) = parse_suno_generation_http_poll_seed(
            200,
            &headers,
            "{\"clips\":[{\"id\":\"clip-1\",\"status\":\"complete\"}]}",
            false,
        )
        .expect("valid generation poll seed should parse");

        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].id, "clip-1");
        assert_eq!(clip_ids, vec!["clip-1".to_string()]);
    }

    #[test]
    fn parse_suno_generation_http_poll_seed_preserves_failure_contract() {
        let headers = rquest::header::HeaderMap::new();

        let error = parse_suno_generation_http_poll_seed(
            401,
            &headers,
            "{\"detail\":\"Unauthorized\"}",
            false,
        )
        .expect_err("non-2xx response should fail");

        assert_eq!(error.code.as_deref(), Some("suno_session_unauthorized"));
        assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
    }

    #[test]
    fn parse_suno_feed_body_reports_invalid_json_contract() {
        let err = parse_suno_feed_body("not-json").expect_err("invalid JSON should fail");
        assert_eq!(err.code.as_deref(), Some("suno_invalid_feed_body"));
        assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
        assert!(err
            .message
            .starts_with("Suno feed poll response body was not valid JSON."));
    }

    #[test]
    fn parse_suno_feed_http_response_reads_success_contract() {
        let headers = rquest::header::HeaderMap::new();

        let body = parse_suno_feed_http_response(200, &headers, "{\"clips\":[]}", false)
            .expect("valid feed response should parse");

        assert!(body["clips"].is_array());
    }

    #[test]
    fn parse_suno_feed_http_response_preserves_failure_contract() {
        let headers = rquest::header::HeaderMap::new();

        let error =
            parse_suno_feed_http_response(401, &headers, "{\"detail\":\"Unauthorized\"}", false)
                .expect_err("non-2xx response should fail");

        assert_eq!(error.code.as_deref(), Some("suno_session_unauthorized"));
        assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
    }

    #[test]
    fn parse_suno_feed_http_clips_response_reads_success_contract() {
        let headers = rquest::header::HeaderMap::new();

        let clips = parse_suno_feed_http_clips_response(
            200,
            &headers,
            "{\"clips\":[{\"id\":\"clip-1\",\"status\":\"complete\"}]}",
            false,
        )
        .expect("valid feed clips response should parse");

        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].id, "clip-1");
    }

    #[test]
    fn parse_suno_feed_http_clips_response_preserves_failure_contract() {
        let headers = rquest::header::HeaderMap::new();

        let error = parse_suno_feed_http_clips_response(
            401,
            &headers,
            "{\"detail\":\"Unauthorized\"}",
            false,
        )
        .expect_err("non-2xx response should fail");

        assert_eq!(error.code.as_deref(), Some("suno_session_unauthorized"));
        assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
    }

    #[test]
    fn resolve_suno_image_generation_plan_preserves_url_response_contract() {
        let req = image_generation_request(json!({}));
        let clips = vec![crate::protocol::suno::SunoClip {
            id: "clip-1".to_string(),
            title: Some("Cover".to_string()),
            image_url: Some("https://cdn.example.com/cover.png".to_string()),
            lyric: None,
            audio_url: None,
            video_url: None,
            created_at: None,
            model_name: None,
            prompt: None,
            gpt_description_prompt: None,
            status: "complete".to_string(),
            clip_type: None,
            tags: None,
            negative_tags: None,
            duration: None,
            error_message: None,
        }];

        let (image_urls, response) =
            resolve_suno_image_generation_plan(&req, "cover prompt", &clips).expect("image plan");

        assert_eq!(
            image_urls,
            vec!["https://cdn.example.com/cover.png".to_string()]
        );
        let response = response.expect("url response");
        assert_eq!(
            response["data"][0]["url"],
            "https://cdn.example.com/cover.png"
        );
        assert_eq!(response["data"][0]["revised_prompt"], "cover prompt");
    }

    #[test]
    fn resolve_suno_image_generation_plan_preserves_b64_contract() {
        let req = image_generation_request(json!({ "response_format": "b64_json" }));
        let clips = vec![crate::protocol::suno::SunoClip {
            id: "clip-2".to_string(),
            title: Some("Poster".to_string()),
            image_url: Some("https://cdn.example.com/poster.png".to_string()),
            lyric: None,
            audio_url: None,
            video_url: None,
            created_at: None,
            model_name: None,
            prompt: None,
            gpt_description_prompt: None,
            status: "complete".to_string(),
            clip_type: None,
            tags: None,
            negative_tags: None,
            duration: None,
            error_message: None,
        }];

        let (image_urls, response) =
            resolve_suno_image_generation_plan(&req, "poster prompt", &clips).expect("image plan");

        assert_eq!(
            image_urls,
            vec!["https://cdn.example.com/poster.png".to_string()]
        );
        assert!(response.is_none());
    }

    #[test]
    fn classify_suno_media_fetch_error_preserves_unauthorized_contract() {
        let headers = rquest::header::HeaderMap::new();
        let error = classify_suno_media_fetch_error(401, &headers, "{\"detail\":\"Unauthorized\"}");

        assert_eq!(error.code.as_deref(), Some("suno_session_unauthorized"));
        assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
    }

    #[test]
    fn classify_suno_media_fetch_error_preserves_generic_failure_contract() {
        let headers = rquest::header::HeaderMap::new();
        let error = classify_suno_media_fetch_error(503, &headers, "service unavailable");

        assert_eq!(error.http_status, Some(503));
        assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
        assert!(error.message.contains("service unavailable"));
    }

    #[test]
    fn ensure_successful_suno_media_fetch_status_accepts_2xx_contract() {
        let headers = rquest::header::HeaderMap::new();

        ensure_successful_suno_media_fetch_status(204, &headers, "").expect("2xx should pass");
    }

    #[test]
    fn ensure_successful_suno_media_fetch_status_preserves_failure_contract() {
        let headers = rquest::header::HeaderMap::new();

        let error = ensure_successful_suno_media_fetch_status(
            401,
            &headers,
            "{\"detail\":\"Unauthorized\"}",
        )
        .expect_err("non-2xx should fail");

        assert_eq!(error.code.as_deref(), Some("suno_session_unauthorized"));
        assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
    }

    #[test]
    fn ensure_successful_suno_upstream_status_accepts_2xx_contract() {
        let headers = rquest::header::HeaderMap::new();

        ensure_successful_suno_upstream_status(204, &headers, "", false).expect("2xx should pass");
    }

    #[test]
    fn ensure_successful_suno_upstream_status_preserves_failure_contract() {
        let headers = rquest::header::HeaderMap::new();

        let error = ensure_successful_suno_upstream_status(
            401,
            &headers,
            "{\"detail\":\"Unauthorized\"}",
            false,
        )
        .expect_err("non-2xx should fail");

        assert_eq!(error.code.as_deref(), Some("suno_session_unauthorized"));
        assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
    }

    #[test]
    fn resolve_suno_downloaded_image_mime_type_prefers_image_header_contract() {
        let mut headers = rquest::header::HeaderMap::new();
        headers.insert(
            rquest::header::CONTENT_TYPE,
            rquest::header::HeaderValue::from_static("image/webp; charset=utf-8"),
        );

        let mime_type =
            resolve_suno_downloaded_image_mime_type(&headers, "https://cdn.example.com/cover.png");

        assert_eq!(mime_type, "image/webp");
    }

    #[test]
    fn resolve_suno_downloaded_image_mime_type_falls_back_to_url_contract() {
        let headers = rquest::header::HeaderMap::new();

        let mime_type =
            resolve_suno_downloaded_image_mime_type(&headers, "https://cdn.example.com/cover.jpg");

        assert_eq!(mime_type, "image/jpeg");
    }

    #[test]
    fn materialize_suno_downloaded_image_preserves_header_mime_contract() {
        let mut headers = rquest::header::HeaderMap::new();
        headers.insert(
            rquest::header::CONTENT_TYPE,
            rquest::header::HeaderValue::from_static("image/webp"),
        );

        let (mime_type, bytes) = materialize_suno_downloaded_image(
            &headers,
            "https://cdn.example.com/cover.png",
            b"webp-bytes",
        );

        assert_eq!(mime_type, "image/webp");
        assert_eq!(bytes, b"webp-bytes".to_vec());
    }

    #[test]
    fn materialize_suno_downloaded_image_falls_back_to_url_contract() {
        let headers = rquest::header::HeaderMap::new();

        let (mime_type, bytes) = materialize_suno_downloaded_image(
            &headers,
            "https://cdn.example.com/cover.jpg",
            b"jpg-bytes",
        );

        assert_eq!(mime_type, "image/jpeg");
        assert_eq!(bytes, b"jpg-bytes".to_vec());
    }

    #[test]
    fn build_suno_downloaded_images_response_preserves_b64_contract() {
        let req = image_generation_request(json!({
            "response_format": "b64_json"
        }));

        let response = build_suno_downloaded_images_response(
            &req,
            "cover prompt",
            &[("image/webp".to_string(), b"webp-bytes".to_vec())],
        )
        .expect("downloaded image response");

        assert_eq!(response["data"][0]["mime_type"], "image/webp");
        assert_eq!(response["data"][0]["revised_prompt"], "cover prompt");
        assert!(response["data"][0]["b64_json"].as_str().is_some());
        assert!(response["data"][0].get("url").is_none());
    }

    #[test]
    fn build_suno_downloaded_images_response_limits_requested_count_contract() {
        let req = image_generation_request(json!({ "n": 1 }));

        let response = build_suno_downloaded_images_response(
            &req,
            "poster prompt",
            &[
                ("image/png".to_string(), b"first".to_vec()),
                ("image/jpeg".to_string(), b"second".to_vec()),
            ],
        )
        .expect("downloaded image response");

        assert_eq!(response["data"].as_array().map(Vec::len), Some(1));
        assert_eq!(response["data"][0]["mime_type"], "image/png");
    }

    #[test]
    fn build_suno_non_image_generation_response_preserves_music_contract() {
        let clips = vec![crate::protocol::suno::SunoClip {
            id: "clip-music-1".to_string(),
            title: Some("Dream Song".to_string()),
            image_url: None,
            lyric: None,
            audio_url: Some("https://cdn.example.com/song.mp3".to_string()),
            video_url: None,
            created_at: None,
            model_name: None,
            prompt: None,
            gpt_description_prompt: None,
            status: "complete".to_string(),
            clip_type: None,
            tags: None,
            negative_tags: None,
            duration: None,
            error_message: None,
        }];

        let body = build_suno_non_image_generation_response(
            crate::protocol::canonical::EndpointKind::MusicGenerations,
            "chirp-v3-5",
            "dream song",
            &clips,
            true,
            Some("done"),
        )
        .expect("music response");

        assert_eq!(body["object"], "music.generation");
        assert_eq!(body["model"], "chirp-v3-5");
        assert_eq!(body["prompt"], "dream song");
        assert_eq!(body["completed"], true);
        assert_eq!(body["message"], "done");
    }

    #[test]
    fn build_suno_non_image_generation_response_preserves_video_contract() {
        let clips = vec![crate::protocol::suno::SunoClip {
            id: "clip-video-1".to_string(),
            title: Some("Dream Video".to_string()),
            image_url: None,
            lyric: None,
            audio_url: None,
            video_url: Some("https://cdn.example.com/video.mp4".to_string()),
            created_at: None,
            model_name: None,
            prompt: None,
            gpt_description_prompt: None,
            status: "complete".to_string(),
            clip_type: None,
            tags: None,
            negative_tags: None,
            duration: None,
            error_message: None,
        }];

        let body = build_suno_non_image_generation_response(
            crate::protocol::canonical::EndpointKind::VideosGenerations,
            "chirp-v3-5",
            "dream video",
            &clips,
            false,
            None,
        )
        .expect("video response");

        assert_eq!(body["object"], "video.generation");
        assert_eq!(body["model"], "chirp-v3-5");
        assert_eq!(body["prompt"], "dream video");
        assert_eq!(body["completed"], false);
    }

    #[test]
    fn build_suno_non_image_generation_response_rejects_unsupported_contract() {
        let error = build_suno_non_image_generation_response(
            crate::protocol::canonical::EndpointKind::ImagesGenerations,
            "chirp-v3-5",
            "dream image",
            &[],
            false,
            None,
        )
        .expect_err("image endpoint should be unsupported here");

        assert_eq!(error.code.as_deref(), Some("unsupported_suno_endpoint"));
        assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
    }

    #[test]
    fn parse_suno_remote_browser_worker_success_reads_result_contract() {
        let parsed = parse_suno_remote_browser_worker_success(json!({
            "clips": [{
                "id": "clip-1",
                "audio_url": "https://cdn.example.com/song.mp3",
                "status": "complete"
            }],
            "completed": true,
            "message": "done"
        }))
        .expect("remote suno worker result");

        assert_eq!(parsed.clips.len(), 1);
        assert!(parsed.completed);
        assert_eq!(parsed.message.as_deref(), Some("done"));
    }

    #[test]
    fn parse_suno_remote_browser_worker_success_rejects_invalid_contract() {
        let err = parse_suno_remote_browser_worker_success(json!({
            "clips": [],
            "completed": "yes"
        }))
        .expect_err("invalid remote result should fail");
        assert_eq!(err.code.as_deref(), Some("suno_remote_result_parse_failed"));
        assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
    }

    #[test]
    fn parse_suno_remote_browser_worker_verified_result_reads_nonempty_clips_contract() {
        let parsed = parse_suno_remote_browser_worker_verified_result(json!({
            "clips": [{
                "id": "clip-1",
                "audio_url": "https://cdn.example.com/song.mp3",
                "status": "complete"
            }],
            "completed": true,
            "message": "done"
        }))
        .expect("verified remote suno worker result");

        assert_eq!(parsed.clips.len(), 1);
        assert!(parsed.completed);
        assert_eq!(parsed.message.as_deref(), Some("done"));
    }

    #[test]
    fn parse_suno_remote_browser_worker_verified_result_rejects_empty_clips_contract() {
        let err = parse_suno_remote_browser_worker_verified_result(json!({
            "clips": [],
            "completed": false,
            "message": null
        }))
        .expect_err("empty clips should fail");

        assert_eq!(err.code.as_deref(), Some("suno_missing_feed_clips"));
        assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
    }

    #[test]
    fn parse_suno_browser_worker_output_reads_result_contract() {
        let parsed = parse_suno_browser_worker_output(
            "{\"ok\":true,\"status\":200,\"result\":{\"clips\":[],\"completed\":true,\"message\":\"done\"}}",
            "",
        )
        .expect("suno worker output");

        assert!(parsed.ok);
        assert_eq!(parsed.status, Some(200));
        assert!(parsed.result.is_some());
    }

    #[test]
    fn extract_suno_browser_worker_success_reads_nonempty_clips() {
        let result: SunoBrowserWorkerResult = serde_json::from_value(json!({
            "ok": true,
            "status": 200,
            "result": {
                "clips": [{
                    "id": "clip-1",
                    "audio_url": "https://cdn.example.com/song.mp3",
                    "status": "complete"
                }],
                "completed": true,
                "message": "done"
            }
        }))
        .expect("suno worker result");

        let success =
            extract_suno_browser_worker_success(result).expect("suno worker success payload");
        assert_eq!(success.clips.len(), 1);
        assert!(success.completed);
    }

    #[test]
    fn extract_suno_browser_worker_success_rejects_empty_clips_contract() {
        let result: SunoBrowserWorkerResult = serde_json::from_value(json!({
            "ok": true,
            "status": 200,
            "result": {
                "clips": [],
                "completed": true,
                "message": "done"
            }
        }))
        .expect("suno worker result");

        let err = extract_suno_browser_worker_success(result).expect_err("empty clips should fail");
        assert_eq!(err.code.as_deref(), Some("suno_missing_feed_clips"));
        assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
    }

    #[test]
    fn classify_suno_browser_worker_failure_prefers_worker_error_contract() {
        let result: SunoBrowserWorkerResult = serde_json::from_value(json!({
            "ok": false,
            "status": 400,
            "error": {
                "code": "suno_worker_blocked",
                "message": "challenge required",
                "status": 422,
                "body": "{\"detail\":\"Unauthorized\"}"
            }
        }))
        .expect("suno worker result");

        let err = classify_suno_browser_worker_failure(result, "suno_compatible");
        assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
        assert_eq!(err.code.as_deref(), Some("suno_worker_blocked"));
        assert_eq!(err.message, "challenge required");
        assert_eq!(err.http_status, Some(422));
    }

    #[test]
    fn classify_suno_browser_worker_failure_uses_top_level_status_when_error_missing() {
        let result: SunoBrowserWorkerResult = serde_json::from_value(json!({
            "ok": false,
            "status": 429
        }))
        .expect("suno worker result");

        let err = classify_suno_browser_worker_failure(result, "suno_compatible");
        assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
        assert_eq!(err.http_status, Some(429));
    }

    #[test]
    fn resolve_suno_browser_worker_result_reads_success_contract() {
        let result: SunoBrowserWorkerResult = serde_json::from_value(json!({
            "ok": true,
            "result": {
                "clips": [{
                    "id": "clip-1",
                    "status": "complete"
                }],
                "completed": true,
                "message": "done"
            }
        }))
        .expect("suno worker result");

        let success = resolve_suno_browser_worker_result(result, "suno_compatible")
            .expect("ok worker result should succeed");

        assert_eq!(success.clips.len(), 1);
        assert_eq!(success.message.as_deref(), Some("done"));
    }

    #[test]
    fn resolve_suno_browser_worker_result_preserves_failure_contract() {
        let result: SunoBrowserWorkerResult = serde_json::from_value(json!({
            "ok": false,
            "status": 429,
            "error": {
                "status": 429,
                "code": "suno_rate_limited",
                "message": "too many requests",
                "body": "upstream body"
            }
        }))
        .expect("suno worker result");

        let err = resolve_suno_browser_worker_result(result, "suno_compatible")
            .expect_err("failed worker result should error");

        assert_eq!(err.code.as_deref(), Some("suno_rate_limited"));
        assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
        assert_eq!(err.http_status, Some(429));
    }

    #[test]
    fn parse_suno_browser_worker_verified_output_reads_success_contract() {
        let success = parse_suno_browser_worker_verified_output(
            "{\"ok\":true,\"status\":200,\"result\":{\"clips\":[{\"id\":\"clip-1\",\"status\":\"complete\"}],\"completed\":true,\"message\":\"done\"}}",
            "",
            "suno_compatible",
        )
        .expect("verified worker output");

        assert_eq!(success.clips.len(), 1);
        assert!(success.completed);
        assert_eq!(success.message.as_deref(), Some("done"));
    }

    #[test]
    fn parse_suno_browser_worker_verified_output_preserves_failure_contract() {
        let err = parse_suno_browser_worker_verified_output(
            "{\"ok\":false,\"status\":429,\"error\":{\"status\":429,\"code\":\"suno_rate_limited\",\"message\":\"too many requests\",\"body\":\"upstream body\"}}",
            "",
            "suno_compatible",
        )
        .expect_err("failed worker output should error");

        assert_eq!(err.code.as_deref(), Some("suno_rate_limited"));
        assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
        assert_eq!(err.http_status, Some(429));
    }

    #[test]
    fn build_suno_browser_executor_service_result_preserves_clips_contract() {
        let result = parse_suno_remote_browser_worker_success(json!({
            "clips": [{
                "id": "clip-1",
                "audio_url": "https://cdn.example.com/song.mp3",
                "status": "complete"
            }],
            "completed": true,
            "message": "done"
        }))
        .expect("remote suno worker result");

        let body = build_suno_browser_executor_service_result(&result);
        assert_eq!(body["clips"][0]["id"], "clip-1");
        assert_eq!(
            body["clips"][0]["audio_url"],
            "https://cdn.example.com/song.mp3"
        );
        assert_eq!(body["completed"], true);
        assert_eq!(body["message"], "done");
    }

    #[test]
    fn build_suno_browser_executor_service_result_preserves_null_message_contract() {
        let result = parse_suno_remote_browser_worker_success(json!({
            "clips": [{
                "id": "clip-1",
                "audio_url": "https://cdn.example.com/song.mp3",
                "status": "complete"
            }],
            "completed": false,
            "message": null
        }))
        .expect("remote suno worker result");

        let body = build_suno_browser_executor_service_result(&result);
        assert_eq!(body["clips"][0]["id"], "clip-1");
        assert_eq!(body["completed"], false);
        assert!(body["message"].is_null());
    }
}
