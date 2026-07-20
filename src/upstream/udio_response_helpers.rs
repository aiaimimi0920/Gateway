use crate::error::{classify_upstream_error, GatewayError};
use crate::protocol::udio::{UdioOutputKind, UdioSong};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::browser_worker_types::{UdioBrowserWorkerResult, UdioBrowserWorkerSuccess};
use rquest::header::HeaderMap;
use serde_json::json;
use std::time::Duration;

#[derive(Debug)]
pub(crate) struct PreparedUdioGenerationPlan {
    pub(crate) output_kind: UdioOutputKind,
    pub(crate) prompt: String,
    pub(crate) generate_request: serde_json::Value,
    pub(crate) wait_audio: bool,
    pub(crate) wait_timeout: Duration,
    pub(crate) poll_interval: Duration,
}

#[derive(Debug)]
pub(crate) struct PreparedUdioExecutionContext {
    pub(crate) base_url: String,
    pub(crate) runtime_state_object_key: Option<String>,
    pub(crate) headers: HeaderMap,
    pub(crate) generation_plan: PreparedUdioGenerationPlan,
    pub(crate) request_timeout: Duration,
}

pub(crate) fn classify_udio_upstream_error(
    status: u16,
    headers: &rquest::header::HeaderMap,
    body_text: &str,
) -> GatewayError {
    let provider = "udio_compatible";
    if status == 401 {
        return GatewayError::server_error(
            "Udio session is not authenticated. Capture a fresh browser session cookie before routing requests through the gateway.",
        )
        .with_provider(provider)
        .with_code("udio_session_unauthorized");
    }

    let vercel_mitigated = headers
        .get("x-vercel-mitigated")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .unwrap_or_default();
    if status == 429 && vercel_mitigated.eq_ignore_ascii_case("challenge") {
        return GatewayError::server_error(
            "Udio requires a browser security check or captcha challenge before generation can continue.",
        )
        .with_provider(provider)
        .with_code("udio_browser_challenge_required");
    }

    let body_lower = body_text.to_lowercase();
    let content_type = headers
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if status == 403 && body_lower.contains("user disallowed") {
        return GatewayError::server_error(
            "Udio rejected the current challenge token or browser clearance. Refresh the challenge in the same browser context and retry.",
        )
        .with_provider(provider)
        .with_code("udio_browser_challenge_required");
    }
    let is_vercel_security_checkpoint = body_lower.contains("vercel security checkpoint")
        || body_lower.contains("x-vercel-challenge-token")
        || body_lower.contains("x-vercel-mitigated")
        || body_lower.contains("data-astro-cid-nbv56vs3");
    if (status == 403 || status == 429)
        && (vercel_mitigated.eq_ignore_ascii_case("challenge")
            || (content_type.contains("text/html") && is_vercel_security_checkpoint))
    {
        return GatewayError::server_error(
            "Udio requires a browser security check or captcha challenge before generation can continue.",
        )
        .with_provider(provider)
        .with_code("udio_browser_challenge_required");
    }

    classify_upstream_error(status, body_text, Some(provider))
}

pub(crate) fn classify_udio_media_fetch_error(
    status: u16,
    headers: &rquest::header::HeaderMap,
    body_text: &str,
) -> GatewayError {
    classify_udio_upstream_error(status, headers, body_text)
}

pub(crate) fn ensure_successful_udio_media_fetch_status(
    status: u16,
    headers: &rquest::header::HeaderMap,
    body_text: &str,
) -> Result<(), GatewayError> {
    if (200..300).contains(&status) {
        return Ok(());
    }

    Err(classify_udio_media_fetch_error(status, headers, body_text))
}

pub(crate) fn parse_udio_browser_worker_output(
    stdout: &str,
    stderr: &str,
) -> Result<UdioBrowserWorkerResult, GatewayError> {
    if stdout.trim().is_empty() {
        return Err(crate::protocol::udio::empty_browser_worker_output_error(
            stderr,
        ));
    }

    serde_json::from_str::<UdioBrowserWorkerResult>(stdout).map_err(|error| {
        crate::protocol::udio::browser_worker_output_parse_error(error.to_string().as_str(), stdout)
    })
}

pub(crate) fn parse_udio_browser_worker_verified_output(
    stdout: &str,
    stderr: &str,
    provider: &str,
) -> Result<UdioBrowserWorkerSuccess, GatewayError> {
    let result = parse_udio_browser_worker_output(stdout, stderr)?;
    resolve_udio_browser_worker_result(result, stderr, provider)
}

pub(crate) fn parse_udio_remote_browser_worker_success(
    result: serde_json::Value,
) -> Result<UdioBrowserWorkerSuccess, GatewayError> {
    serde_json::from_value::<UdioBrowserWorkerSuccess>(result).map_err(|error| {
        crate::protocol::udio::remote_browser_worker_result_parse_error(error.to_string().as_str())
    })
}

