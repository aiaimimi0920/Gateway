use crate::error::{classify_upstream_error, GatewayError};
use crate::upstream::header_map_helpers::insert_runtime_header;
use rquest::header::HeaderMap;
use serde_json::Value;

#[derive(Debug)]
pub(crate) enum ProducerImageAttemptResolution {
    Success(Value),
    Retry(GatewayError),
    Fail(GatewayError),
}

pub(crate) fn extract_producer_send_message_job_id(
    status: u16,
    body: &Value,
    provider: &str,
) -> Result<String, GatewayError> {
    if !(200..300).contains(&status) {
        return Err(classify_upstream_error(
            status,
            &body.to_string(),
            Some(provider),
        ));
    }
    crate::protocol::producer::extract_job_id(body)
}

pub(crate) fn parse_producer_send_message_job(
    status: u16,
    body_text: &str,
    provider: &str,
) -> Result<String, GatewayError> {
    let body = serde_json::from_str::<Value>(body_text)
        .map_err(|_| producer_invalid_conversation_response_error(provider))?;
    extract_producer_send_message_job_id(status, &body, provider)
}

pub(crate) fn parse_producer_video_status_response(
    body_text: &str,
    provider: &str,
) -> Result<Value, GatewayError> {
    serde_json::from_str::<Value>(body_text)
        .map_err(|_| producer_invalid_video_status_response_error(provider))
}

pub(crate) fn parse_producer_video_status_http_response(
    status: u16,
    body_text: &str,
    provider: &str,
) -> Result<Value, GatewayError> {
    ensure_successful_producer_http_status(
        status,
        body_text,
        provider,
        Some("producer_http_video_status_failed"),
    )?;
    parse_producer_video_status_response(body_text, provider)
}

pub(crate) fn parse_producer_image_generation_response(
    body_text: &str,
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    model: &str,
    auth_token: Option<&str>,
    provider: &str,
) -> Result<Value, GatewayError> {
    let body = serde_json::from_str::<Value>(body_text)
        .map_err(|_| producer_invalid_image_response_error(provider))?;
    crate::protocol::producer::build_image_generation_response(&body, req, model, auth_token)
}

pub(crate) fn resolve_producer_image_attempt(
    status: u16,
    body_text: &str,
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    model: &str,
    auth_token: Option<&str>,
    provider: &str,
    can_retry: bool,
) -> ProducerImageAttemptResolution {
    if (200..300).contains(&status) {
        return match parse_producer_image_generation_response(
            body_text, req, model, auth_token, provider,
        ) {
            Ok(body) => ProducerImageAttemptResolution::Success(body),
            Err(error) => ProducerImageAttemptResolution::Fail(error),
        };
    }

    let gateway_error = classify_producer_image_request_failure(status, body_text);
    if can_retry && should_retry_producer_image_request(status, body_text) {
        ProducerImageAttemptResolution::Retry(gateway_error)
    } else {
        ProducerImageAttemptResolution::Fail(gateway_error)
    }
}

pub(crate) fn parse_producer_music_stream_http_response(
    status: u16,
    body_text: &str,
    provider: &str,
    model: &str,
    job_id: &str,
) -> Result<Value, GatewayError> {
    ensure_successful_producer_message_stream_status(status, body_text, provider)?;
    crate::protocol::producer::parse_producer_music_stream_text(body_text, model, job_id)
}

pub(crate) fn ensure_successful_producer_http_status(
    status: u16,
    body_text: &str,
    provider: &str,
    error_code: Option<&str>,
) -> Result<(), GatewayError> {
    if (200..300).contains(&status) {
        return Ok(());
    }
    let error = classify_upstream_error(status, body_text, Some(provider));
    if let Some(error_code) = error_code {
        Err(error.with_code(error_code))
    } else {
        Err(error)
    }
}

pub(crate) fn ensure_successful_producer_message_stream_status(
    status: u16,
    body_text: &str,
    provider: &str,
) -> Result<(), GatewayError> {
    ensure_successful_producer_http_status(status, body_text, provider, None)
}

pub(crate) fn producer_runtime_headers(
    base_headers: &HeaderMap,
    referer: &str,
) -> rquest::header::HeaderMap {
    let mut headers = base_headers.clone();
    if !headers.contains_key(rquest::header::ACCEPT) {
        insert_runtime_header(&mut headers, "accept", "application/json, text/plain, */*");
    }
    if !headers.contains_key(rquest::header::ORIGIN) {
        insert_runtime_header(&mut headers, "origin", "https://www.flowmusic.app");
    }
    insert_runtime_header(&mut headers, "referer", referer);
    headers
}

pub(crate) fn producer_stream_headers(
    base_headers: &HeaderMap,
    referer: &str,
) -> rquest::header::HeaderMap {
    let mut headers = producer_runtime_headers(base_headers, referer);
    insert_runtime_header(&mut headers, "accept", "text/event-stream");
    headers
}

pub(crate) fn producer_invalid_video_status_response_error(provider: &str) -> GatewayError {
    GatewayError::server_error("Producer.ai music-video status response was not valid JSON.")
        .with_provider(provider)
        .with_code("producer_invalid_video_status_response")
}

pub(crate) fn producer_invalid_image_response_error(provider: &str) -> GatewayError {
    GatewayError::server_error("Producer.ai image response was not valid JSON.")
        .with_provider(provider)
        .with_code("producer_invalid_image_response")
}

pub(crate) fn producer_image_retry_exhausted_error(provider: &str) -> GatewayError {
    GatewayError::server_error("Producer.ai image request exhausted retry attempts.")
        .with_provider(provider)
        .with_code("producer_image_retry_exhausted")
}

pub(crate) fn classify_producer_image_request_failure(
    status: u16,
    body_text: &str,
) -> GatewayError {
    classify_upstream_error(status, body_text, Some("producer_compatible"))
}

pub(crate) fn finalize_producer_image_retry_error(
    last_error: Option<GatewayError>,
    provider: &str,
) -> GatewayError {
    last_error.unwrap_or_else(|| producer_image_retry_exhausted_error(provider))
}

pub(crate) fn producer_invalid_conversation_response_error(provider: &str) -> GatewayError {
    GatewayError::server_error("Producer.ai conversation response was not valid JSON.")
        .with_provider(provider)
        .with_code("producer_invalid_conversation_response")
}

pub(crate) fn should_retry_producer_image_request(status: u16, body_text: &str) -> bool {
    if matches!(status, 502 | 503 | 504) {
        return true;
    }
    let normalized = body_text.to_ascii_lowercase();
    normalized.contains("gateway time-out")
        || normalized.contains("error code 504")
        || normalized.contains("temporarily unavailable")
}
