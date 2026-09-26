//! Submit browser operations using the existing bounded recovery policy.

use std::time::Duration;

use rquest::{Client, Method};
use serde_json::Value;
use tokio::time::sleep;

use crate::error::{classify_network_error, GatewayError};
use crate::upstream::gemini::canvas_program_web_reverse as program;

use super::super::browser_operation::build_browser_operation_invocation_input_from_values;
use super::super::result::{browser_request_retry_delay_ms, parse_browser_invocation_response};

pub async fn execute_browser_request(
    http: &Client,
    timeout: Duration,
    provider: &str,
    browser_pool_base_url: &str,
    base_url: &str,
    share_id: &str,
    runtime_state_object_key: &str,
    browser_cdp_url: Option<&str>,
    cookie_header: Option<&str>,
    operation: &str,
    prompt: &str,
    locale: &str,
) -> Result<program::GeminiCanvasBrowserInvocationResult, GatewayError> {
    let input = build_browser_operation_invocation_input_from_values(
        base_url,
        share_id,
        runtime_state_object_key,
        browser_cdp_url,
        cookie_header,
        operation,
        prompt,
        locale,
        timeout,
    );
    execute_browser_request_input(
        http,
        timeout,
        provider,
        browser_pool_base_url,
        &input,
        operation,
    )
    .await
}

async fn execute_browser_request_input(
    http: &Client,
    timeout: Duration,
    provider: &str,
    browser_pool_base_url: &str,
    input: &Value,
    operation: &str,
) -> Result<program::GeminiCanvasBrowserInvocationResult, GatewayError> {
    let response = http
        .request(Method::POST, format!("{}/invoke", browser_pool_base_url))
        .header(rquest::header::CONTENT_TYPE, "application/json")
        .timeout(timeout.max(Duration::from_secs(30)))
        .json(&input)
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;

    let status = response.status().as_u16();
    let body_text = response
        .text()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;
    parse_browser_invocation_response(provider, status, &body_text, operation)
}

pub async fn execute_browser_request_input_with_recovery(
    http: &Client,
    timeout: Duration,
    provider: &str,
    browser_pool_base_url: &str,
    input: &Value,
    operation: &str,
) -> Result<program::GeminiCanvasBrowserInvocationResult, GatewayError> {
    let mut attempts = 0usize;
    loop {
        match execute_browser_request_input(
            http,
            timeout,
            provider,
            browser_pool_base_url,
            input,
            operation,
        )
        .await
        {
            Ok(result) => return Ok(result),
            Err(error) => {
                let Some(delay_ms) = browser_request_retry_delay_ms(&error, attempts) else {
                    return Err(error);
                };
                attempts += 1;
                sleep(Duration::from_millis(delay_ms)).await;
            }
        }
    }
}

pub async fn execute_browser_request_with_recovery(
    http: &Client,
    timeout: Duration,
    provider: &str,
    browser_pool_base_url: &str,
    base_url: &str,
    share_id: &str,
    runtime_state_object_key: &str,
    browser_cdp_url: Option<&str>,
    cookie_header: Option<&str>,
    operation: &str,
    prompt: &str,
    locale: &str,
) -> Result<program::GeminiCanvasBrowserInvocationResult, GatewayError> {
    let mut attempts = 0usize;
    loop {
        match execute_browser_request(
            http,
            timeout,
            provider,
            browser_pool_base_url,
            base_url,
            share_id,
            runtime_state_object_key,
            browser_cdp_url,
            cookie_header,
            operation,
            prompt,
            locale,
        )
        .await
        {
            Ok(result) => return Ok(result),
            Err(error) => {
                let Some(delay_ms) = browser_request_retry_delay_ms(&error, attempts) else {
                    return Err(error);
                };
                attempts += 1;
                sleep(Duration::from_millis(delay_ms)).await;
            }
        }
    }
}
