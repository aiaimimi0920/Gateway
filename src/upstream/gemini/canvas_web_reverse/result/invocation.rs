//! Validate browser-pool invocation envelopes and classify retryable failures.

use serde_json::Value;

use super::{
    GeminiCanvasBrowserOwnedFetchInvocationResult, GeminiCanvasBrowserOwnedInvocationResult,
    GeminiCanvasBrowserOwnedPoolResult,
};
use crate::error::{classify_upstream_error, GatewayError};
use crate::upstream::gemini::canvas_program_web_reverse as program;

fn parse_browser_pool_result(
    provider: &str,
    body_text: &str,
    parse_error_code: &'static str,
    context_label: &str,
) -> Result<GeminiCanvasBrowserOwnedPoolResult, GatewayError> {
    serde_json::from_str(body_text).map_err(|error| {
        GatewayError::server_error(format!(
            "Failed to parse {context_label}: {error}. body: {body_text}"
        ))
        .with_provider(provider)
        .with_code(parse_error_code)
    })
}

fn classify_browser_pool_failure(
    provider: &str,
    http_status: u16,
    result: GeminiCanvasBrowserOwnedPoolResult,
) -> GatewayError {
    let mut gateway_error = classify_upstream_error(
        result
            .status
            .or_else(|| result.error.as_ref().and_then(|entry| entry.status))
            .unwrap_or(http_status),
        result
            .error
            .as_ref()
            .and_then(|entry| entry.body.as_deref())
            .unwrap_or(""),
        Some(provider),
    );
    if let Some(error) = result.error {
        if let Some(code) = error.code {
            gateway_error.code = Some(code);
        }
        if let Some(message) = error.message {
            gateway_error.message = message;
        }
    }
    gateway_error
}

pub fn parse_browser_invocation_response(
    provider: &str,
    http_status: u16,
    body_text: &str,
    expected_operation: &str,
) -> Result<program::GeminiCanvasBrowserInvocationResult, GatewayError> {
    let result = parse_browser_pool_result(
        provider,
        body_text,
        "gemini_canvas_browser_pool_output_parse_failed",
        "Gemini Canvas browser pool response",
    )?;
    if !result.ok {
        return Err(classify_browser_pool_failure(provider, http_status, result));
    }
    let result_body = result.result.ok_or_else(|| {
        GatewayError::server_error(
            "Gemini Canvas browser pool reported success without a result payload.",
        )
        .with_provider(provider)
        .with_code("gemini_canvas_browser_pool_missing_result")
    })?;
    let invocation = serde_json::from_value::<GeminiCanvasBrowserOwnedInvocationResult>(
        result_body,
    )
    .map_err(|error| {
        GatewayError::server_error(format!(
            "Failed to decode Gemini Canvas browser pool media payload: {error}"
        ))
        .with_provider(provider)
        .with_code("gemini_canvas_browser_pool_result_decode_failed")
    })?;
    if invocation.operation != expected_operation {
        return Err(GatewayError::server_error(format!(
            "Gemini Canvas browser pool returned '{}' while '{}' was requested.",
            invocation.operation, expected_operation
        ))
        .with_provider(provider)
        .with_code("gemini_canvas_browser_pool_operation_mismatch"));
    }
    Ok(invocation.into())
}

pub fn parse_remote_browser_invocation_value(
    provider: &str,
    result: Value,
    context_label: &str,
    error_code: &'static str,
) -> Result<program::GeminiCanvasBrowserInvocationResult, GatewayError> {
    serde_json::from_value::<GeminiCanvasBrowserOwnedInvocationResult>(result)
        .map(Into::into)
        .map_err(|error| {
            GatewayError::server_error(format!("Failed to parse {context_label}: {error}"))
                .with_provider(provider)
                .with_code(error_code)
        })
}

pub fn parse_remote_media_browser_invocation_value(
    provider: &str,
    result: Value,
    operation: &str,
) -> Result<program::GeminiCanvasBrowserInvocationResult, GatewayError> {
    parse_remote_browser_invocation_value(
        provider,
        result,
        &format!("remote Gemini Canvas {operation} result"),
        "gemini_canvas_remote_result_parse_failed",
    )
}

