use rquest::header::HeaderMap;
use rquest::{Client, Method};
#[cfg(test)]
use serde_json::json;
use serde_json::Value;
use std::time::Duration;
use tokio::time::{sleep, timeout};

use crate::error::{classify_network_error, GatewayError};
use crate::protocol::upstream_body::collect_bounded_upstream_text_with_provider as read_body;
use crate::upstream::header_map_helpers::extract_bearer_token;

pub(crate) const PRODUCER_IMAGE_MAX_ATTEMPTS: usize = 2;
const PRODUCER_MUSIC_CLIP_POLL_INTERVAL: Duration = Duration::from_secs(5);
const PRODUCER_MUSIC_CLIP_POLL_MAX_TIMEOUT: Duration = Duration::from_secs(300);

#[path = "producer_music_clips.rs"]
mod music_clips;
use music_clips::poll_producer_music_clip_assets;
#[path = "producer_browser_execution.rs"]
mod browser_execution;
#[cfg(test)]
use browser_execution::*;
pub(crate) use browser_execution::{
    build_producer_browser_executor_payload_from_prepared, execute_producer_browser_worker,
    prepare_producer_browser_execution_input, prepare_producer_browser_executor_service_input,
};

#[path = "producer_http_response.rs"]
mod http_response;
#[cfg(test)]
use http_response::*;
use http_response::{
    ensure_successful_producer_http_status, ensure_successful_producer_message_stream_status,
    finalize_producer_image_retry_error, parse_producer_music_stream_http_response,
    parse_producer_send_message_job, parse_producer_video_status_http_response,
    producer_invalid_conversation_response_error, producer_runtime_headers,
    producer_stream_headers, resolve_producer_image_attempt, ProducerImageAttemptResolution,
};

#[path = "producer_video_contract.rs"]
mod video_contract;
pub(crate) use video_contract::should_fallback_producer_video_http_error;
#[cfg(test)]
use video_contract::*;
use video_contract::{
    build_producer_video_bootstrap_plan, build_producer_video_http_response,
    ensure_producer_video_proposal_seen, ensure_successful_producer_video_final_status,
    extract_producer_video_final_status, resolve_producer_video_confirmation_prompt,
    resolve_producer_video_create_job_id_from_summaries, resolve_producer_video_session_plan,
    select_producer_video_final_url,
};

#[cfg(test)]
pub(crate) use browser_execution::parse_producer_browser_worker_verified_output;

#[cfg(test)]
#[path = "producer_async_contract_tests.rs"]
mod async_contract_tests;
#[cfg(test)]
#[path = "producer_response_policy_tests.rs"]
mod response_policy_tests;

#[derive(Debug)]
pub(crate) struct ProducerConversationJobData {
    pub(crate) job_id: String,
    #[allow(dead_code)]
    pub(crate) body: Value,
}

pub(crate) fn parse_producer_conversation_job(
    body_text: &str,
    provider: &str,
) -> Result<ProducerConversationJobData, GatewayError> {
    let body = serde_json::from_str::<Value>(body_text)
        .map_err(|_| producer_invalid_conversation_response_error(provider))?;
    let job_id = crate::protocol::producer::extract_job_id(&body)?;
    Ok(ProducerConversationJobData { job_id, body })
}