pub(crate) fn parse_udio_remote_browser_worker_verified_result(
    result: serde_json::Value,
) -> Result<UdioBrowserWorkerSuccess, GatewayError> {
    let worker = parse_udio_remote_browser_worker_success(result)?;
    if worker.track_ids.is_empty() {
        return Err(crate::protocol::udio::missing_browser_worker_track_ids_error());
    }
    Ok(worker)
}

pub(crate) fn extract_udio_browser_worker_success(
    result: UdioBrowserWorkerResult,
) -> Result<UdioBrowserWorkerSuccess, GatewayError> {
    let payload = result
        .result
        .ok_or_else(crate::protocol::udio::missing_browser_worker_result_error)?;
    if payload.track_ids.is_empty() {
        return Err(crate::protocol::udio::missing_browser_worker_track_ids_error());
    }
    Ok(payload)
}

pub(crate) fn resolve_udio_browser_worker_result(
    result: UdioBrowserWorkerResult,
    stderr: &str,
    provider: &str,
) -> Result<UdioBrowserWorkerSuccess, GatewayError> {
    if result.ok {
        return extract_udio_browser_worker_success(result);
    }

    Err(classify_udio_browser_worker_failure(
        result, stderr, provider,
    ))
}

pub(crate) fn build_udio_browser_executor_service_result(
    result: &UdioBrowserWorkerSuccess,
) -> serde_json::Value {
    json!({
        "trackIds": result.track_ids,
        "songs": result.songs,
        "completed": result.completed,
        "message": result.message,
    })
}

pub(crate) fn resolve_udio_worker_latest_songs(
    worker_result: &UdioBrowserWorkerSuccess,
) -> Result<Vec<UdioSong>, GatewayError> {
    if worker_result.songs.is_empty() {
        return Ok(crate::protocol::udio::pending_songs(
            &worker_result.track_ids,
        ));
    }

    crate::protocol::udio::extract_songs_from_feed(&json!({
        "songs": worker_result.songs
    }))
}

pub(crate) fn prepare_udio_generation_plan(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    model: &str,
    browser_runtime_available: bool,
) -> Result<PreparedUdioGenerationPlan, GatewayError> {
    if !browser_runtime_available {
        return Err(crate::protocol::udio::missing_browser_runtime_error());
    }

    let output_kind = crate::protocol::udio::UdioOutputKind::from_endpoint_kind(req.endpoint_kind)?;
    let prompt = crate::protocol::udio::prompt_from_request(req)?;
    let generate_request = crate::protocol::udio::build_generate_request(req, model)?;
    let wait_audio = match output_kind {
        crate::protocol::udio::UdioOutputKind::Music => crate::protocol::udio::wait_audio(req),
        _ => true,
    };

    Ok(PreparedUdioGenerationPlan {
        output_kind,
        prompt,
        generate_request,
        wait_audio,
        wait_timeout: Duration::from_secs(crate::protocol::udio::wait_timeout_secs(req)),
        poll_interval: Duration::from_millis(crate::protocol::udio::poll_interval_ms(req)),
    })
}

pub(crate) fn prepare_udio_execution_context(
    payload: &ProviderAccountPayload,
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&std::collections::HashMap<String, String>>,
    default_timeout: Duration,
) -> Result<PreparedUdioExecutionContext, GatewayError> {
    let headers = crate::upstream::headers::build_upstream_headers_with(payload, extra_headers);
    let generation_plan = prepare_udio_generation_plan(
        req,
        model,
        headers.contains_key(rquest::header::COOKIE) || payload.runtime_state_object_key.is_some(),
    )?;
    let request_timeout =
        crate::upstream::browser_worker_runtime_helpers::udio_browser_request_timeout(
            default_timeout,
            generation_plan.wait_timeout,
        );

    Ok(PreparedUdioExecutionContext {
        base_url: payload.base_url.trim_end_matches('/').to_string(),
        runtime_state_object_key: payload.runtime_state_object_key.clone(),
        headers,
        generation_plan,
        request_timeout,
    })
}

pub(crate) fn build_udio_non_image_generation_response(
    output_kind: UdioOutputKind,
    model: &str,
    prompt: &str,
    songs: &[UdioSong],
    completed: bool,
    message: Option<&str>,
) -> Result<serde_json::Value, GatewayError> {
    match output_kind {
        UdioOutputKind::Music => Ok(crate::protocol::udio::build_music_generation_response(
            model, prompt, songs, completed, message,
        )),
        UdioOutputKind::Video => crate::protocol::udio::build_video_generation_response(
            model, prompt, songs, completed, message,
        ),
        UdioOutputKind::Image => unreachable!("image responses are built on a different path"),
    }
}

