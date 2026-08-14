use rquest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::Value;

use crate::error::{classify_upstream_error, FallbackHint, GatewayError};
use crate::protocol::canonical::EndpointKind;
use crate::upstream::browser_worker_runtime_helpers::{
    gemini_canvas_browser_pool_script_path, lumalabs_browser_worker_script_path,
    producer_browser_worker_script_path, suno_browser_worker_script_path,
    udio_browser_worker_script_path,
};
use crate::upstream::browser_worker_types::{
    BrowserExecutorInvocationError, BrowserExecutorInvocationResponse,
    BrowserExecutorServiceHealth, BrowserExecutorServiceInvocationResponse,
};

fn read_json_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)?
        .as_str()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string)
}

pub(crate) fn read_json_u64(value: &Value, key: &str) -> Option<u64> {
    value.get(key)?.as_u64()
}

pub(crate) fn read_json_bool(value: &Value, key: &str) -> Option<bool> {
    value.get(key)?.as_bool()
}

pub(crate) fn missing_browser_executor_field_error(
    provider: &str,
    field: &str,
    code: &str,
) -> GatewayError {
    GatewayError::bad_request(format!("{provider} browser executor requires {field}."))
        .with_code(code)
}

pub(crate) fn unsupported_browser_executor_provider_error(provider: &str) -> GatewayError {
    GatewayError::bad_request(format!(
        "Unsupported browser executor provider '{provider}'."
    ))
    .with_code("browser_executor_unsupported_provider")
}

pub(crate) fn browser_execution_status_from_message(message: &str) -> String {
    let normalized = message.trim().to_ascii_lowercase();
    if normalized.contains("challenge") || normalized.contains("cloudflare") {
        return "challenge".to_string();
    }
    if normalized.contains("timeout") {
        return "timed_out".to_string();
    }
    if normalized.contains("crash")
        || normalized.contains("browser closed")
        || normalized.contains("target closed")
        || normalized.contains("execution context destroyed")
    {
        return "crashed".to_string();
    }
    "released".to_string()
}

pub(crate) fn build_browser_executor_header_map(input: &Value) -> HeaderMap {
    let mut headers = HeaderMap::new();

    if let Some(cookie_header) = read_json_string(input, "cookieHeader") {
        if let Ok(value) = HeaderValue::from_str(&cookie_header) {
            headers.insert(rquest::header::COOKIE, value);
        }
    }
    if let Some(auth_token) = read_json_string(input, "authToken") {
        let bearer = format!("Bearer {auth_token}");
        if let Ok(value) = HeaderValue::from_str(&bearer) {
            headers.insert(rquest::header::AUTHORIZATION, value);
        }
    }

    for (json_key, header_name) in [
        ("userAgent", "user-agent"),
        ("acceptLanguage", "accept-language"),
        ("origin", "origin"),
        ("referer", "referer"),
        ("deviceId", "device-id"),
        ("browserToken", "browser-token"),
        ("referringPathname", "referring-pathname"),
        ("referringOrigin", "referring-origin"),
    ] {
        if let Some(value) = read_json_string(input, json_key) {
            if let (Ok(name), Ok(header_value)) = (
                HeaderName::from_bytes(header_name.as_bytes()),
                HeaderValue::from_str(&value),
            ) {
                headers.insert(name, header_value);
            }
        }
    }

    headers
}

pub(crate) fn browser_executor_endpoint_kind_key(endpoint_kind: EndpointKind) -> &'static str {
    match endpoint_kind {
        EndpointKind::ChatCompletions => "chat_completions",
        EndpointKind::Completions => "completions",
        EndpointKind::Embeddings => "embeddings",
        EndpointKind::Messages => "messages",
        EndpointKind::Responses => "responses",
        EndpointKind::Search => "search",
        EndpointKind::Fetch => "fetch",
        EndpointKind::ImagesGenerations => "images_generations",
        EndpointKind::ImagesEdits => "images_edits",
        EndpointKind::MusicGenerations => "music_generations",
        EndpointKind::VideosGenerations => "videos_generations",
        EndpointKind::AudioTranscriptions => "audio_transcriptions",
        EndpointKind::AudioSpeech => "audio_speech",
        EndpointKind::ResearchCreate => "research_create",
        EndpointKind::ResearchList => "research_list",
        EndpointKind::ResearchGet => "research_get",
        EndpointKind::CreditsBalance => "credits_balance",
    }
}

