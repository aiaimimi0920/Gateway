use std::time::{SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use serde_json::json;

use super::{GEMINI_WEB_BROWSER_CHALLENGE_REQUIRED_CODE, GEMINI_WEB_SESSION_INVALID_CODE};
use crate::error::{classify_upstream_error, GatewayError};
use crate::protocol::canonical::CanonicalRelayResponse;

mod frames;
mod payload;

use frames::{parse_response_envelopes, strip_xssi_prefix};
use payload::{extract_generate_response, select_more_complete_response};

pub fn classify_gemini_web_http_error(
    status: u16,
    content_type: Option<&str>,
    body: &str,
) -> GatewayError {
    let provider = "gemini_web_compatible";
    if response_indicates_browser_challenge(status, content_type, body) {
        let mut error = GatewayError::service_unavailable(
            "Gemini Web direct replay hit an upstream browser challenge and requires fresh session material.",
        )
        .with_provider(provider)
        .with_code(GEMINI_WEB_BROWSER_CHALLENGE_REQUIRED_CODE);
        error.http_status = Some(status);
        return error;
    }
    if response_indicates_session_invalid(status, content_type, body) {
        let mut error = GatewayError::unauthorized(
            "Gemini Web browser session is invalid or expired and must be refreshed before replay can continue.",
        )
        .with_provider(provider)
        .with_code(GEMINI_WEB_SESSION_INVALID_CODE);
        error.http_status = Some(status);
        return error;
    }
    classify_upstream_error(status, body, Some(provider))
}

pub fn response_indicates_browser_challenge(
    status: u16,
    content_type: Option<&str>,
    body: &str,
) -> bool {
    let normalized_content_type = content_type.unwrap_or_default().to_ascii_lowercase();
    let lower = body.to_ascii_lowercase();
    let html_like = normalized_content_type.contains("text/html")
        || lower.contains("<!doctype html")
        || lower.contains("<html");
    let challenge_like = lower.contains("captcha")
        || lower.contains("challenge")
        || lower.contains("verify you are human")
        || lower.contains("sorry/index")
        || lower.contains("bot verification")
        || lower.contains("unusual traffic")
        || lower.contains("security check");
    let gemini_shell_like =
        lower.contains("bardchatui") || lower.contains("gemini.gstatic.com/_/mss/boq-bard-web");
    if status == 200 && html_like && gemini_shell_like {
        return false;
    }
    html_like && challenge_like && matches!(status, 200 | 403 | 429 | 503)
}

pub fn response_indicates_session_invalid(
    status: u16,
    content_type: Option<&str>,
    body: &str,
) -> bool {
    let normalized_content_type = content_type.unwrap_or_default().to_ascii_lowercase();
    let lower = body.to_ascii_lowercase();
    let html_like = normalized_content_type.contains("text/html")
        || lower.contains("<!doctype html")
        || lower.contains("<html");
    let auth_like = lower.contains("accounts.google.com")
        || lower.contains("/signin")
        || lower.contains("sign in")
        || lower.contains("log in")
        || lower.contains("unauthorized")
        || lower.contains("session expired");
    let gemini_shell_like =
        lower.contains("bardchatui") || lower.contains("gemini.gstatic.com/_/mss/boq-bard-web");
    if status == 200 && html_like && auth_like && gemini_shell_like {
        return false;
    }
    matches!(status, 401 | 403 | 200) && html_like && auth_like
}

pub fn accumulate_gemini_web_response(
    body: &str,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let preview = |value: &str| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            "<empty>".to_string()
        } else {
            trimmed.chars().take(220).collect::<String>()
        }
    };
    let tail_preview = |value: &str| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            "<empty>".to_string()
        } else {
            let chars: Vec<char> = trimmed.chars().collect();
            let start = chars.len().saturating_sub(220);
            chars[start..].iter().collect::<String>()
        }
    };
    let normalized = strip_xssi_prefix(body);
    let frames = parse_response_envelopes(normalized);
    if frames.is_empty() {
        return Err(GatewayError::server_error(format!(
            "Gemini Web response did not contain any parseable frames. body_head={}; body_tail={}",
            preview(normalized),
            tail_preview(normalized)
        ))
        .with_provider("gemini_web_compatible")
        .with_code("gemini_web_invalid_frame_response"));
    }

    let mut best_result = None;
    for frame in &frames {
        if let Some(result) = extract_generate_response(frame, model) {
            best_result = select_more_complete_response(best_result, result);
        }
    }

    if let Some(result) = best_result {
        return Ok(result);
    }

    Err(GatewayError::server_error(format!(
        "Gemini Web response frames did not include a usable text candidate. frame_count={}; body_head={}; body_tail={}",
        frames.len(),
        preview(normalized),
        tail_preview(normalized)
    ))
    .with_provider("gemini_web_compatible")
    .with_code("gemini_web_missing_candidate_text"))
}

pub fn parse_response(body: &str, model: &str) -> Result<CanonicalRelayResponse, GatewayError> {
    accumulate_gemini_web_response(body, model)
}

pub fn translate_gemini_web_to_openai_sse(
    body: &str,
    model: &str,
) -> Result<Vec<Bytes>, GatewayError> {
    let canonical = accumulate_gemini_web_response(body, model)?;
    let response_id = format!("chatcmpl_geminiweb_{}", uuid::Uuid::new_v4().simple());
    let created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let chunk = json!({
        "id": response_id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": canonical.model,
        "choices": [{
            "index": 0,
            "delta": { "content": canonical.text },
            "finish_reason": canonical.finish_reason,
        }]
    });
    Ok(vec![
        Bytes::from(format!("data: {chunk}\n\n")),
        Bytes::from_static(b"data: [DONE]\n\n"),
    ])
}

#[cfg(test)]
mod tests;