pub fn parse_remote_modular_media_browser_invocation_value(
    provider: &str,
    result: Value,
    operation: &str,
) -> Result<program::GeminiCanvasBrowserInvocationResult, GatewayError> {
    parse_remote_browser_invocation_value(
        provider,
        result,
        &format!("remote Gemini Canvas modular {operation} result"),
        "gemini_canvas_modular_remote_result_parse_failed",
    )
}

pub fn parse_connected_fetch_invocation_response(
    provider: &str,
    http_status: u16,
    body_text: &str,
) -> Result<GeminiCanvasBrowserOwnedFetchInvocationResult, GatewayError> {
    let result = parse_browser_pool_result(
        provider,
        body_text,
        "gemini_canvas_connected_fetch_output_parse_failed",
        "Gemini Canvas connected fetch response",
    )?;
    if !result.ok {
        return Err(classify_browser_pool_failure(provider, http_status, result));
    }
    let result_body = result.result.ok_or_else(|| {
        GatewayError::server_error(
            "Gemini Canvas connected fetch reported success without a result payload.",
        )
        .with_provider(provider)
        .with_code("gemini_canvas_connected_fetch_missing_result")
    })?;
    let invocation =
        serde_json::from_value::<GeminiCanvasBrowserOwnedFetchInvocationResult>(result_body)
            .map_err(|error| {
                GatewayError::server_error(format!(
                    "Failed to decode Gemini Canvas connected fetch payload: {error}"
                ))
                .with_provider(provider)
                .with_code("gemini_canvas_connected_fetch_decode_failed")
            })?;
    if !(200..300).contains(&invocation.status) {
        return Err(classify_upstream_error(
            invocation.status,
            invocation.body_text.as_deref().unwrap_or(""),
            Some(provider),
        ));
    }
    Ok(invocation)
}

pub fn parse_connected_fetch_json_body(
    provider: &str,
    body_text: Option<&str>,
) -> Result<Value, GatewayError> {
    let body_text = body_text.ok_or_else(|| {
        GatewayError::server_error("Gemini Canvas connected fetch completed without a JSON body.")
            .with_provider(provider)
            .with_code("gemini_canvas_connected_fetch_missing_body")
    })?;
    serde_json::from_str::<Value>(body_text).map_err(|error| {
        GatewayError::server_error(format!(
            "Gemini Canvas connected fetch did not return valid JSON: {error}"
        ))
        .with_provider(provider)
        .with_code("gemini_canvas_connected_fetch_invalid_json")
    })
}

pub fn browser_request_retry_delay_ms(error: &GatewayError, attempts: usize) -> Option<u64> {
    let code = error.code.as_deref();
    let explicit_quota_gate = matches!(code, Some("gemini_canvas_video_quota_reached"));
    let retryable = matches!(
        code,
        Some("gemini_canvas_context_busy")
            | Some("gemini_canvas_auth_required")
            | Some("gemini_canvas_browser_worker_failed")
    ) || (error.http_status == Some(429) && !explicit_quota_gate)
        || (error.retryable && code.is_none());
    if !retryable || attempts >= 2 {
        return None;
    }
    Some(match code {
        Some("gemini_canvas_auth_required") => 2_000,
        Some("gemini_canvas_context_busy") => 3_000 + (attempts as u64 * 1_500),
        _ => 2_500,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{ErrorKind, FallbackHint};

    #[test]
    fn browser_request_retry_delay_retries_retryable_network_error_without_code() {
        let error = GatewayError {
            kind: ErrorKind::Network,
            message: "Connection error: client error (SendRequest)".to_string(),
            code: None,
            http_status: None,
            retryable: true,
            fallback_hint: FallbackHint::Retry {
                delay_ms: 2_000,
                reason: "temporary network error".to_string(),
            },
            provider_name: Some("gemini_canvas_compatible".to_string()),
        };

        assert_eq!(browser_request_retry_delay_ms(&error, 0), Some(2_500));
        assert_eq!(browser_request_retry_delay_ms(&error, 1), Some(2_500));
        assert_eq!(browser_request_retry_delay_ms(&error, 2), None);
    }
}