pub(crate) fn build_browser_executor_runtime_health(
    remote_base_url: Option<String>,
) -> BrowserExecutorServiceHealth {
    BrowserExecutorServiceHealth {
        ok: true,
        enabled: true,
        mode: if remote_base_url.is_some() {
            "local+remote-override".to_string()
        } else {
            "local".to_string()
        },
        remote_base_url,
        lumalabs_script_path: lumalabs_browser_worker_script_path().display().to_string(),
        producer_script_path: producer_browser_worker_script_path().display().to_string(),
        suno_script_path: suno_browser_worker_script_path().display().to_string(),
        udio_script_path: udio_browser_worker_script_path().display().to_string(),
        gemini_canvas_pool_script_path: gemini_canvas_browser_pool_script_path()
            .display()
            .to_string(),
    }
}

pub(crate) fn parse_browser_executor_invocation_response_body(
    body_text: &str,
) -> Result<BrowserExecutorInvocationResponse, serde_json::Error> {
    serde_json::from_str(body_text)
}

pub(crate) fn extract_browser_executor_invocation_success(
    result: BrowserExecutorInvocationResponse,
) -> Option<Value> {
    result.result
}

pub(crate) fn classify_browser_executor_invocation_failure(
    result: BrowserExecutorInvocationResponse,
    provider: &str,
) -> GatewayError {
    let error = result.error;
    let upstream_status = error
        .as_ref()
        .and_then(|entry| entry.status)
        .or(result.status)
        .unwrap_or(500);
    let body = error
        .as_ref()
        .and_then(|entry| entry.body.as_deref())
        .unwrap_or("");
    let challenge_required = error.as_ref().is_some_and(|entry| {
        browser_executor_failure_requires_interactive_challenge(
            entry.code.as_deref(),
            entry.message.as_deref(),
            entry.body.as_deref(),
        )
    });
    let mut gateway_error = classify_upstream_error(upstream_status, body, Some(provider));
    if let Some(error) = error {
        if let Some(code) = error.code {
            gateway_error.code = Some(code);
        }
        if let Some(message) = error.message {
            gateway_error.message = message;
        }
        if gateway_error.http_status.is_none() {
            gateway_error.http_status = Some(upstream_status);
        }
    }
    if challenge_required {
        gateway_error.retryable = false;
        gateway_error.fallback_hint = FallbackHint::Abort {
            reason: "Interactive browser security checks require manual completion and must not be replayed automatically.".to_string(),
        };
    }
    gateway_error
}

fn browser_executor_failure_requires_interactive_challenge(
    code: Option<&str>,
    message: Option<&str>,
    body: Option<&str>,
) -> bool {
    let code = code.unwrap_or_default().to_ascii_lowercase();
    let details = format!(
        "{} {}",
        message.unwrap_or_default(),
        body.unwrap_or_default(),
    )
    .to_ascii_lowercase();
    code.contains("challenge_required")
        || code.contains("captcha_required")
        || details.contains("security check")
        || details.contains("captcha")
        || details.contains("turnstile")
        || details.contains("cloudflare challenge")
}

pub(crate) fn build_browser_executor_service_invocation_failure_response(
    provider: &str,
    error: GatewayError,
) -> BrowserExecutorServiceInvocationResponse {
    let message = error.message.clone();
    let code = error.code.clone();
    let status = error.http_status.map(|value| value as u16);

    BrowserExecutorServiceInvocationResponse {
        ok: false,
        provider: provider.to_string(),
        status,
        result: None,
        error: Some(BrowserExecutorInvocationError {
            code,
            message: Some(message.clone()),
            status,
            body: Some(message.clone()),
        }),
        lease: None,
        browser_execution_status: browser_execution_status_from_message(&message),
    }
}

