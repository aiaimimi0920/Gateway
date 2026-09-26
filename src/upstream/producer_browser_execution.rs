use crate::error::GatewayError;
use crate::upstream::browser_executor_helpers::{
    build_browser_executor_header_map, missing_browser_executor_field_error, read_json_u64,
};
use crate::upstream::browser_worker_message;
use crate::upstream::browser_worker_runtime_helpers::producer_browser_worker_script_path;
use crate::upstream::browser_worker_types::{
    ProducerBrowserWorkerInput, ProducerBrowserWorkerResult,
};
use crate::upstream::header_map_helpers::{extract_bearer_token, header_map_string};
use crate::upstream::producer_browser_worker_process::{
    execute_producer_browser_worker_process, PreparedProducerBrowserWorkerLaunch,
};
use rquest::header::HeaderMap;
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Debug)]
pub(crate) struct PreparedProducerBrowserExecutorServiceInput {
    pub(crate) base_url: String,
    pub(crate) headers: HeaderMap,
    pub(crate) request_body: Value,
    pub(crate) model: String,
    pub(crate) timeout: Duration,
}

pub(crate) fn build_producer_browser_executor_payload(
    base_url: &str,
    headers: &HeaderMap,
    request_body: &Value,
    model: &str,
    timeout: Duration,
    browser_executable_path: Option<String>,
) -> Value {
    json!({
        "baseUrl": base_url,
        "authToken": extract_bearer_token(headers),
        "cookieHeader": header_map_string(headers, "cookie"),
        "requestBody": request_body,
        "model": model,
        "timeoutMs": timeout.as_millis().min(u128::from(u64::MAX)) as u64,
        "browserExecutablePath": browser_executable_path,
        "userAgent": header_map_string(headers, "user-agent"),
        "acceptLanguage": header_map_string(headers, "accept-language"),
        "origin": header_map_string(headers, "origin"),
        "referer": header_map_string(headers, "referer"),
    })
}

pub(crate) fn build_producer_browser_executor_payload_from_prepared(
    prepared: &PreparedProducerBrowserExecutorServiceInput,
    browser_executable_path: Option<String>,
) -> Value {
    build_producer_browser_executor_payload(
        &prepared.base_url,
        &prepared.headers,
        &prepared.request_body,
        &prepared.model,
        prepared.timeout,
        browser_executable_path,
    )
}

pub(crate) fn prepare_producer_browser_execution_input(
    base_url: &str,
    headers: &HeaderMap,
    request_body: &Value,
    model: &str,
    timeout: Duration,
) -> PreparedProducerBrowserExecutorServiceInput {
    PreparedProducerBrowserExecutorServiceInput {
        base_url: base_url.trim_end_matches('/').to_string(),
        headers: headers.clone(),
        request_body: request_body.clone(),
        model: model.to_string(),
        timeout,
    }
}

pub(crate) fn build_producer_browser_worker_input<'a>(
    base_url: &'a str,
    headers: &HeaderMap,
    request_body: &'a Value,
    model: &'a str,
    accept_async_job: bool,
    timeout: Duration,
    browser_executable_path: Option<String>,
) -> ProducerBrowserWorkerInput<'a> {
    ProducerBrowserWorkerInput {
        base_url,
        auth_token: extract_bearer_token(headers),
        cookie_header: header_map_string(headers, "cookie"),
        request_body,
        model,
        accept_async_job,
        timeout_ms: timeout.as_millis().min(u128::from(u64::MAX)) as u64,
        browser_executable_path,
        user_agent: header_map_string(headers, "user-agent"),
        accept_language: header_map_string(headers, "accept-language"),
        origin: header_map_string(headers, "origin"),
        referer: header_map_string(headers, "referer"),
    }
}

pub(crate) fn serialize_producer_browser_worker_input(
    base_url: &str,
    headers: &HeaderMap,
    request_body: &Value,
    model: &str,
    accept_async_job: bool,
    timeout: Duration,
    browser_executable_path: Option<String>,
) -> Result<Vec<u8>, GatewayError> {
    let input = build_producer_browser_worker_input(
        base_url,
        headers,
        request_body,
        model,
        accept_async_job,
        timeout,
        browser_executable_path,
    );
    serde_json::to_vec(&input).map_err(|error| {
        crate::protocol::producer::browser_worker_input_serialize_error(error.to_string().as_str())
    })
}

pub(crate) fn serialize_producer_browser_worker_input_from_prepared(
    prepared: &PreparedProducerBrowserExecutorServiceInput,
    accept_async_job: bool,
    browser_executable_path: Option<String>,
) -> Result<Vec<u8>, GatewayError> {
    serialize_producer_browser_worker_input(
        &prepared.base_url,
        &prepared.headers,
        &prepared.request_body,
        &prepared.model,
        accept_async_job,
        prepared.timeout,
        browser_executable_path,
    )
}

