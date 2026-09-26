//! LumaLabs local and remote browser worker results and failure classification.

use crate::error::{classify_upstream_error, sanitize_provider_error_message, GatewayError};
use crate::upstream::browser_worker_types::LumalabsBrowserWorkerResult;
use serde_json::json;

pub(crate) fn parse_lumalabs_browser_worker_output(
    stdout: &str,
    stderr: &str,
) -> Result<LumalabsBrowserWorkerResult, GatewayError> {
    if stdout.trim().is_empty() {
        return Err(crate::protocol::lumalabs::empty_browser_worker_output_error(stderr));
    }

    serde_json::from_str::<LumalabsBrowserWorkerResult>(stdout).map_err(|error| {
        crate::protocol::lumalabs::browser_worker_output_parse_error(
            error.to_string().as_str(),
            stdout,
        )
    })
}

pub(crate) fn parse_lumalabs_browser_worker_verified_output(
    stdout: &str,
    stderr: &str,
    provider: &str,
) -> Result<String, GatewayError> {
    let result = parse_lumalabs_browser_worker_output(stdout, stderr)?;
    resolve_lumalabs_browser_worker_result(result, stderr, provider)
}

pub(crate) fn classify_lumalabs_browser_worker_failure(
    result: LumalabsBrowserWorkerResult,
    stderr: &str,
    provider: &str,
) -> GatewayError {
    let error = result.error;
    let status = error.as_ref().and_then(|entry| entry.status).unwrap_or(500);
    let body_text = error
        .as_ref()
        .and_then(|entry| entry.body.as_deref())
        .or_else(|| stderr.is_empty().then_some("").or(Some(stderr)))
        .unwrap_or_default();
    let mut gateway_error = classify_upstream_error(status, body_text, Some(provider));
    if let Some(worker_error) = error {
        if let Some(code) = worker_error.code {
            gateway_error.code = Some(code);
        }
        if let Some(message) = worker_error.message {
            // Classify the raw body first; bound the separate override before regex work.
            gateway_error.message = if message.len() > 16 * 1024 {
                "[upstream detail omitted: safe processing limit exceeded]".to_string()
            } else {
                sanitize_provider_error_message(&message)
            };
        }
        if gateway_error.http_status.is_none() {
            gateway_error.http_status = Some(status);
        }
    }
    gateway_error
}

pub(crate) fn extract_lumalabs_browser_worker_success(
    result: LumalabsBrowserWorkerResult,
) -> Result<String, GatewayError> {
    crate::protocol::lumalabs::extract_browser_worker_signed_url(result.signed_url)
}

pub(crate) fn resolve_lumalabs_browser_worker_result(
    result: LumalabsBrowserWorkerResult,
    stderr: &str,
    provider: &str,
) -> Result<String, GatewayError> {
    if result.ok {
        return extract_lumalabs_browser_worker_success(result);
    }

    Err(classify_lumalabs_browser_worker_failure(
        result, stderr, provider,
    ))
}

pub(crate) fn build_lumalabs_browser_executor_service_result(
    signed_url: &str,
) -> serde_json::Value {
    json!({
        "signedUrl": signed_url,
    })
}

pub(crate) fn parse_lumalabs_remote_browser_executor_signed_url(
    value: &serde_json::Value,
) -> Result<String, GatewayError> {
    crate::protocol::lumalabs::extract_remote_executor_signed_url(value)
}
