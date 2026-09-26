use crate::error::{classify_upstream_error, sanitize_provider_error_message, GatewayError};

const BROWSER_WORKER_DETAIL_MAX_BYTES: usize = 16 * 1024;
const OVERSIZED_BROWSER_WORKER_DETAIL: &str =
    "[upstream detail omitted: safe processing limit exceeded]";

fn bounded_detail(value: &str) -> &str {
    if value.len() > BROWSER_WORKER_DETAIL_MAX_BYTES {
        OVERSIZED_BROWSER_WORKER_DETAIL
    } else {
        value
    }
}

fn sanitize_detail(value: &str) -> String {
    sanitize_provider_error_message(bounded_detail(value))
}

pub(crate) fn classify_error(status: u16, body: &str, provider: &str) -> GatewayError {
    classify_upstream_error(status, bounded_detail(body), Some(provider))
}

pub(crate) fn enrich(message: String, body: Option<String>) -> String {
    let normalized_message = message.trim();
    let sanitized_message = sanitize_detail(normalized_message);
    let Some(body) = body
        .as_deref()
        .map(str::trim)
        .filter(|body| !body.is_empty())
    else {
        return sanitized_message;
    };
    if body == normalized_message {
        return sanitized_message;
    }

    let sanitized_body = sanitize_detail(body);
    sanitize_detail(&format!(
        "{sanitized_message} upstream body: {sanitized_body}"
    ))
}