pub(crate) fn resolve_udio_image_generation_plan(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    prompt: &str,
    songs: &[UdioSong],
) -> Result<(Vec<String>, Option<serde_json::Value>), GatewayError> {
    let image_urls = crate::protocol::udio::collect_output_urls(songs, UdioOutputKind::Image)?
        .into_iter()
        .take(crate::protocol::udio::requested_output_count(req))
        .collect::<Vec<_>>();
    let response = if crate::protocol::udio::prefers_url_response(req)? {
        Some(
            crate::protocol::udio::build_openai_images_response_from_urls(
                req,
                prompt,
                &image_urls,
            )?,
        )
    } else {
        None
    };
    Ok((image_urls, response))
}

pub(crate) fn resolve_udio_downloaded_image_mime_type(
    headers: &rquest::header::HeaderMap,
) -> String {
    headers
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .filter(|value| value.starts_with("image/"))
        .map(ToString::to_string)
        .unwrap_or_else(|| "image/jpeg".to_string())
}

pub(crate) fn materialize_udio_downloaded_image(
    headers: &rquest::header::HeaderMap,
    bytes: &[u8],
) -> (String, Vec<u8>) {
    (
        resolve_udio_downloaded_image_mime_type(headers),
        bytes.to_vec(),
    )
}

pub(crate) fn build_udio_downloaded_images_response(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    prompt: &str,
    images: &[(String, Vec<u8>)],
) -> serde_json::Value {
    crate::protocol::udio::build_openai_images_response_from_bytes(req, prompt, images)
}

