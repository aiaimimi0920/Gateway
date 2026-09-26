//! Udio local and remote browser worker admission, result envelopes and failure classification.

use crate::error::{classify_upstream_error, sanitize_provider_error_message, GatewayError};
use crate::upstream::browser_worker_types::UdioBrowserWorkerResult;
use crate::upstream::browser_worker_types::UdioBrowserWorkerSuccess;
use serde_json::json;

pub(crate) fn parse_udio_browser_worker_output(
    stdout: &str,
    stderr: &str,
) -> Result<UdioBrowserWorkerResult, GatewayError> {
    if stdout.trim().is_empty() {
        return Err(crate::protocol::udio::empty_browser_worker_output_error(
            stderr,
        ));
    }

    serde_json::from_str::<UdioBrowserWorkerResult>(stdout).map_err(|error| {
        crate::protocol::udio::browser_worker_output_parse_error(error.to_string().as_str(), stdout)
    })
}

pub(crate) fn parse_udio_browser_worker_verified_output(
    stdout: &str,
    stderr: &str,
    provider: &str,
) -> Result<UdioBrowserWorkerSuccess, GatewayError> {
    let result = parse_udio_browser_worker_output(stdout, stderr)?;
    resolve_udio_browser_worker_result(result, stderr, provider)
}

pub(crate) fn parse_udio_remote_browser_worker_success(
    result: serde_json::Value,
) -> Result<UdioBrowserWorkerSuccess, GatewayError> {
    serde_json::from_value::<UdioBrowserWorkerSuccess>(result).map_err(|error| {
        crate::protocol::udio::remote_browser_worker_result_parse_error(error.to_string().as_str())
    })
}

pub(crate) fn parse_udio_remote_browser_worker_verified_result(
    result: serde_json::Value,
) -> Result<UdioBrowserWorkerSuccess, GatewayError> {
    let worker = parse_udio_remote_browser_worker_success(result)?;
    if worker.track_ids.is_empty() {
        return Err(crate::protocol::udio::missing_browser_worker_track_ids_error());
    }
    Ok(worker)
}

pub(crate) fn extract_udio_browser_worker_success(
    result: UdioBrowserWorkerResult,
) -> Result<UdioBrowserWorkerSuccess, GatewayError> {
    let payload = result
        .result
        .ok_or_else(crate::protocol::udio::missing_browser_worker_result_error)?;
    if payload.track_ids.is_empty() {
        return Err(crate::protocol::udio::missing_browser_worker_track_ids_error());
    }
    Ok(payload)
}

pub(crate) fn resolve_udio_browser_worker_result(
    result: UdioBrowserWorkerResult,
    stderr: &str,
    provider: &str,
) -> Result<UdioBrowserWorkerSuccess, GatewayError> {
    if result.ok {
        return extract_udio_browser_worker_success(result);
    }

    Err(classify_udio_browser_worker_failure(
        result, stderr, provider,
    ))
}

pub(crate) fn build_udio_browser_executor_service_result(
    result: &UdioBrowserWorkerSuccess,
) -> serde_json::Value {
    json!({
        "trackIds": result.track_ids,
        "songs": result.songs,
        "completed": result.completed,
        "message": result.message,
    })
}

pub(crate) fn classify_udio_browser_worker_failure(
    result: UdioBrowserWorkerResult,
    stderr: &str,
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
        .or_else(|| stderr.is_empty().then_some("").or(Some(stderr)))
        .unwrap_or_default();
    let mut gateway_error = classify_upstream_error(status, body_text, Some(provider));
    if let Some(worker_error) = result.error {
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
