//! Suno challenge, generation and feed JSON response decoding.

use super::upstream_errors::ensure_successful_suno_upstream_status;
use crate::error::{sanitize_provider_error_message, GatewayError};
use crate::upstream::response_preview_helpers::truncate_response_preview;
use serde_json::Value;

pub(crate) fn parse_suno_json_response(
    body_text: &str,
    message: &str,
    code: &'static str,
) -> Result<Value, GatewayError> {
    serde_json::from_str(body_text).map_err(|error| {
        let detail = if body_text.len() > 16 * 1024 {
            "[upstream detail omitted: safe processing limit exceeded]".to_string()
        } else if body_text.trim().is_empty() {
            "<empty body>".to_string()
        } else {
            // Truncating first can leave part of a quoted credential outside redaction.
            let sanitized = sanitize_provider_error_message(body_text);
            truncate_response_preview(&sanitized, 200).to_string()
        };
        GatewayError::server_error(sanitize_provider_error_message(&format!(
            "{message} {error}; body={detail}"
        )))
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