pub(crate) fn build_browser_executor_service_invocation_success_response(
    provider: &str,
    result: Value,
) -> BrowserExecutorServiceInvocationResponse {
    BrowserExecutorServiceInvocationResponse {
        ok: true,
        provider: provider.to_string(),
        status: Some(200),
        result: Some(result),
        error: None,
        lease: None,
        browser_execution_status: "completed".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::upstream::header_map_helpers::header_map_string;
    use serde_json::json;

    fn assert_status(message: &str, expected: &str) {
        assert_eq!(browser_execution_status_from_message(message), expected);
    }

    #[test]
    fn build_browser_executor_runtime_health_reports_local_mode_without_override() {
        let health = build_browser_executor_runtime_health(None);

        assert!(health.ok);
        assert!(health.enabled);
        assert_eq!(health.mode, "local");
        assert_eq!(health.remote_base_url, None);
        assert_eq!(
            health.lumalabs_script_path,
            lumalabs_browser_worker_script_path().display().to_string()
        );
        assert_eq!(
            health.gemini_canvas_pool_script_path,
            gemini_canvas_browser_pool_script_path()
                .display()
                .to_string()
        );
    }

    #[test]
    fn build_browser_executor_runtime_health_reports_remote_override_mode() {
        let health =
            build_browser_executor_runtime_health(Some("http://127.0.0.1:4227".to_string()));

        assert!(health.ok);
        assert!(health.enabled);
        assert_eq!(health.mode, "local+remote-override");
        assert_eq!(
            health.remote_base_url.as_deref(),
            Some("http://127.0.0.1:4227")
        );
        assert_eq!(
            health.producer_script_path,
            producer_browser_worker_script_path().display().to_string()
        );
        assert_eq!(
            health.suno_script_path,
            suno_browser_worker_script_path().display().to_string()
        );
        assert_eq!(
            health.udio_script_path,
            udio_browser_worker_script_path().display().to_string()
        );
    }

    #[test]
    fn browser_executor_json_scalar_readers_preserve_bool_and_u64_contract() {
        let input = json!({
            "timeoutMs": 180000,
            "waitCompletion": true,
            "invalidString": "180000"
        });

        assert_eq!(read_json_u64(&input, "timeoutMs"), Some(180000));
        assert_eq!(read_json_bool(&input, "waitCompletion"), Some(true));
        assert_eq!(read_json_u64(&input, "invalidString"), None);
        assert_eq!(read_json_bool(&input, "missing"), None);
    }

    #[test]
    fn build_browser_executor_header_map_preserves_cookie_auth_and_metadata() {
        let input = json!({
            "cookieHeader": "__Secure-1PSID=psid",
            "authToken": "browser-bearer",
            "userAgent": "Mozilla/5.0",
            "acceptLanguage": "zh-CN",
            "origin": "https://gemini.google.com",
            "referer": "https://gemini.google.com/share/example",
            "deviceId": "device-123",
            "browserToken": "token-xyz",
            "referringPathname": "/share/example",
            "referringOrigin": "https://gemini.google.com"
        });

        let headers = build_browser_executor_header_map(&input);
        assert_eq!(
            header_map_string(&headers, "cookie").as_deref(),
            Some("__Secure-1PSID=psid")
        );
        assert_eq!(
            header_map_string(&headers, "authorization").as_deref(),
            Some("Bearer browser-bearer")
        );
        assert_eq!(
            header_map_string(&headers, "user-agent").as_deref(),
            Some("Mozilla/5.0")
        );
        assert_eq!(
            header_map_string(&headers, "device-id").as_deref(),
            Some("device-123")
        );
        assert_eq!(
            header_map_string(&headers, "referring-origin").as_deref(),
            Some("https://gemini.google.com")
        );
    }

    #[test]
    fn browser_execution_status_from_message_classifies_expected_outcomes() {
        assert_status("Cloudflare challenge required", "challenge");
        assert_status("operation timeout", "timed_out");
        assert_status("target closed unexpectedly", "crashed");
        assert_status("normal release", "released");
    }

    #[test]
    fn browser_executor_endpoint_kind_key_covers_media_and_research_routes() {
        assert_eq!(
            browser_executor_endpoint_kind_key(EndpointKind::ImagesGenerations),
            "images_generations"
        );
        assert_eq!(
            browser_executor_endpoint_kind_key(EndpointKind::VideosGenerations),
            "videos_generations"
        );
        assert_eq!(
            browser_executor_endpoint_kind_key(EndpointKind::ResearchGet),
            "research_get"
        );
    }

    #[test]
    fn missing_browser_executor_field_error_formats_provider_field_and_code() {
        let err = missing_browser_executor_field_error(
            "Gemini Canvas",
            "runtimeStateObjectKey",
            "browser_executor_missing_runtime_state",
        );

        assert_eq!(err.http_status, Some(400));
        assert_eq!(
            err.code.as_deref(),
            Some("browser_executor_missing_runtime_state")
        );
        assert_eq!(
            err.message.as_str(),
            "Gemini Canvas browser executor requires runtimeStateObjectKey."
        );
    }

    #[test]
    fn unsupported_browser_executor_provider_error_mentions_provider_name() {
        let err = unsupported_browser_executor_provider_error("mystery");
        assert_eq!(err.http_status, Some(400));
        assert_eq!(
            err.code.as_deref(),
            Some("browser_executor_unsupported_provider")
        );
        assert_eq!(
            err.message.as_str(),
            "Unsupported browser executor provider 'mystery'."
        );
    }

    #[test]
    fn parse_browser_executor_invocation_response_body_reads_result_contract() {
        let parsed = parse_browser_executor_invocation_response_body(
            "{\"ok\":true,\"status\":200,\"result\":{\"signedUrl\":\"https://example.com/video.mp4\"}}",
        )
        .expect("browser executor response");

        assert!(parsed.ok);
        assert_eq!(parsed.status, Some(200));
        assert_eq!(
            parsed
                .result
                .as_ref()
                .and_then(|value| value.get("signedUrl"))
                .and_then(Value::as_str),
            Some("https://example.com/video.mp4")
        );
    }

    #[test]
    fn parse_browser_executor_invocation_response_body_rejects_invalid_json_contract() {
        let error = parse_browser_executor_invocation_response_body("{\"ok\":true")
            .expect_err("invalid browser executor body should fail");
        assert!(
            error.to_string().contains("EOF"),
            "expected serde parse error, got: {error}"
        );
    }

    #[test]
    fn extract_browser_executor_invocation_success_reads_result_payload() {
        let result = parse_browser_executor_invocation_response_body(
            "{\"ok\":true,\"status\":200,\"result\":{\"signedUrl\":\"https://example.com/video.mp4\"}}",
        )
        .expect("browser executor response");

        let payload = extract_browser_executor_invocation_success(result);
        assert_eq!(
            payload
                .as_ref()
                .and_then(|value| value.get("signedUrl"))
                .and_then(Value::as_str),
            Some("https://example.com/video.mp4")
        );
    }

    #[test]
    fn extract_browser_executor_invocation_success_preserves_missing_result_contract() {
        let result =
            parse_browser_executor_invocation_response_body("{\"ok\":true,\"status\":200}")
                .expect("browser executor response");

        assert!(
            extract_browser_executor_invocation_success(result).is_none(),
            "missing result should stay as the fallback-to-local None contract"
        );
    }

    #[test]
    fn classify_browser_executor_invocation_failure_prefers_error_contract() {
        let result = parse_browser_executor_invocation_response_body(
            "{\"ok\":false,\"error\":{\"code\":\"browser_executor_failed\",\"message\":\"challenge required\",\"status\":422,\"body\":\"{\\\"detail\\\":\\\"Unauthorized\\\"}\"}}",
        )
        .expect("browser executor response");

        let error = classify_browser_executor_invocation_failure(result, "gemini_canvas");
        assert_eq!(error.provider_name.as_deref(), Some("gemini_canvas"));
        assert_eq!(error.code.as_deref(), Some("browser_executor_failed"));
        assert_eq!(error.message, "challenge required");
        assert_eq!(error.http_status, Some(422));
    }

    #[test]
    fn classify_browser_executor_invocation_failure_uses_top_level_status_when_error_missing() {
        let result =
            parse_browser_executor_invocation_response_body("{\"ok\":false,\"status\":503}")
                .expect("browser executor response");

        let error = classify_browser_executor_invocation_failure(result, "gemini_canvas");
        assert_eq!(error.provider_name.as_deref(), Some("gemini_canvas"));
        assert_eq!(error.http_status, Some(503));
        assert_eq!(error.code, None);
        assert!(matches!(error.kind, crate::error::ErrorKind::ServerError));
    }

    #[test]
    fn classify_browser_executor_challenge_failure_disables_automatic_replay() {
        let result = parse_browser_executor_invocation_response_body(
            "{\"ok\":false,\"status\":429,\"error\":{\"code\":\"suno_browser_challenge_required\",\"message\":\"Complete the visible Suno security check.\",\"status\":429}}",
        )
        .expect("browser executor response");

        let error = classify_browser_executor_invocation_failure(result, "suno");
        assert_eq!(error.http_status, Some(429));
        assert_eq!(
            error.code.as_deref(),
            Some("suno_browser_challenge_required")
        );
        assert!(!error.retryable);
        assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
    }

    #[test]
    fn classify_browser_executor_ordinary_rate_limit_remains_retryable() {
        let result = parse_browser_executor_invocation_response_body(
            "{\"ok\":false,\"status\":429,\"error\":{\"code\":\"suno_rate_limited\",\"message\":\"Rate limit exceeded.\",\"status\":429}}",
        )
        .expect("browser executor response");

        let error = classify_browser_executor_invocation_failure(result, "suno");
        assert!(error.retryable);
        assert!(matches!(error.fallback_hint, FallbackHint::Retry { .. }));
    }

    #[test]
    fn build_browser_executor_service_invocation_failure_response_preserves_gateway_error_contract()
    {
        let error = crate::error::GatewayError::service_unavailable("browser closed unexpectedly")
            .with_code("browser_executor_crashed");

        let response =
            build_browser_executor_service_invocation_failure_response("producer", error);
        assert!(!response.ok);
        assert_eq!(response.provider, "producer");
        assert_eq!(response.status, Some(503));
        assert!(response.result.is_none());
        assert_eq!(
            response
                .error
                .as_ref()
                .and_then(|value| value.code.as_deref()),
            Some("browser_executor_crashed")
        );
        assert_eq!(
            response
                .error
                .as_ref()
                .and_then(|value| value.message.as_deref()),
            Some("browser closed unexpectedly")
        );
        assert_eq!(
            response
                .error
                .as_ref()
                .and_then(|value| value.body.as_deref()),
            Some("browser closed unexpectedly")
        );
        assert_eq!(response.browser_execution_status, "crashed");
    }

    #[test]
    fn build_browser_executor_service_invocation_failure_response_marks_challenge_status() {
        let error = crate::error::GatewayError::bad_request("challenge required before continuing")
            .with_code("browser_executor_challenge");

        let response =
            build_browser_executor_service_invocation_failure_response("gemini_canvas", error);
        assert!(!response.ok);
        assert_eq!(response.provider, "gemini_canvas");
        assert_eq!(response.status, Some(400));
        assert_eq!(response.browser_execution_status, "challenge");
    }

    #[test]
    fn build_browser_executor_service_invocation_success_response_wraps_result_contract() {
        let response = build_browser_executor_service_invocation_success_response(
            "producer",
            serde_json::json!({
                "signedUrl": "https://example.com/video.mp4"
            }),
        );

        assert!(response.ok);
        assert_eq!(response.provider, "producer");
        assert_eq!(response.status, Some(200));
        assert_eq!(
            response
                .result
                .as_ref()
                .and_then(|value| value.get("signedUrl"))
                .and_then(Value::as_str),
            Some("https://example.com/video.mp4")
        );
        assert!(response.error.is_none());
        assert!(response.lease.is_none());
        assert_eq!(response.browser_execution_status, "completed");
    }

    #[test]
    fn build_browser_executor_service_invocation_success_response_preserves_null_payload() {
        let response =
            build_browser_executor_service_invocation_success_response("suno", Value::Null);

        assert!(response.ok);
        assert_eq!(response.provider, "suno");
        assert_eq!(response.status, Some(200));
        assert_eq!(response.result, Some(Value::Null));
        assert!(response.error.is_none());
        assert_eq!(response.browser_execution_status, "completed");
    }
}
