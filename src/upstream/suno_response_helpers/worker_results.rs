//! Suno local and remote browser worker admission, result envelopes and failure classification.

use crate::error::{classify_upstream_error, sanitize_provider_error_message, GatewayError};
use crate::upstream::browser_worker_types::SunoBrowserWorkerResult;
use crate::upstream::browser_worker_types::SunoBrowserWorkerSuccess;
use serde_json::json;
use serde_json::Value;

pub(crate) fn parse_suno_remote_browser_worker_success(
    result: Value,
) -> Result<SunoBrowserWorkerSuccess, GatewayError> {
    serde_json::from_value::<SunoBrowserWorkerSuccess>(result).map_err(|error| {
        crate::protocol::suno::remote_browser_worker_result_parse_error(error.to_string().as_str())
    })
}

pub(crate) fn parse_suno_remote_browser_worker_verified_result(
    result: Value,
) -> Result<SunoBrowserWorkerSuccess, GatewayError> {
    let worker = parse_suno_remote_browser_worker_success(result)?;
    if worker.clips.is_empty() {
        return Err(crate::protocol::suno::missing_browser_worker_clips_error());
    }
    Ok(worker)
}

pub(crate) fn parse_suno_browser_worker_output(
    stdout: &str,
    stderr: &str,
) -> Result<SunoBrowserWorkerResult, GatewayError> {
    if stdout.trim().is_empty() {
        return Err(crate::protocol::suno::empty_browser_worker_output_error(
            stderr,
        ));
    }

    serde_json::from_str::<SunoBrowserWorkerResult>(stdout).map_err(|error| {
        crate::protocol::suno::browser_worker_output_parse_error(error.to_string().as_str(), stdout)
    })
}

pub(crate) fn extract_suno_browser_worker_success(
    result: SunoBrowserWorkerResult,
) -> Result<SunoBrowserWorkerSuccess, GatewayError> {
    let payload = result
        .result
        .ok_or_else(crate::protocol::suno::missing_browser_worker_result_error)?;
    if payload.clips.is_empty() {
        return Err(crate::protocol::suno::missing_browser_worker_clips_error());
    }
    Ok(payload)
}

pub(crate) fn resolve_suno_browser_worker_result(
    result: SunoBrowserWorkerResult,
    provider: &str,
) -> Result<SunoBrowserWorkerSuccess, GatewayError> {
    if result.ok {
        return extract_suno_browser_worker_success(result);
    }

    Err(classify_suno_browser_worker_failure(result, provider))
}

pub(crate) fn build_suno_browser_executor_service_result(
    result: &SunoBrowserWorkerSuccess,
) -> serde_json::Value {
    json!({
        "clips": result.clips,
        "completed": result.completed,
        "message": result.message,
    })
}

pub(crate) fn classify_suno_browser_worker_failure(
    result: SunoBrowserWorkerResult,
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
        .unwrap_or_default();
    let mut gateway_error = classify_upstream_error(status, body_text, Some(provider));
    if let Some(error) = result.error {
        if let Some(code) = error.code {
            gateway_error.code = Some(code);
        }
        if let Some(message) = error.message {
            // Classify the raw body first; bound the separate override before regex work.
            gateway_error.message = if message.len() > 16 * 1024 {
                "[upstream detail omitted: safe processing limit exceeded]".to_string()
            } else {
                sanitize_provider_error_message(&message)
            };
        }
        if gateway_error.http_status.is_none() {
            gateway_error.http_status = error.status.or(Some(status));
        }
    }
    gateway_error
}