pub(crate) fn classify_udio_browser_worker_failure(
    result: UdioBrowserWorkerResult,
    stderr: &str,
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
        .or_else(|| stderr.is_empty().then_some("").or(Some(stderr)))
        .unwrap_or_default();
    let mut gateway_error = classify_upstream_error(status, body_text, Some(provider));
    if let Some(worker_error) = result.error {
        if let Some(code) = worker_error.code {
            gateway_error.code = Some(code);
        }
        if let Some(message) = worker_error.message {
            gateway_error.message = message;
        }
        if gateway_error.http_status.is_none() {
            gateway_error.http_status = Some(status);
        }
    }
    gateway_error
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::candidate::ProviderAccountPayload;

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

    fn music_generation_request(
        raw_body: serde_json::Value,
    ) -> crate::protocol::canonical::CanonicalRelayRequest {
        crate::protocol::canonical::CanonicalRelayRequest {
            protocol_family: crate::protocol::canonical::ProtocolFamily::OpenAi,
            endpoint_kind: crate::protocol::canonical::EndpointKind::MusicGenerations,
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

    fn make_payload(base_url: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
            adapter: "udio_compatible".to_string(),
            base_url: base_url.to_string(),
            api_key: "sk-test".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: None,
            headers: std::collections::HashMap::new(),
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

    #[test]
    fn classify_udio_html_checkpoint_as_browser_challenge() {
        let mut headers = rquest::header::HeaderMap::new();
        headers.insert(
            rquest::header::CONTENT_TYPE,
            rquest::header::HeaderValue::from_static("text/html; charset=utf-8"),
        );
        let err = classify_udio_upstream_error(
            403,
            &headers,
            "<!DOCTYPE html><title>Vercel Security Checkpoint</title>",
        );
        assert_eq!(err.code.as_deref(), Some("udio_browser_challenge_required"));
        assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
    }

    #[test]
    fn classify_udio_user_disallowed_as_browser_challenge() {
        let headers = rquest::header::HeaderMap::new();
        let err = classify_udio_upstream_error(403, &headers, r#"{"error":"User disallowed"}"#);
        assert_eq!(err.code.as_deref(), Some("udio_browser_challenge_required"));
        assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
    }

    #[test]
    fn classify_udio_401_as_session_unauthorized() {
        let headers = rquest::header::HeaderMap::new();
        let err = classify_udio_upstream_error(401, &headers, r#"{"detail":"Unauthorized"}"#);
        assert_eq!(err.code.as_deref(), Some("udio_session_unauthorized"));
        assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
    }

    #[test]
    fn classify_udio_media_fetch_error_preserves_challenge_contract() {
        let mut headers = rquest::header::HeaderMap::new();
        headers.insert(
            rquest::header::HeaderName::from_static("x-vercel-mitigated"),
            rquest::header::HeaderValue::from_static("challenge"),
        );
        let err = classify_udio_media_fetch_error(429, &headers, "rate limited");
        assert_eq!(err.code.as_deref(), Some("udio_browser_challenge_required"));
        assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
    }

    #[test]
    fn classify_udio_media_fetch_error_preserves_unauthorized_contract() {
        let headers = rquest::header::HeaderMap::new();
        let err = classify_udio_media_fetch_error(401, &headers, r#"{"detail":"Unauthorized"}"#);
        assert_eq!(err.code.as_deref(), Some("udio_session_unauthorized"));
        assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
    }

    #[test]
    fn parse_udio_remote_browser_worker_success_reads_result_contract() {
        let parsed = parse_udio_remote_browser_worker_success(serde_json::json!({
            "trackIds": ["track-1"],
            "songs": [],
            "completed": true
        }))
        .expect("udio remote result");

        assert_eq!(parsed.track_ids, vec!["track-1"]);
        assert!(parsed.completed);
    }

    #[test]
    fn parse_udio_remote_browser_worker_success_rejects_invalid_contract() {
        let error = parse_udio_remote_browser_worker_success(serde_json::json!({
            "trackIds": [],
            "completed": "yes"
        }))
        .expect_err("invalid remote result should fail");

        assert_eq!(
            error.code.as_deref(),
            Some("udio_remote_result_parse_failed")
        );
        assert_eq!(error.provider_name.as_deref(), Some("udio_compatible"));
    }

    #[test]
    fn parse_udio_remote_browser_worker_verified_result_reads_track_ids_contract() {
        let worker = parse_udio_remote_browser_worker_verified_result(serde_json::json!({
            "trackIds": ["track-1"],
            "songs": [],
            "completed": true,
            "message": "done"
        }))
        .expect("verified remote result");

        assert_eq!(worker.track_ids, vec!["track-1"]);
        assert!(worker.completed);
        assert_eq!(worker.message.as_deref(), Some("done"));
    }

    #[test]
    fn parse_udio_remote_browser_worker_verified_result_rejects_empty_track_ids_contract() {
        let error = parse_udio_remote_browser_worker_verified_result(serde_json::json!({
            "trackIds": [],
            "songs": [],
            "completed": false
        }))
        .expect_err("empty track ids should fail");

        assert_eq!(error.code.as_deref(), Some("udio_missing_track_ids"));
        assert_eq!(error.provider_name.as_deref(), Some("udio_compatible"));
    }

    #[test]
    fn prepare_udio_generation_plan_reads_music_contract() {
        let req = music_generation_request(serde_json::json!({
            "prompt": "cinematic synthwave anthem",
            "wait_audio": false,
            "wait_timeout_secs": 12,
            "poll_interval_ms": 750
        }));

        let plan =
            prepare_udio_generation_plan(&req, crate::protocol::udio::UDIO_DEFAULT_MODEL, true)
                .expect("generation plan");

        assert_eq!(
            plan.output_kind,
            crate::protocol::udio::UdioOutputKind::Music
        );
        assert_eq!(plan.prompt, "cinematic synthwave anthem");
        assert_eq!(plan.wait_audio, false);
        assert_eq!(plan.wait_timeout, Duration::from_secs(15));
        assert_eq!(plan.poll_interval, Duration::from_millis(1000));
        assert_eq!(
            plan.generate_request["gen_params"]["prompt"],
            "cinematic synthwave anthem"
        );
    }

    #[test]
    fn prepare_udio_generation_plan_preserves_missing_browser_runtime_contract() {
        let req = image_generation_request(serde_json::json!({
            "prompt": "cover art"
        }));

        let error =
            prepare_udio_generation_plan(&req, crate::protocol::udio::UDIO_DEFAULT_MODEL, false)
                .expect_err("missing runtime should fail");

        assert_eq!(error.code.as_deref(), Some("missing_udio_browser_runtime"));
        assert_eq!(error.provider_name.as_deref(), Some("udio_compatible"));
    }

    #[test]
    fn prepare_udio_execution_context_reads_cookie_runtime_contract() {
        let mut payload = make_payload("https://www.udio.com/");
        payload
            .headers
            .insert("cookie".to_string(), "a=b".to_string());
        let req = music_generation_request(serde_json::json!({
            "prompt": "cinematic synthwave anthem",
            "wait_audio": false,
            "wait_timeout_secs": 480,
            "poll_secs": 2
        }));

        let prepared = prepare_udio_execution_context(
            &payload,
            &req,
            crate::protocol::udio::UDIO_DEFAULT_MODEL,
            None,
            Duration::from_secs(60),
        )
        .expect("udio execution context");

        assert_eq!(prepared.base_url, "https://www.udio.com");
        assert_eq!(prepared.runtime_state_object_key, None);
        assert_eq!(
            prepared
                .headers
                .get("cookie")
                .and_then(|value| value.to_str().ok()),
            Some("a=b")
        );
        assert_eq!(prepared.request_timeout, Duration::from_secs(540));
        assert_eq!(
            prepared.generation_plan.output_kind,
            crate::protocol::udio::UdioOutputKind::Music
        );
        assert_eq!(
            prepared.generation_plan.prompt,
            "cinematic synthwave anthem"
        );
        assert!(!prepared.generation_plan.wait_audio);
    }

    #[test]
    fn prepare_udio_execution_context_reads_runtime_state_fallback_contract() {
        let mut payload = make_payload("https://www.udio.com/");
        payload.runtime_state_object_key = Some("runtime-123".to_string());
        let req = image_generation_request(serde_json::json!({
            "prompt": "cover art"
        }));

        let prepared = prepare_udio_execution_context(
            &payload,
            &req,
            crate::protocol::udio::UDIO_DEFAULT_MODEL,
            None,
            Duration::from_secs(45),
        )
        .expect("udio execution context");

        assert_eq!(prepared.base_url, "https://www.udio.com");
        assert_eq!(
            prepared.runtime_state_object_key.as_deref(),
            Some("runtime-123")
        );
        assert_eq!(prepared.request_timeout, Duration::from_secs(300));
        assert_eq!(
            prepared.generation_plan.output_kind,
            crate::protocol::udio::UdioOutputKind::Image
        );
        assert_eq!(prepared.generation_plan.prompt, "cover art");
    }

    #[test]
    fn parse_udio_browser_worker_output_reads_result_contract() {
        let parsed = parse_udio_browser_worker_output(
            "{\"ok\":true,\"status\":200,\"result\":{\"trackIds\":[\"track-1\"],\"songs\":[],\"completed\":true}}",
            "",
        )
        .expect("udio worker output");

        assert!(parsed.ok);
        assert_eq!(parsed.status, Some(200));
        let result = parsed.result.expect("worker result");
        assert_eq!(result.track_ids, vec!["track-1"]);
        assert!(result.completed);
    }

    #[test]
    fn parse_udio_browser_worker_output_rejects_empty_stdout_contract() {
        let error = parse_udio_browser_worker_output("", "permission denied")
            .expect_err("empty stdout should fail");
        assert_eq!(
            error.code.as_deref(),
            Some("udio_browser_worker_empty_output")
        );
        assert_eq!(error.provider_name.as_deref(), Some("udio_compatible"));
    }

    #[test]
    fn extract_udio_browser_worker_success_reads_track_ids() {
        let result = parse_udio_browser_worker_output(
            "{\"ok\":true,\"status\":200,\"result\":{\"trackIds\":[\"track-1\"],\"songs\":[],\"completed\":true}}",
            "",
        )
        .expect("udio worker output");

        let payload = extract_udio_browser_worker_success(result).expect("udio worker success");
        assert_eq!(payload.track_ids, vec!["track-1"]);
        assert!(payload.completed);
    }

    #[test]
    fn extract_udio_browser_worker_success_rejects_empty_track_ids_contract() {
        let result = parse_udio_browser_worker_output(
            "{\"ok\":true,\"status\":200,\"result\":{\"trackIds\":[],\"songs\":[],\"completed\":true}}",
            "",
        )
        .expect("udio worker output");

        let error =
            extract_udio_browser_worker_success(result).expect_err("missing track ids should fail");
        assert_eq!(error.code.as_deref(), Some("udio_missing_track_ids"));
        assert_eq!(error.provider_name.as_deref(), Some("udio_compatible"));
    }

    #[test]
    fn classify_udio_browser_worker_failure_prefers_worker_error_contract() {
        let result = parse_udio_browser_worker_output(
            "{\"ok\":false,\"status\":400,\"error\":{\"code\":\"udio_worker_blocked\",\"message\":\"challenge required\",\"status\":422,\"body\":\"{\\\"detail\\\":\\\"Unauthorized\\\"}\"}}",
            "",
        )
        .expect("udio worker output");

        let error = classify_udio_browser_worker_failure(result, "", "udio_compatible");
        assert_eq!(error.provider_name.as_deref(), Some("udio_compatible"));
        assert_eq!(error.code.as_deref(), Some("udio_worker_blocked"));
        assert_eq!(error.message, "challenge required");
        assert_eq!(error.http_status, Some(422));
    }

    #[test]
    fn classify_udio_browser_worker_failure_uses_stderr_when_body_missing() {
        let result = parse_udio_browser_worker_output("{\"ok\":false,\"status\":429}", "")
            .expect("udio worker output");

        let error =
            classify_udio_browser_worker_failure(result, "permission denied", "udio_compatible");
        assert_eq!(error.provider_name.as_deref(), Some("udio_compatible"));
        assert_eq!(error.http_status, Some(429));
        assert!(error.message.contains("permission denied"));
    }

    #[test]
    fn resolve_udio_browser_worker_result_reads_success_contract() {
        let result = parse_udio_browser_worker_output(
            "{\"ok\":true,\"status\":200,\"result\":{\"trackIds\":[\"track-1\"],\"songs\":[],\"completed\":true,\"message\":\"done\"}}",
            "",
        )
        .expect("udio worker output");

        let payload = resolve_udio_browser_worker_result(result, "", "udio_compatible")
            .expect("ok worker result should succeed");

        assert_eq!(payload.track_ids, vec!["track-1"]);
        assert!(payload.completed);
        assert_eq!(payload.message.as_deref(), Some("done"));
    }

    #[test]
    fn resolve_udio_browser_worker_result_preserves_failure_contract() {
        let result = parse_udio_browser_worker_output(
            "{\"ok\":false,\"status\":429,\"error\":{\"code\":\"udio_rate_limited\",\"message\":\"too many requests\",\"status\":429}}",
            "",
        )
        .expect("udio worker output");

        let error = resolve_udio_browser_worker_result(result, "", "udio_compatible")
            .expect_err("failed worker result should error");

        assert_eq!(error.provider_name.as_deref(), Some("udio_compatible"));
        assert_eq!(error.code.as_deref(), Some("udio_rate_limited"));
        assert_eq!(error.message, "too many requests");
        assert_eq!(error.http_status, Some(429));
    }

    #[test]
    fn parse_udio_browser_worker_verified_output_reads_success_contract() {
        let payload = parse_udio_browser_worker_verified_output(
            "{\"ok\":true,\"status\":200,\"result\":{\"trackIds\":[\"track-1\"],\"songs\":[],\"completed\":true,\"message\":\"done\"}}",
            "",
            "udio_compatible",
        )
        .expect("verified worker output");

        assert_eq!(payload.track_ids, vec!["track-1"]);
        assert!(payload.completed);
        assert_eq!(payload.message.as_deref(), Some("done"));
    }

    #[test]
    fn parse_udio_browser_worker_verified_output_preserves_failure_contract() {
        let error = parse_udio_browser_worker_verified_output(
            "{\"ok\":false,\"status\":429,\"error\":{\"code\":\"udio_rate_limited\",\"message\":\"too many requests\",\"status\":429}}",
            "",
            "udio_compatible",
        )
        .expect_err("failed worker output should error");

        assert_eq!(error.provider_name.as_deref(), Some("udio_compatible"));
        assert_eq!(error.code.as_deref(), Some("udio_rate_limited"));
        assert_eq!(error.message, "too many requests");
        assert_eq!(error.http_status, Some(429));
    }

    #[test]
    fn build_udio_browser_executor_service_result_preserves_track_and_song_contract() {
        let result = parse_udio_remote_browser_worker_success(serde_json::json!({
            "trackIds": ["track-1"],
            "songs": [{
                "audio_url": "https://cdn.example.com/song.mp3"
            }],
            "completed": true,
            "message": "done"
        }))
        .expect("udio remote result");

        let body = build_udio_browser_executor_service_result(&result);
        assert_eq!(body["trackIds"][0], "track-1");
        assert_eq!(
            body["songs"][0]["audio_url"],
            "https://cdn.example.com/song.mp3"
        );
        assert_eq!(body["completed"], true);
        assert_eq!(body["message"], "done");
    }

    #[test]
    fn build_udio_browser_executor_service_result_preserves_null_message_contract() {
        let result = parse_udio_remote_browser_worker_success(serde_json::json!({
            "trackIds": ["track-1"],
            "songs": [],
            "completed": false,
            "message": null
        }))
        .expect("udio remote result");

        let body = build_udio_browser_executor_service_result(&result);
        assert_eq!(body["trackIds"][0], "track-1");
        assert_eq!(body["completed"], false);
        assert!(body["message"].is_null());
    }

    #[test]
    fn resolve_udio_worker_latest_songs_falls_back_to_pending_track_ids_contract() {
        let worker = parse_udio_remote_browser_worker_success(serde_json::json!({
            "trackIds": ["track-1", "track-2"],
            "songs": [],
            "completed": false
        }))
        .expect("udio remote result");

        let songs = resolve_udio_worker_latest_songs(&worker).expect("pending songs");
        assert_eq!(songs.len(), 2);
        assert_eq!(songs[0].id, "track-1");
        assert_eq!(songs[0].status, "pending");
        assert!(!songs[0].finished);
    }

    #[test]
    fn resolve_udio_worker_latest_songs_uses_worker_song_feed_contract() {
        let worker = parse_udio_remote_browser_worker_success(serde_json::json!({
            "trackIds": ["track-1"],
            "songs": [{
                "id": "track-1",
                "image_url": "https://cdn.example.com/image.png",
                "song_path": "https://cdn.example.com/song.mp3",
                "finished": true,
                "readyToStream": true
            }],
            "completed": true
        }))
        .expect("udio remote result");

        let songs = resolve_udio_worker_latest_songs(&worker).expect("worker songs");
        assert_eq!(songs.len(), 1);
        assert_eq!(songs[0].id, "track-1");
        assert_eq!(
            songs[0].audio_url.as_deref(),
            Some("https://cdn.example.com/song.mp3")
        );
        assert!(songs[0].finished);
    }

    #[test]
    fn build_udio_non_image_generation_response_preserves_music_contract() {
        let response = build_udio_non_image_generation_response(
            crate::protocol::udio::UdioOutputKind::Music,
            "udio-music-model",
            "dreamy synthpop",
            &[crate::protocol::udio::UdioSong {
                id: "track-1".to_string(),
                title: Some("Dream Waves".to_string()),
                image_url: None,
                audio_url: Some("https://cdn.example.com/song.mp3".to_string()),
                video_url: None,
                created_at: None,
                duration_seconds: Some(42.0),
                prompt: None,
                lyrics: None,
                lyric_input: None,
                finished: true,
                ready_to_stream: true,
                estimated_duration_seconds: None,
                status: "finished".to_string(),
                error_message: None,
            }],
            true,
            Some("done"),
        )
        .expect("music response");

        assert_eq!(response["object"], "music.generation");
        assert_eq!(response["model"], "udio-music-model");
        assert_eq!(response["prompt"], "dreamy synthpop");
        assert_eq!(response["completed"], true);
        assert_eq!(response["message"], "done");
        assert_eq!(response["data"][0]["id"], "track-1");
    }

    #[test]
    fn build_udio_non_image_generation_response_preserves_video_contract() {
        let response = build_udio_non_image_generation_response(
            crate::protocol::udio::UdioOutputKind::Video,
            "udio-video-model",
            "cinematic trailer",
            &[crate::protocol::udio::UdioSong {
                id: "track-2".to_string(),
                title: Some("Trailer Cut".to_string()),
                image_url: None,
                audio_url: None,
                video_url: Some("https://cdn.example.com/video.mp4".to_string()),
                created_at: None,
                duration_seconds: Some(30.0),
                prompt: None,
                lyrics: None,
                lyric_input: None,
                finished: true,
                ready_to_stream: true,
                estimated_duration_seconds: None,
                status: "finished".to_string(),
                error_message: None,
            }],
            false,
            None,
        )
        .expect("video response");

        assert_eq!(response["object"], "video.generation");
        assert_eq!(response["model"], "udio-video-model");
        assert_eq!(response["prompt"], "cinematic trailer");
        assert_eq!(response["completed"], false);
        assert_eq!(
            response["data"][0]["url"],
            "https://cdn.example.com/video.mp4"
        );
    }

    #[test]
    fn resolve_udio_image_generation_plan_preserves_url_response_contract() {
        let req = image_generation_request(serde_json::json!({}));
        let songs = vec![crate::protocol::udio::UdioSong {
            id: "track-3".to_string(),
            title: Some("Cover Art".to_string()),
            image_url: Some("https://cdn.example.com/cover.jpg".to_string()),
            audio_url: None,
            video_url: None,
            created_at: None,
            duration_seconds: None,
            prompt: None,
            lyrics: None,
            lyric_input: None,
            finished: true,
            ready_to_stream: true,
            estimated_duration_seconds: None,
            status: "finished".to_string(),
            error_message: None,
        }];

        let (image_urls, response) =
            resolve_udio_image_generation_plan(&req, "album art", &songs).expect("image plan");

        assert_eq!(
            image_urls,
            vec!["https://cdn.example.com/cover.jpg".to_string()]
        );
        let response = response.expect("url response");
        assert_eq!(
            response["data"][0]["url"],
            "https://cdn.example.com/cover.jpg"
        );
        assert_eq!(response["data"][0]["revised_prompt"], "album art");
    }

    #[test]
    fn resolve_udio_image_generation_plan_preserves_b64_contract() {
        let req = image_generation_request(serde_json::json!({ "response_format": "b64_json" }));
        let songs = vec![crate::protocol::udio::UdioSong {
            id: "track-4".to_string(),
            title: Some("Poster".to_string()),
            image_url: Some("https://cdn.example.com/poster.jpg".to_string()),
            audio_url: None,
            video_url: None,
            created_at: None,
            duration_seconds: None,
            prompt: None,
            lyrics: None,
            lyric_input: None,
            finished: true,
            ready_to_stream: true,
            estimated_duration_seconds: None,
            status: "finished".to_string(),
            error_message: None,
        }];

        let (image_urls, response) =
            resolve_udio_image_generation_plan(&req, "poster art", &songs).expect("image plan");

        assert_eq!(
            image_urls,
            vec!["https://cdn.example.com/poster.jpg".to_string()]
        );
        assert!(response.is_none());
    }

    #[test]
    fn resolve_udio_image_generation_plan_limits_download_urls_to_requested_count_contract() {
        let req = image_generation_request(serde_json::json!({ "n": 1 }));
        let songs = vec![
            crate::protocol::udio::UdioSong {
                id: "track-5".to_string(),
                title: Some("Poster One".to_string()),
                image_url: Some("https://cdn.example.com/poster-1.jpg".to_string()),
                audio_url: None,
                video_url: None,
                created_at: None,
                duration_seconds: None,
                prompt: None,
                lyrics: None,
                lyric_input: None,
                finished: true,
                ready_to_stream: true,
                estimated_duration_seconds: None,
                status: "finished".to_string(),
                error_message: None,
            },
            crate::protocol::udio::UdioSong {
                id: "track-6".to_string(),
                title: Some("Poster Two".to_string()),
                image_url: Some("https://cdn.example.com/poster-2.jpg".to_string()),
                audio_url: None,
                video_url: None,
                created_at: None,
                duration_seconds: None,
                prompt: None,
                lyrics: None,
                lyric_input: None,
                finished: true,
                ready_to_stream: true,
                estimated_duration_seconds: None,
                status: "finished".to_string(),
                error_message: None,
            },
        ];

        let (image_urls, response) =
            resolve_udio_image_generation_plan(&req, "poster art", &songs).expect("image plan");

        assert_eq!(
            image_urls,
            vec!["https://cdn.example.com/poster-1.jpg".to_string()]
        );
        let response = response.expect("url response");
        assert_eq!(response["data"].as_array().map(Vec::len), Some(1));
    }

    #[test]
    fn resolve_udio_downloaded_image_mime_type_preserves_image_header_contract() {
        let mut headers = rquest::header::HeaderMap::new();
        headers.insert(
            rquest::header::CONTENT_TYPE,
            rquest::header::HeaderValue::from_static("image/png; charset=utf-8"),
        );

        let mime_type = resolve_udio_downloaded_image_mime_type(&headers);

        assert_eq!(mime_type, "image/png");
    }

    #[test]
    fn resolve_udio_downloaded_image_mime_type_defaults_to_jpeg_contract() {
        let mut headers = rquest::header::HeaderMap::new();
        headers.insert(
            rquest::header::CONTENT_TYPE,
            rquest::header::HeaderValue::from_static("application/octet-stream"),
        );

        let mime_type = resolve_udio_downloaded_image_mime_type(&headers);

        assert_eq!(mime_type, "image/jpeg");
    }

    #[test]
    fn materialize_udio_downloaded_image_preserves_header_mime_contract() {
        let mut headers = rquest::header::HeaderMap::new();
        headers.insert(
            rquest::header::CONTENT_TYPE,
            rquest::header::HeaderValue::from_static("image/png"),
        );

        let (mime_type, bytes) = materialize_udio_downloaded_image(&headers, b"png-bytes");

        assert_eq!(mime_type, "image/png");
        assert_eq!(bytes, b"png-bytes".to_vec());
    }

    #[test]
    fn materialize_udio_downloaded_image_defaults_to_jpeg_contract() {
        let headers = rquest::header::HeaderMap::new();

        let (mime_type, bytes) = materialize_udio_downloaded_image(&headers, b"jpg-bytes");

        assert_eq!(mime_type, "image/jpeg");
        assert_eq!(bytes, b"jpg-bytes".to_vec());
    }

    #[test]
    fn build_udio_downloaded_images_response_preserves_b64_contract() {
        let req = image_generation_request(serde_json::json!({
            "response_format": "b64_json"
        }));

        let response = build_udio_downloaded_images_response(
            &req,
            "poster art",
            &[("image/png".to_string(), b"png-bytes".to_vec())],
        );

        assert_eq!(response["data"][0]["mime_type"], "image/png");
        assert_eq!(response["data"][0]["revised_prompt"], "poster art");
        assert!(response["data"][0]["b64_json"].as_str().is_some());
        assert!(response["data"][0].get("url").is_none());
    }

    #[test]
    fn build_udio_downloaded_images_response_limits_requested_count_contract() {
        let req = image_generation_request(serde_json::json!({ "n": 1 }));

        let response = build_udio_downloaded_images_response(
            &req,
            "cover art",
            &[
                ("image/png".to_string(), b"first".to_vec()),
                ("image/jpeg".to_string(), b"second".to_vec()),
            ],
        );

        assert_eq!(response["data"].as_array().map(Vec::len), Some(1));
        assert_eq!(response["data"][0]["mime_type"], "image/png");
    }

    #[test]
    fn ensure_successful_udio_media_fetch_status_accepts_2xx_contract() {
        let headers = rquest::header::HeaderMap::new();

        ensure_successful_udio_media_fetch_status(204, &headers, "").expect("2xx should pass");
    }

    #[test]
    fn ensure_successful_udio_media_fetch_status_preserves_failure_contract() {
        let headers = rquest::header::HeaderMap::new();

        let error = ensure_successful_udio_media_fetch_status(
            401,
            &headers,
            "{\"detail\":\"Unauthorized\"}",
        )
        .expect_err("non-2xx should fail");

        assert_eq!(error.code.as_deref(), Some("udio_session_unauthorized"));
        assert_eq!(error.provider_name.as_deref(), Some("udio_compatible"));
    }
}
