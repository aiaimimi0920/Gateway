use crate::error::{sanitize_provider_error_message, GatewayError};
use crate::upstream::response_preview_helpers::truncate_response_preview;
use tracing::debug;

pub(crate) fn compact_response_preview(body_text: &str, max_chars: usize) -> String {
    // Redact before shortening: a cut credential may no longer match the sanitizer.
    let sanitized = sanitize_provider_error_message(body_text);
    let mut chars = sanitized.chars();
    let preview: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{preview}...")
    } else {
        preview
    }
}

pub(crate) fn compact_sanitized_response_preview(body_text: &str, max_chars: usize) -> String {
    crate::upstream::response_preview_helpers::compact_response_preview(
        &sanitize_provider_error_message(body_text),
        max_chars,
    )
}

pub(crate) fn summarize_gateway_error(error: &GatewayError) -> String {
    let status = error
        .http_status
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let code = error.code.as_deref().unwrap_or("unknown").to_string();
    sanitize_provider_error_message(&format!(
        "status={status}, code={code}, message={}",
        error.message
    ))
}

pub(crate) fn append_gateway_error_summary(
    mut error: GatewayError,
    field: &str,
    summary: Option<&str>,
) -> GatewayError {
    if let Some(summary) = summary.filter(|value| !value.trim().is_empty()) {
        error.message = format!("{}; {field}={summary}", error.message);
    }
    error.message = sanitize_provider_error_message(&error.message);
    error
}

pub(crate) fn append_gateway_error_fields(error: &mut GatewayError, fields: &[(&str, String)]) {
    for (field, value) in fields {
        error.message.push_str("; ");
        error.message.push_str(field);
        error.message.push('=');
        error.message.push_str(value);
    }
    error.message = sanitize_provider_error_message(&error.message);
}

pub(crate) fn wrap_gemini_canvas_stream_parse_error(
    body_text: &str,
    error: GatewayError,
) -> GatewayError {
    // Select both previews from sanitized input; a sliced JWT can evade redaction.
    let sanitized_body = sanitize_provider_error_message(body_text);
    let body_text = sanitized_body.as_str();
    let preview = |body: &str| {
        let trimmed = body.trim();
        if trimmed.is_empty() {
            "<empty>".to_string()
        } else {
            truncate_response_preview(trimmed, 180).to_string()
        }
    };
    let tail_preview = |body: &str| {
        let trimmed = body.trim();
        if trimmed.is_empty() {
            "<empty>".to_string()
        } else {
            let chars: Vec<char> = trimmed.chars().collect();
            let start = chars.len().saturating_sub(180);
            chars[start..].iter().collect::<String>()
        }
    };
    let stream_head_preview = preview(body_text);
    let stream_tail_preview = tail_preview(body_text);
    let preview = body_text.split_whitespace().collect::<Vec<_>>().join(" ");
    debug!(
        provider = "gemini_canvas_compatible",
        body_preview = %preview.chars().take(400).collect::<String>(),
        "Gemini Canvas StreamGenerate direct HTTP returned an unparseable frame payload"
    );
    let mut wrapped = error.with_provider("gemini_canvas_compatible");
    wrapped.message = sanitize_provider_error_message(&format!(
        "{}; stream_head={stream_head_preview}; stream_tail={stream_tail_preview}",
        wrapped.message
    ));
    wrapped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_gateway_error_formats_status_code_and_message() {
        let mut error = GatewayError::service_unavailable("upstream refused")
            .with_provider("gemini_canvas_compatible")
            .with_code("ctx_busy");
        error.http_status = Some(503);

        assert_eq!(
            summarize_gateway_error(&error),
            "status=503, code=ctx_busy, message=upstream refused"
        );
    }

    #[test]
    fn append_gateway_error_summary_appends_named_context_once() {
        let error = GatewayError::service_unavailable("base message")
            .with_provider("gemini_canvas_compatible")
            .with_code("base_code");

        let updated = append_gateway_error_summary(error, "followup_stage", Some("PCck7e"));

        assert_eq!(updated.message, "base message; followup_stage=PCck7e");
        assert_eq!(updated.code.as_deref(), Some("base_code"));
    }
}
