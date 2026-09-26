use base64::Engine;
use bytes::Bytes;
use serde_json::Value;

use crate::error::{classify_upstream_error, GatewayError};

use super::types::{
    GeminiCanvasBrowserFetchInvocationResult, GeminiCanvasBrowserInvocationResult,
    GeminiCanvasBrowserPoolResult,
};

fn parse_browser_pool_result(
    provider: &str,
    body_text: &str,
    parse_error_code: &'static str,
    context_label: &str,
) -> Result<GeminiCanvasBrowserPoolResult, GatewayError> {
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
    result: GeminiCanvasBrowserPoolResult,
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

pub fn parse_program_browser_invocation_response(
    provider: &str,
    http_status: u16,
    body_text: &str,
    expected_operation: &str,
) -> Result<GeminiCanvasBrowserInvocationResult, GatewayError> {
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
    let invocation = serde_json::from_value::<GeminiCanvasBrowserInvocationResult>(result_body)
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
    Ok(invocation)
}

pub fn parse_program_bootstrap_invocation_response(
    provider: &str,
    http_status: u16,
    body_text: &str,
) -> Result<GeminiCanvasBrowserInvocationResult, GatewayError> {
    let result = parse_browser_pool_result(
        provider,
        body_text,
        "gemini_canvas_program_bootstrap_output_parse_failed",
        "Gemini Canvas program bootstrap response",
    )?;
    if !result.ok {
        return Err(classify_browser_pool_failure(provider, http_status, result));
    }
    let result_body = result.result.ok_or_else(|| {
        GatewayError::server_error(
            "Gemini Canvas program bootstrap reported success without a result payload.",
        )
        .with_provider(provider)
        .with_code("gemini_canvas_program_bootstrap_missing_result")
    })?;
    let invocation = serde_json::from_value::<GeminiCanvasBrowserInvocationResult>(result_body)
        .map_err(|error| {
            GatewayError::server_error(format!(
                "Failed to decode Gemini Canvas program bootstrap payload: {error}"
            ))
            .with_provider(provider)
            .with_code("gemini_canvas_program_bootstrap_decode_failed")
        })?;
    if invocation.operation != "bootstrap_program" {
        return Err(GatewayError::server_error(format!(
            "Gemini Canvas program bootstrap returned '{}' instead of 'bootstrap_program'.",
            invocation.operation
        ))
        .with_provider(provider)
        .with_code("gemini_canvas_program_bootstrap_operation_mismatch"));
    }
    Ok(invocation)
}

pub fn parse_connected_fetch_invocation_response(
    provider: &str,
    http_status: u16,
    body_text: &str,
) -> Result<GeminiCanvasBrowserFetchInvocationResult, GatewayError> {
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
    let invocation = serde_json::from_value::<GeminiCanvasBrowserFetchInvocationResult>(
        result_body,
    )
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

pub fn parse_connected_fetch_get_json_body(
    provider: &str,
    body_text: Option<&str>,
) -> Result<Value, GatewayError> {
    let body_text = body_text.ok_or_else(|| {
        GatewayError::server_error(
            "Gemini Canvas connected fetch GET completed without a JSON body.",
        )
        .with_provider(provider)
        .with_code("gemini_canvas_connected_fetch_missing_body")
    })?;
    serde_json::from_str::<Value>(body_text).map_err(|error| {
        GatewayError::server_error(format!(
            "Gemini Canvas connected fetch GET did not return valid JSON: {error}"
        ))
        .with_provider(provider)
        .with_code("gemini_canvas_connected_fetch_invalid_json")
    })
}

pub fn decode_connected_fetch_body_bytes(
    provider: &str,
    invocation: &GeminiCanvasBrowserFetchInvocationResult,
) -> Result<(Bytes, Option<String>), GatewayError> {
    let bytes = if let Some(body_base64) = invocation.body_base64.as_deref() {
        Bytes::from(
            base64::engine::general_purpose::STANDARD
                .decode(body_base64)
                .map_err(|error| {
                    GatewayError::server_error(format!(
                        "Gemini Canvas connected fetch returned invalid base64 bytes: {error}"
                    ))
                    .with_provider(provider)
                    .with_code("gemini_canvas_connected_fetch_invalid_base64")
                })?,
        )
    } else if let Some(body_text) = invocation.body_text.as_ref() {
        Bytes::from(body_text.clone().into_bytes())
    } else {
        return Err(GatewayError::server_error(
            "Gemini Canvas connected fetch GET completed without a body payload.",
        )
        .with_provider(provider)
        .with_code("gemini_canvas_connected_fetch_missing_body"));
    };
    Ok((bytes, invocation.content_type.clone()))
}