pub(crate) fn prepare_producer_browser_worker_launch_from_prepared(
    prepared: &PreparedProducerBrowserExecutorServiceInput,
    accept_async_job: bool,
    browser_executable_path: Option<String>,
    node_bin: Option<String>,
) -> Result<PreparedProducerBrowserWorkerLaunch, GatewayError> {
    Ok(PreparedProducerBrowserWorkerLaunch {
        node_bin: node_bin.unwrap_or_else(|| "node".to_string()),
        script_path: producer_browser_worker_script_path(),
        stdin_json: serialize_producer_browser_worker_input_from_prepared(
            prepared,
            accept_async_job,
            browser_executable_path,
        )?,
    })
}

pub(crate) async fn execute_producer_browser_worker(
    provider: &str,
    prepared: &PreparedProducerBrowserExecutorServiceInput,
    accept_async_job: bool,
) -> Result<Value, GatewayError> {
    let launch = prepare_producer_browser_worker_launch_from_prepared(
        prepared,
        accept_async_job,
        std::env::var("PRODUCER_BROWSER_EXECUTABLE_PATH").ok(),
        std::env::var("PRODUCER_BROWSER_NODE_BIN").ok(),
    )?;

    let output = execute_producer_browser_worker_process(launch, prepared.timeout).await?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    parse_producer_browser_worker_verified_output(stdout.trim(), stderr.trim(), provider)
}

pub(crate) fn prepare_producer_browser_executor_service_input(
    input: &Value,
) -> Result<PreparedProducerBrowserExecutorServiceInput, GatewayError> {
    let base_url = input
        .get("baseUrl")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            missing_browser_executor_field_error(
                "Producer",
                "baseUrl",
                "browser_executor_missing_base_url",
            )
        })?;
    let request_body = input.get("requestBody").cloned().ok_or_else(|| {
        missing_browser_executor_field_error(
            "Producer",
            "requestBody",
            "browser_executor_missing_request_body",
        )
    })?;
    let model = input
        .get("model")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            missing_browser_executor_field_error(
                "Producer",
                "model",
                "browser_executor_missing_model",
            )
        })?;
    let timeout = Duration::from_millis(read_json_u64(input, "timeoutMs").unwrap_or(900_000));

    Ok(PreparedProducerBrowserExecutorServiceInput {
        base_url,
        headers: build_browser_executor_header_map(input),
        request_body,
        model,
        timeout,
    })
}

pub(crate) fn parse_producer_browser_worker_output(
    stdout: &str,
    stderr: &str,
) -> Result<ProducerBrowserWorkerResult, GatewayError> {
    if stdout.trim().is_empty() {
        return Err(crate::protocol::producer::empty_browser_worker_output_error(stderr));
    }

    serde_json::from_str::<ProducerBrowserWorkerResult>(stdout).map_err(|error| {
        crate::protocol::producer::browser_worker_output_parse_error(
            error.to_string().as_str(),
            stdout,
        )
    })
}

pub(crate) fn parse_producer_browser_worker_verified_output(
    stdout: &str,
    stderr: &str,
    provider: &str,
) -> Result<Value, GatewayError> {
    let result = parse_producer_browser_worker_output(stdout, stderr)?;
    resolve_producer_browser_worker_result(result, stderr, provider)
}

pub(crate) fn extract_producer_browser_worker_success(
    result: ProducerBrowserWorkerResult,
) -> Result<Value, GatewayError> {
    result
        .result
        .ok_or_else(crate::protocol::producer::missing_browser_worker_result_error)
}

pub(crate) fn resolve_producer_browser_worker_result(
    result: ProducerBrowserWorkerResult,
    stderr: &str,
    provider: &str,
) -> Result<Value, GatewayError> {
    if result.ok {
        return extract_producer_browser_worker_success(result);
    }

    Err(classify_producer_browser_worker_failure(
        result, stderr, provider,
    ))
}

pub(crate) fn classify_producer_browser_worker_failure(
    result: ProducerBrowserWorkerResult,
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
    let mut gateway_error = browser_worker_message::classify_error(status, body_text, provider);
    if let Some(worker_error) = result.error {
        if let Some(code) = worker_error.code {
            gateway_error.code = Some(code);
        }
        if let Some(message) = worker_error.message {
            gateway_error.message = browser_worker_message::enrich(message, worker_error.body);
        }
        if gateway_error.http_status.is_none() {
            gateway_error.http_status = Some(status);
        }
    }
    gateway_error
}
