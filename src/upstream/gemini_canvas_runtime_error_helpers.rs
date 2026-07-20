use crate::error::GatewayError;

pub(crate) fn summarize_gateway_error(error: &GatewayError) -> String {
    let status = error
        .http_status
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let code = error.code.as_deref().unwrap_or("unknown").to_string();
    format!("status={status}, code={code}, message={}", error.message)
}

pub(crate) fn append_gateway_error_summary(
    mut error: GatewayError,
    field: &str,
    summary: Option<&str>,
) -> GatewayError {
    if let Some(summary) = summary.filter(|value| !value.trim().is_empty()) {
        error.message = format!("{}; {field}={summary}", error.message);
    }
    error
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