pub(crate) fn parse_producer_conversation_http_job(
    status: u16,
    body_text: &str,
    provider: &str,
) -> Result<ProducerConversationJobData, GatewayError> {
    ensure_successful_producer_http_status(status, body_text, provider, None)?;
    parse_producer_conversation_job(body_text, provider)
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn send_producer_conversation(
    http: &Client,
    base_url: &str,
    headers: &HeaderMap,
    prompt: &str,
    conversation_id: Option<&str>,
    client_context: &Value,
    model_name: &str,
    referer: &str,
    request_timeout: Duration,
) -> Result<ProducerConversationJobData, GatewayError> {
    let provider = "producer_compatible";
    let body = crate::protocol::producer::build_conversation_request_body(
        prompt,
        conversation_id,
        client_context,
        model_name,
    );
    let response = http
        .request(
            Method::POST,
            crate::protocol::producer::build_conversation_url(base_url),
        )
        .headers(producer_runtime_headers(headers, referer))
        .json(&body)
        .timeout(request_timeout)
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;
    let status = response.status().as_u16();
    let body_text = read_body(response, "Producer.ai conversation response", provider).await?;
    parse_producer_conversation_http_job(status, &body_text, provider)
}

pub(crate) async fn read_producer_message_stream(
    http: &Client,
    base_url: &str,
    headers: &HeaderMap,
    job_id: &str,
    referer: &str,
    request_timeout: Duration,
) -> Result<String, GatewayError> {
    let provider = "producer_compatible";
    let response = http
        .request(
            Method::GET,
            crate::protocol::producer::build_message_stream_url(base_url, job_id),
        )
        .headers(producer_stream_headers(headers, referer))
        .query(&[("last_id", "0")])
        .timeout(request_timeout)
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;
    let status = response.status().as_u16();
    let body_text = read_body(response, "Producer.ai message SSE body", provider).await?;
    ensure_successful_producer_message_stream_status(status, &body_text, provider)?;
    Ok(body_text)
}

pub(crate) async fn fetch_producer_video_status(
    http: &Client,
    base_url: &str,
    headers: &HeaderMap,
    job_id: &str,
    referer: &str,
    request_timeout: Duration,
) -> Result<Value, GatewayError> {
    let provider = "producer_compatible";
    let response = http
        .request(
            Method::GET,
            crate::protocol::producer::build_video_status_url(base_url, job_id),
        )
        .headers(producer_runtime_headers(headers, referer))
        .timeout(request_timeout.min(Duration::from_secs(60)))
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;
    let status = response.status().as_u16();
    let body_text = read_body(response, "Producer.ai video status response", provider).await?;
    parse_producer_video_status_http_response(status, &body_text, provider)
}

async fn poll_producer_video_status_until_complete(
    http: &Client,
    base_url: &str,
    headers: &HeaderMap,
    job_id: &str,
    referer: &str,
    request_timeout: Duration,
) -> Result<Value, GatewayError> {
    let provider = "producer_compatible";
    // One budget owns requests, body reads and sleeps; expiry drops the in-flight I/O.
    timeout(request_timeout, async {
        loop {
            let status_payload = fetch_producer_video_status(
                http,
                base_url,
                headers,
                job_id,
                referer,
                // Only the outer timer owns the shorter total budget, avoiding a transport race.
                Duration::from_secs(60),
            )
            .await?;
            let final_status = extract_producer_video_final_status(&status_payload);
            ensure_successful_producer_video_final_status(
                provider,
                &final_status,
                &status_payload,
            )?;
            if final_status == "completed"
                && select_producer_video_final_url(&status_payload, job_id).is_some()
            {
                return Ok(status_payload);
            }
            sleep(PRODUCER_MUSIC_CLIP_POLL_INTERVAL).await;
        }
    })
    .await
    .map_err(|_| {
        GatewayError::service_unavailable(
            "Producer music-video media did not become available before the poll timeout.",
        )
        .with_provider(provider)
        .with_code("producer_video_poll_timeout")
    })?
}

pub(crate) async fn execute_producer_image_http(
    http: &Client,
    base_url: &str,
    headers: &HeaderMap,
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    model: &str,
    request_timeout: Duration,
) -> Result<Value, GatewayError> {
    let provider = "producer_compatible";
    let auth_token = extract_bearer_token(headers);
    let request_body = crate::protocol::producer::build_image_generation_request(req, model)?;
    let mut last_error: Option<GatewayError> = None;

    for attempt in 0..PRODUCER_IMAGE_MAX_ATTEMPTS {
        let response = http
            .request(
                Method::POST,
                crate::protocol::producer::build_image_generation_url(base_url),
            )
            .headers(headers.clone())
            .json(&request_body)
            .timeout(request_timeout)
            .send()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;
        let status = response.status().as_u16();
        let body_text = read_body(response, "Producer.ai image response", provider).await?;

        match resolve_producer_image_attempt(
            status,
            &body_text,
            req,
            model,
            auth_token.as_deref(),
            provider,
            attempt + 1 < PRODUCER_IMAGE_MAX_ATTEMPTS,
        ) {
            ProducerImageAttemptResolution::Success(body) => return Ok(body),
            ProducerImageAttemptResolution::Retry(gateway_error) => {
                last_error = Some(gateway_error);
                sleep(Duration::from_secs(2)).await;
            }
            ProducerImageAttemptResolution::Fail(gateway_error) => {
                return Err(gateway_error);
            }
        }
    }

    Err(finalize_producer_image_retry_error(last_error, provider))
}

pub(crate) async fn execute_producer_music_http(
    http: &Client,
    base_url: &str,
    headers: &HeaderMap,
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    model: &str,
    request_timeout: Duration,
) -> Result<Value, GatewayError> {
    let provider = "producer_compatible";
    let send_body = crate::protocol::producer::build_send_message_request(req, model)?;
    let send_response = http
        .request(
            Method::POST,
            crate::protocol::producer::build_conversation_url(base_url),
        )
        .headers(headers.clone())
        .json(&send_body)
        .timeout(request_timeout)
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;
    let send_status = send_response.status().as_u16();
    let send_body_text = read_body(
        send_response,
        "Producer.ai music send-message response",
        provider,
    )
    .await?;
    let job_id = parse_producer_send_message_job(send_status, &send_body_text, provider)?;

    let stream_response = http
        .request(
            Method::GET,
            crate::protocol::producer::build_message_stream_url(base_url, &job_id),
        )
        .headers(headers.clone())
        .query(&[("last_id", "0")])
        .timeout(request_timeout)
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;
    let stream_status = stream_response.status().as_u16();
    let stream_body = read_body(stream_response, "Producer.ai music SSE body", provider).await?;
    let stream_summary = parse_producer_music_stream_http_response(
        stream_status,
        &stream_body,
        provider,
        model,
        &job_id,
    )?;
    poll_producer_music_clip_assets(
        http,
        base_url,
        headers,
        stream_summary,
        request_timeout.min(PRODUCER_MUSIC_CLIP_POLL_MAX_TIMEOUT),
    )
    .await
}

pub(crate) async fn execute_producer_video_http(
    http: &Client,
    base_url: &str,
    headers: &HeaderMap,
    request_body: &Value,
    model: &str,
    request_timeout: Duration,
) -> Result<Value, GatewayError> {
    let provider = "producer_compatible";
    let bootstrap_plan = build_producer_video_bootstrap_plan(base_url, request_body)?;
    let bootstrap_response = send_producer_conversation(
        http,
        base_url,
        headers,
        &bootstrap_plan.bootstrap_prompt,
        None,
        &bootstrap_plan.client_context,
        crate::protocol::producer::PRODUCER_DEFAULT_MODEL,
        &bootstrap_plan.bootstrap_referer,
        request_timeout,
    )
    .await
    .map_err(|error| error.with_code("producer_http_video_bootstrap_failed"))?;
    let bootstrap_stream = read_producer_message_stream(
        http,
        base_url,
        headers,
        &bootstrap_response.job_id,
        &bootstrap_plan.bootstrap_referer,
        request_timeout,
    )
    .await
    .map_err(|error| error.with_code("producer_http_video_bootstrap_stream_failed"))?;
    let bootstrap_summary =
        crate::protocol::producer::summarize_conversation_stream_text(&bootstrap_stream)?;
    let session_plan = resolve_producer_video_session_plan(base_url, &bootstrap_summary, provider)?;

    let proposal_prompt = crate::protocol::producer::build_video_proposal_prompt(request_body)?;
    let creative_response = send_producer_conversation(
        http,
        base_url,
        headers,
        &proposal_prompt,
        Some(&session_plan.conversation_id),
        &bootstrap_plan.client_context,
        crate::protocol::producer::PRODUCER_DEFAULT_MODEL,
        &session_plan.session_referer,
        request_timeout,
    )
    .await
    .map_err(|error| error.with_code("producer_http_video_creative_failed"))?;
    let creative_stream = read_producer_message_stream(
        http,
        base_url,
        headers,
        &creative_response.job_id,
        &session_plan.session_referer,
        request_timeout,
    )
    .await
    .map_err(|error| error.with_code("producer_http_video_creative_stream_failed"))?;
    let creative_summary =
        crate::protocol::producer::summarize_conversation_stream_text(&creative_stream)?;
    ensure_producer_video_proposal_seen(&creative_summary, provider)?;
    let mut confirmation_job_id: Option<String> = None;
    let mut confirmation_summary: Option<Value> = None;

    if let Some(confirm_prompt) =
        resolve_producer_video_confirmation_prompt(request_body, &creative_summary)?
    {
        let confirm_response = send_producer_conversation(
            http,
            base_url,
            headers,
            &confirm_prompt,
            Some(&session_plan.conversation_id),
            &bootstrap_plan.client_context,
            crate::protocol::producer::PRODUCER_DEFAULT_MODEL,
            &session_plan.session_referer,
            request_timeout,
        )
        .await
        .map_err(|error| error.with_code("producer_http_video_confirm_failed"))?;
        let confirm_stream = read_producer_message_stream(
            http,
            base_url,
            headers,
            &confirm_response.job_id,
            &session_plan.session_referer,
            request_timeout,
        )
        .await
        .map_err(|error| error.with_code("producer_http_video_confirm_stream_failed"))?;
        let confirm_summary =
            crate::protocol::producer::summarize_conversation_stream_text(&confirm_stream)?;
        confirmation_job_id = Some(confirm_response.job_id.clone());
        confirmation_summary = Some(confirm_summary);
    }

    let video_job_id = resolve_producer_video_create_job_id_from_summaries(
        &creative_summary,
        confirmation_summary.as_ref(),
        provider,
    )?;

    let accept_async_job = request_body
        .get("async")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let status_payload = if accept_async_job {
        fetch_producer_video_status(
            http,
            base_url,
            headers,
            &video_job_id,
            &session_plan.session_referer,
            request_timeout,
        )
        .await?
    } else {
        poll_producer_video_status_until_complete(
            http,
            base_url,
            headers,
            &video_job_id,
            &session_plan.session_referer,
            request_timeout,
        )
        .await?
    };
    build_producer_video_http_response(
        provider,
        base_url,
        request_body,
        model,
        &bootstrap_plan.clip_id,
        &session_plan.conversation_id,
        &bootstrap_response.job_id,
        &creative_response.job_id,
        confirmation_job_id.as_deref(),
        &video_job_id,
        &status_payload,
        &creative_summary,
        confirmation_summary.as_ref(),
    )
}

#[cfg(test)]
#[path = "producer_browser_outcome_tests.rs"]
mod browser_outcome_tests;
#[cfg(test)]
#[path = "producer_browser_request_tests.rs"]
mod browser_request_tests;
#[cfg(test)]
#[path = "producer_http_response_tests.rs"]
mod http_response_tests;
#[cfg(test)]
#[path = "producer_video_completion_tests.rs"]
mod video_completion_tests;
#[cfg(test)]
#[path = "producer_video_session_tests.rs"]
mod video_session_tests;

#[cfg(test)]
#[path = "producer_video_hardening_tests.rs"]
mod video_hardening_tests;
