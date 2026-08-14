use rquest::header::HeaderMap;
use rquest::{Client, Method};
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::{sleep, Instant};

use crate::error::{classify_network_error, classify_upstream_error, GatewayError};
use crate::upstream::browser_executor_helpers::{
    build_browser_executor_header_map, missing_browser_executor_field_error, read_json_u64,
};
use crate::upstream::browser_worker_runtime_helpers::{
    enrich_browser_worker_message, producer_browser_worker_script_path,
};
use crate::upstream::browser_worker_types::{
    ProducerBrowserWorkerInput, ProducerBrowserWorkerResult,
};
use crate::upstream::header_map_helpers::{
    extract_bearer_token, header_map_string, insert_runtime_header,
};

pub(crate) const PRODUCER_IMAGE_MAX_ATTEMPTS: usize = 2;
const PRODUCER_MUSIC_CLIP_POLL_INTERVAL: Duration = Duration::from_secs(5);
const PRODUCER_MUSIC_CLIP_POLL_MAX_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Debug)]
pub(crate) struct ProducerConversationJobData {
    pub(crate) job_id: String,
    #[allow(dead_code)]
    pub(crate) body: Value,
}

#[derive(Debug)]
pub(crate) struct PreparedProducerBrowserExecutorServiceInput {
    pub(crate) base_url: String,
    pub(crate) headers: HeaderMap,
    pub(crate) request_body: Value,
    pub(crate) model: String,
    pub(crate) timeout: Duration,
}

#[derive(Debug)]
pub(crate) struct PreparedProducerBrowserWorkerLaunch {
    pub(crate) node_bin: String,
    pub(crate) script_path: PathBuf,
    pub(crate) stdin_json: Vec<u8>,
}

#[derive(Debug)]
pub(crate) struct ProducerVideoBootstrapPlan {
    pub(crate) clip_id: String,
    pub(crate) client_context: Value,
    pub(crate) bootstrap_prompt: String,
    pub(crate) bootstrap_referer: String,
}

#[derive(Debug)]
pub(crate) struct ProducerVideoSessionPlan {
    pub(crate) conversation_id: String,
    pub(crate) session_referer: String,
}

#[derive(Debug)]
pub(crate) enum ProducerImageAttemptResolution {
    Success(Value),
    Retry(GatewayError),
    Fail(GatewayError),
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
    let body_text = response
        .text()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;
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
    let body_text = response
        .text()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;
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
    let body_text = response
        .text()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;
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
    let deadline = Instant::now() + request_timeout;
    loop {
        let status_payload =
            fetch_producer_video_status(http, base_url, headers, job_id, referer, request_timeout)
                .await?;
        let final_status = extract_producer_video_final_status(&status_payload);
        ensure_successful_producer_video_final_status(provider, &final_status, &status_payload)?;
        if final_status == "completed"
            && select_producer_video_final_url(&status_payload, job_id).is_some()
        {
            return Ok(status_payload);
        }
        if Instant::now() >= deadline {
            return Err(GatewayError::service_unavailable(
                "Producer music-video media did not become available before the poll timeout.",
            )
            .with_provider(provider)
            .with_code("producer_video_poll_timeout"));
        }
        sleep(PRODUCER_MUSIC_CLIP_POLL_INTERVAL).await;
    }
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
        let body_text = response
            .text()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;

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
    let send_body_text = send_response
        .text()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;
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
    let stream_body = stream_response
        .text()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;
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

async fn fetch_producer_music_library(
    http: &Client,
    base_url: &str,
    headers: &HeaderMap,
    request_timeout: Duration,
) -> Result<Value, GatewayError> {
    let provider = "producer_compatible";
    let response = http
        .request(
            Method::GET,
            crate::protocol::producer::build_clips_library_url(base_url),
        )
        .headers(headers.clone())
        .timeout(request_timeout.min(Duration::from_secs(60)))
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;
    let status = response.status().as_u16();
    let body_text = response
        .text()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;
    ensure_successful_producer_http_status(
        status,
        &body_text,
        provider,
        Some("producer_music_clip_poll_failed"),
    )?;
    serde_json::from_str(&body_text).map_err(|_| {
        GatewayError::server_error("Producer music clip library response was not valid JSON.")
            .with_provider(provider)
            .with_code("producer_music_clip_poll_invalid_response")
    })
}

async fn poll_producer_music_clip_assets(
    http: &Client,
    base_url: &str,
    headers: &HeaderMap,
    stream_summary: Value,
    poll_timeout: Duration,
) -> Result<Value, GatewayError> {
    let provider = "producer_compatible";
    let clip_ids = producer_music_clip_ids(&stream_summary);
    if clip_ids.is_empty() {
        return Err(GatewayError::server_error(
            "Producer music stream completed without returning a clip ID.",
        )
        .with_provider(provider)
        .with_code("producer_music_missing_clip_id"));
    }

    let deadline = Instant::now() + poll_timeout;
    loop {
        let library = fetch_producer_music_library(http, base_url, headers, poll_timeout).await?;
        let clips = producer_music_clip_assets(&library, &clip_ids);
        if producer_music_clip_assets_complete(&clips, &clip_ids) {
            return build_producer_music_completed_response(stream_summary, clips);
        }
        if Instant::now() >= deadline {
            break;
        }
        sleep(PRODUCER_MUSIC_CLIP_POLL_INTERVAL).await;
    }

    Err(GatewayError::service_unavailable(
        "Producer music clip media did not become available before the poll timeout.",
    )
    .with_provider(provider)
    .with_code("producer_music_clip_poll_timeout"))
}

fn producer_music_clip_ids(value: &Value) -> Vec<String> {
    fn visit(value: &Value, output: &mut Vec<String>) {
        match value {
            Value::Array(values) => {
                for value in values {
                    visit(value, output);
                }
            }
            Value::Object(object) => {
                for (key, value) in object {
                    if matches!(key.as_str(), "clip_id" | "clipId" | "song_id" | "songId") {
                        if let Some(value) = value
                            .as_str()
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                        {
                            if !output.iter().any(|entry| entry == value) {
                                output.push(value.to_string());
                            }
                        }
                    }
                    visit(value, output);
                }
            }
            _ => {}
        }
    }

    let mut output = Vec::new();
    visit(value, &mut output);
    output
}

fn producer_music_clip_assets(library: &Value, clip_ids: &[String]) -> Vec<Value> {
    fn visit(
        value: &Value,
        targets: &HashSet<&str>,
        seen: &mut HashSet<String>,
        output: &mut Vec<Value>,
    ) {
        match value {
            Value::Array(values) => {
                for value in values {
                    visit(value, targets, seen, output);
                }
            }
            Value::Object(object) => {
                let candidate_id = ["id", "clip_id", "clipId", "song_id", "songId"]
                    .iter()
                    .find_map(|key| object.get(*key).and_then(Value::as_str))
                    .map(str::trim)
                    .filter(|value| !value.is_empty());
                if let Some(candidate_id) = candidate_id {
                    if targets.contains(candidate_id) && seen.insert(candidate_id.to_string()) {
                        let mut clip = Map::new();
                        clip.insert(
                            "clip_id".to_string(),
                            Value::String(candidate_id.to_string()),
                        );
                        for field in [
                            "title",
                            "duration",
                            "audio_url",
                            "wav_url",
                            "image_url",
                            "video_url",
                            "status",
                            "state",
                        ] {
                            if let Some(value) = object.get(field) {
                                clip.insert(field.to_string(), value.clone());
                            }
                        }
                        output.push(Value::Object(clip));
                    }
                }
                for value in object.values() {
                    visit(value, targets, seen, output);
                }
            }
            _ => {}
        }
    }

    let targets = clip_ids.iter().map(String::as_str).collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    let mut output = Vec::new();
    visit(library, &targets, &mut seen, &mut output);
    output
}

fn producer_music_clip_assets_complete(clips: &[Value], clip_ids: &[String]) -> bool {
    clip_ids.iter().all(|clip_id| {
        clips.iter().any(|clip| {
            clip.get("clip_id").and_then(Value::as_str) == Some(clip_id.as_str())
                && ["audio_url", "wav_url"].iter().any(|field| {
                    clip.get(*field)
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .is_some_and(|value| !value.is_empty())
                })
        })
    })
}

fn build_producer_music_completed_response(
    mut stream_summary: Value,
    clips: Vec<Value>,
) -> Result<Value, GatewayError> {
    let provider = "producer_compatible";
    let object = stream_summary.as_object_mut().ok_or_else(|| {
        GatewayError::server_error("Producer music stream summary must be a JSON object.")
            .with_provider(provider)
            .with_code("producer_music_invalid_stream_summary")
    })?;
    let data = clips
        .iter()
        .filter_map(|clip| {
            let clip_id = clip.get("clip_id")?.as_str()?;
            let url = clip
                .get("audio_url")
                .or_else(|| clip.get("wav_url"))?
                .as_str()?;
            let mut item = Map::new();
            item.insert("clip_id".to_string(), Value::String(clip_id.to_string()));
            item.insert("url".to_string(), Value::String(url.to_string()));
            for field in ["wav_url", "image_url", "title", "duration"] {
                if let Some(value) = clip.get(field) {
                    item.insert(field.to_string(), value.clone());
                }
            }
            Some(Value::Object(item))
        })
        .collect::<Vec<_>>();
    object.insert("clips".to_string(), Value::Array(clips));
    object.insert("data".to_string(), Value::Array(data));
    object.insert("completed".to_string(), Value::Bool(true));
    object.insert("media_completed".to_string(), Value::Bool(true));
    Ok(stream_summary)
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

pub(crate) fn extract_producer_send_message_job_id(
    status: u16,
    body: &Value,
    provider: &str,
) -> Result<String, GatewayError> {
    if !(200..300).contains(&status) {
        return Err(classify_upstream_error(
            status,
            &body.to_string(),
            Some(provider),
        ));
    }
    crate::protocol::producer::extract_job_id(body)
}

pub(crate) fn parse_producer_send_message_job(
    status: u16,
    body_text: &str,
    provider: &str,
) -> Result<String, GatewayError> {
    let body = serde_json::from_str::<Value>(body_text)
        .map_err(|_| producer_invalid_conversation_response_error(provider))?;
    extract_producer_send_message_job_id(status, &body, provider)
}

pub(crate) fn parse_producer_video_status_response(
    body_text: &str,
    provider: &str,
) -> Result<Value, GatewayError> {
    serde_json::from_str::<Value>(body_text)
        .map_err(|_| producer_invalid_video_status_response_error(provider))
}

pub(crate) fn parse_producer_video_status_http_response(
    status: u16,
    body_text: &str,
    provider: &str,
) -> Result<Value, GatewayError> {
    ensure_successful_producer_http_status(
        status,
        body_text,
        provider,
        Some("producer_http_video_status_failed"),
    )?;
    parse_producer_video_status_response(body_text, provider)
}

pub(crate) fn parse_producer_image_generation_response(
    body_text: &str,
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    model: &str,
    auth_token: Option<&str>,
    provider: &str,
) -> Result<Value, GatewayError> {
    let body = serde_json::from_str::<Value>(body_text)
        .map_err(|_| producer_invalid_image_response_error(provider))?;
    crate::protocol::producer::build_image_generation_response(&body, req, model, auth_token)
}

pub(crate) fn resolve_producer_image_attempt(
    status: u16,
    body_text: &str,
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    model: &str,
    auth_token: Option<&str>,
    provider: &str,
    can_retry: bool,
) -> ProducerImageAttemptResolution {
    if (200..300).contains(&status) {
        return match parse_producer_image_generation_response(
            body_text, req, model, auth_token, provider,
        ) {
            Ok(body) => ProducerImageAttemptResolution::Success(body),
            Err(error) => ProducerImageAttemptResolution::Fail(error),
        };
    }

    let gateway_error = classify_producer_image_request_failure(status, body_text);
    if can_retry && should_retry_producer_image_request(status, body_text) {
        ProducerImageAttemptResolution::Retry(gateway_error)
    } else {
        ProducerImageAttemptResolution::Fail(gateway_error)
    }
}

pub(crate) fn parse_producer_music_stream_http_response(
    status: u16,
    body_text: &str,
    provider: &str,
    model: &str,
    job_id: &str,
) -> Result<Value, GatewayError> {
    ensure_successful_producer_message_stream_status(status, body_text, provider)?;
    crate::protocol::producer::parse_producer_music_stream_text(body_text, model, job_id)
}

pub(crate) fn ensure_successful_producer_http_status(
    status: u16,
    body_text: &str,
    provider: &str,
    error_code: Option<&str>,
) -> Result<(), GatewayError> {
    if (200..300).contains(&status) {
        return Ok(());
    }
    let error = classify_upstream_error(status, body_text, Some(provider));
    if let Some(error_code) = error_code {
        Err(error.with_code(error_code))
    } else {
        Err(error)
    }
}

pub(crate) fn ensure_successful_producer_message_stream_status(
    status: u16,
    body_text: &str,
    provider: &str,
) -> Result<(), GatewayError> {
    ensure_successful_producer_http_status(status, body_text, provider, None)
}

pub(crate) fn producer_runtime_headers(
    base_headers: &HeaderMap,
    referer: &str,
) -> rquest::header::HeaderMap {
    let mut headers = base_headers.clone();
    if !headers.contains_key(rquest::header::ACCEPT) {
        insert_runtime_header(&mut headers, "accept", "application/json, text/plain, */*");
    }
    if !headers.contains_key(rquest::header::ORIGIN) {
        insert_runtime_header(&mut headers, "origin", "https://www.flowmusic.app");
    }
    insert_runtime_header(&mut headers, "referer", referer);
    headers
}

pub(crate) fn producer_stream_headers(
    base_headers: &HeaderMap,
    referer: &str,
) -> rquest::header::HeaderMap {
    let mut headers = producer_runtime_headers(base_headers, referer);
    insert_runtime_header(&mut headers, "accept", "text/event-stream");
    headers
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

pub(crate) fn build_producer_video_bootstrap_plan(
    base_url: &str,
    request_body: &Value,
) -> Result<ProducerVideoBootstrapPlan, GatewayError> {
    let clip_id = crate::protocol::producer::extract_video_clip_id(request_body)?;
    let client_context = crate::protocol::producer::build_video_client_context(
        request_body,
        crate::protocol::producer::PRODUCER_DEFAULT_MODEL,
    )?;
    Ok(ProducerVideoBootstrapPlan {
        clip_id: clip_id.clone(),
        client_context,
        bootstrap_prompt: crate::protocol::producer::build_video_bootstrap_prompt(
            base_url, &clip_id,
        ),
        bootstrap_referer: format!("{}/", base_url.trim_end_matches('/')),
    })
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

    let mut child = Command::new(&launch.node_bin)
        .arg(&launch.script_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            crate::protocol::producer::browser_worker_spawn_failed_error(
                launch.script_path.as_path(),
                error.to_string().as_str(),
            )
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(&launch.stdin_json).await.map_err(|error| {
            crate::protocol::producer::browser_worker_stdin_error(error.to_string().as_str())
        })?;
    }

    let output = tokio::time::timeout(prepared.timeout, child.wait_with_output())
        .await
        .map_err(|_| crate::protocol::producer::browser_worker_timeout_error())?
        .map_err(|error| {
            crate::protocol::producer::browser_worker_wait_failed_error(error.to_string().as_str())
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    parse_producer_browser_worker_verified_output(&stdout, &stderr, provider)
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
    let mut gateway_error = classify_upstream_error(status, body_text, Some(provider));
    if let Some(worker_error) = result.error {
        if let Some(code) = worker_error.code {
            gateway_error.code = Some(code);
        }
        if let Some(message) = worker_error.message {
            gateway_error.message =
                enrich_browser_worker_message(message, worker_error.body.clone());
        }
        if gateway_error.http_status.is_none() {
            gateway_error.http_status = Some(status);
        }
    }
    gateway_error
}

pub(crate) fn producer_summary_has_tool_name(
    summary: &Value,
    field: &str,
    tool_name: &str,
) -> bool {
    summary
        .get(field)
        .and_then(|value| value.as_array())
        .map(|entries| {
            entries.iter().any(|entry| {
                entry
                    .get("tool_name")
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    == Some(tool_name)
            })
        })
        .unwrap_or(false)
}

pub(crate) fn producer_summary_find_tool_return_job_id(
    summary: &Value,
    tool_name: &str,
) -> Option<String> {
    summary
        .get("tool_returns")
        .and_then(|value| value.as_array())
        .and_then(|entries| {
            entries.iter().find_map(|entry| {
                if entry
                    .get("tool_name")
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    != Some(tool_name)
                {
                    return None;
                }
                entry.get("content").and_then(|content| {
                    content
                        .get("job_id")
                        .or_else(|| content.get("jobId"))
                        .and_then(|value| value.as_str())
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_string)
                })
            })
        })
}

pub(crate) fn extract_producer_conversation_id(summary: &Value) -> Option<String> {
    summary
        .get("conversation_id")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) fn require_producer_conversation_id(
    summary: &Value,
    provider: &str,
) -> Result<String, GatewayError> {
    extract_producer_conversation_id(summary)
        .ok_or_else(|| producer_http_video_missing_conversation_id_error(provider))
}

pub(crate) fn resolve_producer_video_session_plan(
    base_url: &str,
    summary: &Value,
    provider: &str,
) -> Result<ProducerVideoSessionPlan, GatewayError> {
    let conversation_id = require_producer_conversation_id(summary, provider)?;
    Ok(ProducerVideoSessionPlan {
        session_referer: crate::protocol::producer::build_session_url(base_url, &conversation_id),
        conversation_id,
    })
}

pub(crate) fn resolve_producer_video_confirmation_prompt(
    request_body: &Value,
    creative_summary: &Value,
) -> Result<Option<String>, GatewayError> {
    if extract_producer_video_create_job_id(creative_summary).is_some() {
        return Ok(None);
    }

    crate::protocol::producer::choose_video_confirm_prompt(request_body, creative_summary).map(Some)
}

pub(crate) fn producer_summary_has_video_proposal(summary: &Value) -> bool {
    producer_summary_has_tool_name(summary, "tool_calls", "video__propose_music_video")
        || producer_summary_has_tool_name(summary, "tool_returns", "video__propose_music_video")
}

pub(crate) fn ensure_producer_video_proposal_seen(
    summary: &Value,
    provider: &str,
) -> Result<(), GatewayError> {
    if producer_summary_has_video_proposal(summary) {
        return Ok(());
    }

    Err(producer_http_video_missing_video_proposal_error(provider))
}

pub(crate) fn extract_producer_video_create_job_id(summary: &Value) -> Option<String> {
    producer_summary_find_tool_return_job_id(summary, "video__create_music_video")
}

pub(crate) fn require_producer_video_create_job_id(
    create_job_id: Option<String>,
    provider: &str,
) -> Result<String, GatewayError> {
    create_job_id.ok_or_else(|| producer_http_video_missing_video_job_id_error(provider))
}

pub(crate) fn resolve_producer_video_create_job_id_from_summaries(
    creative_summary: &Value,
    confirmation_summary: Option<&Value>,
    provider: &str,
) -> Result<String, GatewayError> {
    ensure_producer_video_proposal_seen(creative_summary, provider)?;
    let create_job_id = extract_producer_video_create_job_id(creative_summary)
        .or_else(|| confirmation_summary.and_then(extract_producer_video_create_job_id));
    require_producer_video_create_job_id(create_job_id, provider)
}

pub(crate) fn select_producer_video_final_url(
    status_payload: &Value,
    video_job_id: &str,
) -> Option<String> {
    let urls = producer_collect_media_urls(status_payload);
    urls.iter()
        .find(|entry| entry.contains(&format!("/music-video/{video_job_id}/")))
        .cloned()
        .or_else(|| {
            urls.into_iter()
                .find(|entry| entry.contains("/music-video/"))
        })
}

pub(crate) fn extract_producer_video_final_status(status_payload: &Value) -> String {
    status_payload
        .get("status")
        .and_then(|value| value.as_str())
        .or_else(|| {
            status_payload
                .get("state")
                .and_then(|value| value.get("status"))
                .and_then(|value| value.as_str())
        })
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_else(|| "accepted".to_string())
}

pub(crate) fn producer_collect_media_urls(value: &Value) -> Vec<String> {
    let mut urls = Vec::new();
    collect_producer_media_urls(value, &mut urls);
    urls
}

fn collect_producer_media_urls(value: &Value, urls: &mut Vec<String>) {
    match value {
        Value::String(text) if text.starts_with("http://") || text.starts_with("https://") => {
            if text.contains(".mp4") || text.contains(".mov") || text.contains("/music-video/") {
                urls.push(text.to_string());
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_producer_media_urls(item, urls);
            }
        }
        Value::Object(map) => {
            for item in map.values() {
                collect_producer_media_urls(item, urls);
            }
        }
        _ => {}
    }
}

pub(crate) fn producer_http_video_missing_conversation_id_error(provider: &str) -> GatewayError {
    GatewayError::server_error(
        "Producer video bootstrap completed without returning a conversation_id.",
    )
    .with_provider(provider)
    .with_code("producer_http_video_missing_conversation_id")
}

pub(crate) fn producer_http_video_missing_video_proposal_error(provider: &str) -> GatewayError {
    GatewayError::server_error(
        "Producer video flow did not complete the required video__propose_music_video step before confirmation.",
    )
    .with_provider(provider)
    .with_code("producer_http_video_missing_video_proposal")
}

pub(crate) fn producer_http_video_missing_video_job_id_error(provider: &str) -> GatewayError {
    GatewayError::server_error(
        "Producer conversation completed without returning a music-video job_id.",
    )
    .with_provider(provider)
    .with_code("producer_http_video_missing_video_job_id")
}

pub(crate) fn producer_invalid_video_status_response_error(provider: &str) -> GatewayError {
    GatewayError::server_error("Producer.ai music-video status response was not valid JSON.")
        .with_provider(provider)
        .with_code("producer_invalid_video_status_response")
}

pub(crate) fn producer_invalid_image_response_error(provider: &str) -> GatewayError {
    GatewayError::server_error("Producer.ai image response was not valid JSON.")
        .with_provider(provider)
        .with_code("producer_invalid_image_response")
}

pub(crate) fn producer_image_retry_exhausted_error(provider: &str) -> GatewayError {
    GatewayError::server_error("Producer.ai image request exhausted retry attempts.")
        .with_provider(provider)
        .with_code("producer_image_retry_exhausted")
}

pub(crate) fn classify_producer_image_request_failure(
    status: u16,
    body_text: &str,
) -> GatewayError {
    classify_upstream_error(status, body_text, Some("producer_compatible"))
}

pub(crate) fn finalize_producer_image_retry_error(
    last_error: Option<GatewayError>,
    provider: &str,
) -> GatewayError {
    last_error.unwrap_or_else(|| producer_image_retry_exhausted_error(provider))
}

pub(crate) fn producer_invalid_conversation_response_error(provider: &str) -> GatewayError {
    GatewayError::server_error("Producer.ai conversation response was not valid JSON.")
        .with_provider(provider)
        .with_code("producer_invalid_conversation_response")
}

pub(crate) fn producer_http_video_failed_error(
    provider: &str,
    final_status: &str,
    status_payload: &str,
) -> GatewayError {
    GatewayError::server_error(format!(
        "Producer video job entered terminal status '{final_status}'. status payload: {status_payload}"
    ))
    .with_provider(provider)
    .with_code("producer_http_video_failed")
}

pub(crate) fn ensure_successful_producer_video_final_status(
    provider: &str,
    final_status: &str,
    status_payload: &Value,
) -> Result<(), GatewayError> {
    if matches!(final_status, "failed" | "error" | "cancelled" | "canceled") {
        return Err(producer_http_video_failed_error(
            provider,
            final_status,
            status_payload.to_string().as_str(),
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_producer_video_http_response(
    provider: &str,
    base_url: &str,
    request_body: &Value,
    model: &str,
    clip_id: &str,
    conversation_id: &str,
    bootstrap_job_id: &str,
    creative_job_id: &str,
    confirmation_job_id: Option<&str>,
    video_job_id: &str,
    status_payload: &Value,
    creative_summary: &Value,
    confirmation_summary: Option<&Value>,
) -> Result<Value, GatewayError> {
    let final_status = extract_producer_video_final_status(status_payload);
    ensure_successful_producer_video_final_status(provider, &final_status, status_payload)?;
    let final_url = select_producer_video_final_url(status_payload, video_job_id);
    Ok(crate::protocol::producer::build_video_generation_response(
        base_url,
        request_body,
        model,
        clip_id,
        conversation_id,
        bootstrap_job_id,
        creative_job_id,
        confirmation_job_id,
        video_job_id,
        final_url.as_deref(),
        &final_status,
        status_payload,
        creative_summary,
        confirmation_summary,
    ))
}

pub(crate) fn should_fallback_producer_video_http_error(error: &GatewayError) -> bool {
    if matches!(error.http_status, Some(401 | 403)) {
        return false;
    }
    let Some(code) = error.code.as_deref() else {
        return matches!(
            error.http_status,
            Some(404 | 408 | 429 | 500 | 502 | 503 | 504)
        );
    };
    if code == "producer_http_video_failed" {
        return false;
    }
    code.starts_with("producer_http_video_")
        || matches!(
            error.http_status,
            Some(404 | 408 | 429 | 500 | 502 | 503 | 504)
        )
}

pub(crate) fn should_retry_producer_image_request(status: u16, body_text: &str) -> bool {
    if matches!(status, 502 | 503 | 504) {
        return true;
    }
    let normalized = body_text.to_ascii_lowercase();
    normalized.contains("gateway time-out")
        || normalized.contains("error code 504")
        || normalized.contains("temporarily unavailable")
}

#[cfg(test)]
mod tests {
    use rquest::header::HeaderMap;
    use serde_json::{json, Value};

    use crate::error::GatewayError;
    use crate::protocol::canonical::{
        CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole,
        ProtocolFamily,
    };
    use crate::protocol::producer::{
        normalize_image_generations, PRODUCER_DEFAULT_MODEL, PRODUCER_IMAGE_DEFAULT_MODEL,
    };
    use std::collections::HashMap;
    use std::future::Future;

    use super::{
        build_producer_browser_executor_payload,
        build_producer_browser_executor_payload_from_prepared, build_producer_browser_worker_input,
        build_producer_music_completed_response, build_producer_video_bootstrap_plan,
        build_producer_video_http_response, classify_producer_browser_worker_failure,
        classify_producer_image_request_failure, ensure_producer_video_proposal_seen,
        ensure_successful_producer_http_status, ensure_successful_producer_message_stream_status,
        ensure_successful_producer_video_final_status, execute_producer_browser_worker,
        execute_producer_image_http, execute_producer_music_http, execute_producer_video_http,
        extract_producer_browser_worker_success, extract_producer_conversation_id,
        extract_producer_send_message_job_id, extract_producer_video_create_job_id,
        extract_producer_video_final_status, fetch_producer_video_status,
        finalize_producer_image_retry_error, parse_producer_browser_worker_output,
        parse_producer_browser_worker_verified_output, parse_producer_conversation_http_job,
        parse_producer_conversation_job, parse_producer_image_generation_response,
        parse_producer_music_stream_http_response, parse_producer_send_message_job,
        parse_producer_video_status_http_response, parse_producer_video_status_response,
        prepare_producer_browser_execution_input, prepare_producer_browser_executor_service_input,
        prepare_producer_browser_worker_launch_from_prepared, producer_collect_media_urls,
        producer_http_video_failed_error, producer_http_video_missing_conversation_id_error,
        producer_http_video_missing_video_job_id_error,
        producer_http_video_missing_video_proposal_error, producer_image_retry_exhausted_error,
        producer_invalid_conversation_response_error, producer_invalid_image_response_error,
        producer_invalid_video_status_response_error, producer_music_clip_assets,
        producer_music_clip_assets_complete, producer_music_clip_ids, producer_runtime_headers,
        producer_stream_headers, producer_summary_find_tool_return_job_id,
        producer_summary_has_tool_name, producer_summary_has_video_proposal,
        read_producer_message_stream, require_producer_conversation_id,
        require_producer_video_create_job_id, resolve_producer_browser_worker_result,
        resolve_producer_image_attempt, resolve_producer_video_confirmation_prompt,
        resolve_producer_video_create_job_id_from_summaries, resolve_producer_video_session_plan,
        select_producer_video_final_url, send_producer_conversation,
        serialize_producer_browser_worker_input,
        serialize_producer_browser_worker_input_from_prepared,
        should_fallback_producer_video_http_error, should_retry_producer_image_request,
        PreparedProducerBrowserExecutorServiceInput, ProducerConversationJobData,
        ProducerImageAttemptResolution, PRODUCER_IMAGE_MAX_ATTEMPTS,
    };

    #[test]
    fn producer_music_clip_completion_builds_downloadable_media_contract() {
        let summary = json!({
            "object": "music.generation",
            "completed": true,
            "parts": [
                {"part": {"content": {"clip_id": "clip-a"}}},
                {"part": {"content": {"clip_id": "clip-b"}}}
            ]
        });
        let clip_ids = producer_music_clip_ids(&summary);
        assert_eq!(clip_ids, vec!["clip-a", "clip-b"]);

        let library = json!({
            "data": [
                {
                    "id": "clip-b",
                    "title": "Paris B",
                    "audio_url": "https://storage.example/clips/b.m4a",
                    "wav_url": "https://storage.example/clips/b.wav"
                },
                {
                    "id": "clip-a",
                    "title": "Paris A",
                    "audio_url": "https://storage.example/clips/a.m4a"
                }
            ]
        });
        let clips = producer_music_clip_assets(&library, &clip_ids);
        assert!(producer_music_clip_assets_complete(&clips, &clip_ids));

        let completed = build_producer_music_completed_response(summary, clips)
            .expect("music completion response");
        assert_eq!(completed["completed"], true);
        assert_eq!(completed["media_completed"], true);
        assert_eq!(completed["data"].as_array().map(Vec::len), Some(2));
        assert!(completed["data"].as_array().is_some_and(|items| {
            items.iter().all(|item| {
                item.get("clip_id").and_then(Value::as_str).is_some()
                    && item.get("url").and_then(Value::as_str).is_some()
            })
        }));
    }

    #[test]
    fn producer_http_video_missing_conversation_id_error_matches_contract() {
        let error = producer_http_video_missing_conversation_id_error("producer_compatible");
        assert_eq!(error.http_status, Some(500));
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
        assert_eq!(
            error.code.as_deref(),
            Some("producer_http_video_missing_conversation_id")
        );
        assert_eq!(
            error.message.as_str(),
            "Producer video bootstrap completed without returning a conversation_id."
        );
    }

    #[test]
    fn producer_http_video_missing_video_proposal_error_matches_contract() {
        let error = producer_http_video_missing_video_proposal_error("producer_compatible");
        assert_eq!(error.http_status, Some(500));
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
        assert_eq!(
            error.code.as_deref(),
            Some("producer_http_video_missing_video_proposal")
        );
        assert_eq!(
            error.message.as_str(),
            "Producer video flow did not complete the required video__propose_music_video step before confirmation."
        );
    }

    #[test]
    fn producer_http_video_missing_video_job_id_error_matches_contract() {
        let error = producer_http_video_missing_video_job_id_error("producer_compatible");
        assert_eq!(error.http_status, Some(500));
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
        assert_eq!(
            error.code.as_deref(),
            Some("producer_http_video_missing_video_job_id")
        );
        assert_eq!(
            error.message.as_str(),
            "Producer conversation completed without returning a music-video job_id."
        );
    }

    #[test]
    fn producer_invalid_video_status_response_error_matches_contract() {
        let error = producer_invalid_video_status_response_error("producer_compatible");
        assert_eq!(error.http_status, Some(500));
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
        assert_eq!(
            error.code.as_deref(),
            Some("producer_invalid_video_status_response")
        );
        assert_eq!(
            error.message.as_str(),
            "Producer.ai music-video status response was not valid JSON."
        );
    }

    #[test]
    fn build_producer_browser_executor_payload_preserves_header_and_timeout_contract() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer token-123".parse().unwrap());
        headers.insert("cookie", "a=b".parse().unwrap());
        headers.insert("user-agent", "producer-agent".parse().unwrap());
        headers.insert("accept-language", "en-US".parse().unwrap());
        headers.insert("origin", "https://www.flowmusic.app".parse().unwrap());
        headers.insert(
            "referer",
            "https://www.flowmusic.app/create".parse().unwrap(),
        );

        let payload = build_producer_browser_executor_payload(
            "https://www.flowmusic.app",
            &headers,
            &json!({ "prompt": "cover art" }),
            "producer:image",
            std::time::Duration::from_secs(900),
            Some("C:/browser/chrome.exe".to_string()),
        );

        assert_eq!(payload["baseUrl"], "https://www.flowmusic.app");
        assert_eq!(payload["authToken"], "token-123");
        assert_eq!(payload["cookieHeader"], "a=b");
        assert_eq!(payload["requestBody"]["prompt"], "cover art");
        assert_eq!(payload["model"], "producer:image");
        assert_eq!(payload["timeoutMs"], 900_000u64);
        assert_eq!(payload["browserExecutablePath"], "C:/browser/chrome.exe");
        assert_eq!(payload["userAgent"], "producer-agent");
        assert_eq!(payload["acceptLanguage"], "en-US");
        assert_eq!(payload["origin"], "https://www.flowmusic.app");
        assert_eq!(payload["referer"], "https://www.flowmusic.app/create");
    }

    #[test]
    fn build_producer_browser_executor_payload_from_prepared_preserves_header_and_timeout_contract()
    {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer token-123".parse().unwrap());
        headers.insert("cookie", "a=b".parse().unwrap());
        headers.insert("user-agent", "producer-agent".parse().unwrap());
        headers.insert("accept-language", "en-US".parse().unwrap());
        headers.insert("origin", "https://www.flowmusic.app".parse().unwrap());
        headers.insert(
            "referer",
            "https://www.flowmusic.app/create".parse().unwrap(),
        );
        let request_body = json!({ "prompt": "cover art" });
        let prepared = prepare_producer_browser_execution_input(
            "https://www.flowmusic.app/",
            &headers,
            &request_body,
            "producer:image",
            std::time::Duration::from_secs(900),
        );

        let payload = build_producer_browser_executor_payload_from_prepared(
            &prepared,
            Some("C:/browser/chrome.exe".to_string()),
        );

        assert_eq!(payload["baseUrl"], "https://www.flowmusic.app");
        assert_eq!(payload["authToken"], "token-123");
        assert_eq!(payload["cookieHeader"], "a=b");
        assert_eq!(payload["requestBody"]["prompt"], "cover art");
        assert_eq!(payload["model"], "producer:image");
        assert_eq!(payload["timeoutMs"], 900_000u64);
        assert_eq!(payload["browserExecutablePath"], "C:/browser/chrome.exe");
        assert_eq!(payload["userAgent"], "producer-agent");
        assert_eq!(payload["acceptLanguage"], "en-US");
        assert_eq!(payload["origin"], "https://www.flowmusic.app");
        assert_eq!(payload["referer"], "https://www.flowmusic.app/create");
    }

    #[test]
    fn build_producer_browser_worker_input_preserves_header_and_async_contract() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer token-123".parse().unwrap());
        headers.insert("cookie", "a=b".parse().unwrap());
        headers.insert("user-agent", "producer-agent".parse().unwrap());
        headers.insert("accept-language", "en-US".parse().unwrap());
        headers.insert("origin", "https://www.flowmusic.app".parse().unwrap());
        headers.insert(
            "referer",
            "https://www.flowmusic.app/create".parse().unwrap(),
        );
        let request_body = json!({ "prompt": "cover art" });

        let input = build_producer_browser_worker_input(
            "https://www.flowmusic.app",
            &headers,
            &request_body,
            "producer:image",
            true,
            std::time::Duration::from_secs(900),
            Some("C:/browser/chrome.exe".to_string()),
        );
        let payload = serde_json::to_value(&input).expect("producer worker input");

        assert_eq!(payload["baseUrl"], "https://www.flowmusic.app");
        assert_eq!(payload["authToken"], "token-123");
        assert_eq!(payload["cookieHeader"], "a=b");
        assert_eq!(payload["requestBody"]["prompt"], "cover art");
        assert_eq!(payload["model"], "producer:image");
        assert_eq!(payload["acceptAsyncJob"], true);
        assert_eq!(payload["timeoutMs"], 900_000u64);
        assert_eq!(payload["browserExecutablePath"], "C:/browser/chrome.exe");
        assert_eq!(payload["userAgent"], "producer-agent");
        assert_eq!(payload["acceptLanguage"], "en-US");
        assert_eq!(payload["origin"], "https://www.flowmusic.app");
        assert_eq!(payload["referer"], "https://www.flowmusic.app/create");
    }

    #[test]
    fn prepare_producer_browser_execution_input_preserves_header_and_timeout_contract() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer token-123".parse().unwrap());
        headers.insert("cookie", "a=b".parse().unwrap());
        headers.insert("user-agent", "producer-agent".parse().unwrap());
        headers.insert("accept-language", "en-US".parse().unwrap());
        headers.insert("origin", "https://www.flowmusic.app".parse().unwrap());
        headers.insert(
            "referer",
            "https://www.flowmusic.app/create".parse().unwrap(),
        );
        let request_body = json!({ "prompt": "cover art" });

        let prepared = prepare_producer_browser_execution_input(
            "https://www.flowmusic.app/",
            &headers,
            &request_body,
            "producer:image",
            std::time::Duration::from_secs(900),
        );

        assert_eq!(prepared.base_url, "https://www.flowmusic.app");
        assert_eq!(prepared.headers["authorization"], "Bearer token-123");
        assert_eq!(prepared.headers["cookie"], "a=b");
        assert_eq!(prepared.request_body["prompt"], "cover art");
        assert_eq!(prepared.model, "producer:image");
        assert_eq!(prepared.timeout, std::time::Duration::from_secs(900));
    }

    #[test]
    fn serialize_producer_browser_worker_input_preserves_header_and_async_contract() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer token-123".parse().unwrap());
        headers.insert("cookie", "a=b".parse().unwrap());
        headers.insert("user-agent", "producer-agent".parse().unwrap());
        headers.insert("accept-language", "en-US".parse().unwrap());
        headers.insert("origin", "https://www.flowmusic.app".parse().unwrap());
        headers.insert(
            "referer",
            "https://www.flowmusic.app/create".parse().unwrap(),
        );
        let request_body = json!({ "prompt": "cover art" });

        let stdin_json = serialize_producer_browser_worker_input(
            "https://www.flowmusic.app",
            &headers,
            &request_body,
            "producer:image",
            true,
            std::time::Duration::from_secs(900),
            Some("C:/browser/chrome.exe".to_string()),
        )
        .expect("producer worker stdin");
        let payload: serde_json::Value =
            serde_json::from_slice(&stdin_json).expect("stdin json should parse");

        assert_eq!(payload["baseUrl"], "https://www.flowmusic.app");
        assert_eq!(payload["authToken"], "token-123");
        assert_eq!(payload["cookieHeader"], "a=b");
        assert_eq!(payload["requestBody"]["prompt"], "cover art");
        assert_eq!(payload["model"], "producer:image");
        assert_eq!(payload["acceptAsyncJob"], true);
        assert_eq!(payload["timeoutMs"], 900_000u64);
        assert_eq!(payload["browserExecutablePath"], "C:/browser/chrome.exe");
        assert_eq!(payload["userAgent"], "producer-agent");
        assert_eq!(payload["acceptLanguage"], "en-US");
        assert_eq!(payload["origin"], "https://www.flowmusic.app");
        assert_eq!(payload["referer"], "https://www.flowmusic.app/create");
    }

    #[test]
    fn serialize_producer_browser_worker_input_from_prepared_preserves_header_and_async_contract() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer token-123".parse().unwrap());
        headers.insert("cookie", "a=b".parse().unwrap());
        headers.insert("user-agent", "producer-agent".parse().unwrap());
        headers.insert("accept-language", "en-US".parse().unwrap());
        headers.insert("origin", "https://www.flowmusic.app".parse().unwrap());
        headers.insert(
            "referer",
            "https://www.flowmusic.app/create".parse().unwrap(),
        );
        let request_body = json!({ "prompt": "cover art" });
        let prepared = prepare_producer_browser_execution_input(
            "https://www.flowmusic.app/",
            &headers,
            &request_body,
            "producer:image",
            std::time::Duration::from_secs(900),
        );

        let stdin_json = serialize_producer_browser_worker_input_from_prepared(
            &prepared,
            true,
            Some("C:/browser/chrome.exe".to_string()),
        )
        .expect("producer prepared worker stdin");
        let payload: serde_json::Value =
            serde_json::from_slice(&stdin_json).expect("stdin json should parse");

        assert_eq!(payload["baseUrl"], "https://www.flowmusic.app");
        assert_eq!(payload["authToken"], "token-123");
        assert_eq!(payload["cookieHeader"], "a=b");
        assert_eq!(payload["requestBody"]["prompt"], "cover art");
        assert_eq!(payload["model"], "producer:image");
        assert_eq!(payload["acceptAsyncJob"], true);
        assert_eq!(payload["timeoutMs"], 900_000u64);
        assert_eq!(payload["browserExecutablePath"], "C:/browser/chrome.exe");
        assert_eq!(payload["userAgent"], "producer-agent");
        assert_eq!(payload["acceptLanguage"], "en-US");
        assert_eq!(payload["origin"], "https://www.flowmusic.app");
        assert_eq!(payload["referer"], "https://www.flowmusic.app/create");
    }

    #[test]
    fn prepare_producer_browser_worker_launch_from_prepared_preserves_header_and_async_contract() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer token-123".parse().unwrap());
        headers.insert("cookie", "a=b".parse().unwrap());
        headers.insert("user-agent", "producer-agent".parse().unwrap());
        headers.insert("accept-language", "en-US".parse().unwrap());
        headers.insert("origin", "https://www.flowmusic.app".parse().unwrap());
        headers.insert(
            "referer",
            "https://www.flowmusic.app/create".parse().unwrap(),
        );
        let request_body = json!({ "prompt": "cover art" });
        let prepared = prepare_producer_browser_execution_input(
            "https://www.flowmusic.app/",
            &headers,
            &request_body,
            "producer:image",
            std::time::Duration::from_secs(900),
        );

        let launch = prepare_producer_browser_worker_launch_from_prepared(
            &prepared,
            true,
            Some("C:/browser/chrome.exe".to_string()),
            Some("node-custom".to_string()),
        )
        .expect("producer prepared worker launch");
        let payload: serde_json::Value =
            serde_json::from_slice(&launch.stdin_json).expect("stdin json should parse");

        assert_eq!(launch.node_bin, "node-custom");
        assert_eq!(
            launch
                .script_path
                .file_name()
                .and_then(|value| value.to_str()),
            Some("producer-browser-worker.mjs")
        );
        assert_eq!(payload["baseUrl"], "https://www.flowmusic.app");
        assert_eq!(payload["authToken"], "token-123");
        assert_eq!(payload["cookieHeader"], "a=b");
        assert_eq!(payload["requestBody"]["prompt"], "cover art");
        assert_eq!(payload["model"], "producer:image");
        assert_eq!(payload["acceptAsyncJob"], true);
        assert_eq!(payload["timeoutMs"], 900_000u64);
        assert_eq!(payload["browserExecutablePath"], "C:/browser/chrome.exe");
        assert_eq!(payload["userAgent"], "producer-agent");
        assert_eq!(payload["acceptLanguage"], "en-US");
        assert_eq!(payload["origin"], "https://www.flowmusic.app");
        assert_eq!(payload["referer"], "https://www.flowmusic.app/create");
    }

    #[test]
    fn prepare_producer_browser_executor_service_input_preserves_header_and_timeout_contract() {
        let input = json!({
            "baseUrl": "https://www.flowmusic.app",
            "authToken": "token-123",
            "cookieHeader": "a=b",
            "requestBody": { "prompt": "cover art" },
            "model": "producer:image",
            "timeoutMs": 900_000u64,
            "userAgent": "producer-agent",
            "acceptLanguage": "en-US",
            "origin": "https://www.flowmusic.app",
            "referer": "https://www.flowmusic.app/create"
        });

        let prepared = prepare_producer_browser_executor_service_input(&input)
            .expect("producer service input");

        assert_eq!(prepared.base_url, "https://www.flowmusic.app");
        assert_eq!(prepared.request_body["prompt"], "cover art");
        assert_eq!(prepared.model, "producer:image");
        assert_eq!(prepared.timeout, std::time::Duration::from_secs(900));
        assert_eq!(
            crate::upstream::header_map_helpers::header_map_string(
                &prepared.headers,
                "authorization"
            )
            .as_deref(),
            Some("Bearer token-123")
        );
        assert_eq!(
            crate::upstream::header_map_helpers::header_map_string(&prepared.headers, "cookie")
                .as_deref(),
            Some("a=b")
        );
        assert_eq!(
            crate::upstream::header_map_helpers::header_map_string(&prepared.headers, "user-agent")
                .as_deref(),
            Some("producer-agent")
        );
    }

    #[test]
    fn prepare_producer_browser_executor_service_input_requires_model_contract() {
        let input = json!({
            "baseUrl": "https://www.flowmusic.app",
            "requestBody": { "prompt": "cover art" }
        });

        let error = prepare_producer_browser_executor_service_input(&input)
            .expect_err("missing model should fail");
        assert_eq!(
            error.code.as_deref(),
            Some("browser_executor_missing_model")
        );
        assert_eq!(error.http_status, Some(400));
    }

    #[test]
    fn parse_producer_browser_worker_output_reads_result_contract() {
        let parsed = parse_producer_browser_worker_output(
            "{\"ok\":true,\"status\":200,\"result\":{\"conversation_id\":\"conv-1\"}}",
            "",
        )
        .expect("producer worker output");

        assert!(parsed.ok);
        assert_eq!(parsed.status, Some(200));
        assert_eq!(
            parsed
                .result
                .as_ref()
                .and_then(|value| value.get("conversation_id")),
            Some(&json!("conv-1"))
        );
    }

    #[test]
    fn parse_producer_browser_worker_output_rejects_empty_stdout_contract() {
        let error = parse_producer_browser_worker_output("", "permission denied")
            .expect_err("empty stdout should fail");
        assert_eq!(
            error.code.as_deref(),
            Some("producer_browser_worker_empty_output")
        );
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    }

    #[test]
    fn extract_producer_browser_worker_success_reads_result_payload() {
        let result = parse_producer_browser_worker_output(
            "{\"ok\":true,\"status\":200,\"result\":{\"conversation_id\":\"conv-1\"}}",
            "",
        )
        .expect("producer worker output");

        let payload =
            extract_producer_browser_worker_success(result).expect("producer worker success");
        assert_eq!(payload["conversation_id"], "conv-1");
    }

    #[test]
    fn extract_producer_browser_worker_success_rejects_missing_result_contract() {
        let result = parse_producer_browser_worker_output("{\"ok\":true,\"status\":200}", "")
            .expect("producer worker output");

        let error = extract_producer_browser_worker_success(result)
            .expect_err("missing result payload should fail");
        assert_eq!(
            error.code.as_deref(),
            Some("producer_browser_worker_missing_result")
        );
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    }

    #[test]
    fn classify_producer_browser_worker_failure_prefers_worker_error_contract() {
        let result = parse_producer_browser_worker_output(
            "{\"ok\":false,\"status\":400,\"error\":{\"code\":\"producer_worker_blocked\",\"message\":\"challenge required\",\"status\":422,\"body\":\"{\\\"detail\\\":\\\"Unauthorized\\\"}\"}}",
            "",
        )
        .expect("producer worker output");

        let error = classify_producer_browser_worker_failure(result, "", "producer_compatible");
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
        assert_eq!(error.code.as_deref(), Some("producer_worker_blocked"));
        assert_eq!(
            error.message,
            "challenge required upstream body: {\"detail\":\"Unauthorized\"}"
        );
        assert_eq!(error.http_status, Some(422));
    }

    #[test]
    fn classify_producer_browser_worker_failure_uses_stderr_when_body_missing() {
        let result = parse_producer_browser_worker_output("{\"ok\":false,\"status\":429}", "")
            .expect("producer worker output");

        let error = classify_producer_browser_worker_failure(
            result,
            "permission denied",
            "producer_compatible",
        );
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
        assert_eq!(error.http_status, Some(429));
        assert!(error.message.contains("permission denied"));
    }

    #[test]
    fn resolve_producer_browser_worker_result_reads_result_payload_contract() {
        let result = parse_producer_browser_worker_output(
            "{\"ok\":true,\"status\":200,\"result\":{\"conversation_id\":\"conv-1\"}}",
            "",
        )
        .expect("producer worker output");

        let payload = resolve_producer_browser_worker_result(result, "", "producer_compatible")
            .expect("ok worker result should succeed");
        assert_eq!(payload["conversation_id"], "conv-1");
    }

    #[test]
    fn resolve_producer_browser_worker_result_preserves_failure_contract() {
        let result = parse_producer_browser_worker_output(
            "{\"ok\":false,\"status\":400,\"error\":{\"code\":\"producer_worker_blocked\",\"message\":\"challenge required\",\"status\":422,\"body\":\"{\\\"detail\\\":\\\"Unauthorized\\\"}\"}}",
            "",
        )
        .expect("producer worker output");

        let error = resolve_producer_browser_worker_result(result, "", "producer_compatible")
            .expect_err("failed worker result should error");
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
        assert_eq!(error.code.as_deref(), Some("producer_worker_blocked"));
        assert_eq!(
            error.message,
            "challenge required upstream body: {\"detail\":\"Unauthorized\"}"
        );
        assert_eq!(error.http_status, Some(422));
    }

    #[test]
    fn parse_producer_browser_worker_verified_output_reads_result_payload_contract() {
        let payload = parse_producer_browser_worker_verified_output(
            "{\"ok\":true,\"status\":200,\"result\":{\"conversation_id\":\"conv-1\"}}",
            "",
            "producer_compatible",
        )
        .expect("verified worker output");
        assert_eq!(payload["conversation_id"], "conv-1");
    }

    #[test]
    fn parse_producer_browser_worker_verified_output_preserves_failure_contract() {
        let error = parse_producer_browser_worker_verified_output(
            "{\"ok\":false,\"status\":400,\"error\":{\"code\":\"producer_worker_blocked\",\"message\":\"challenge required\",\"status\":422,\"body\":\"{\\\"detail\\\":\\\"Unauthorized\\\"}\"}}",
            "",
            "producer_compatible",
        )
        .expect_err("failed worker output should error");
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
        assert_eq!(error.code.as_deref(), Some("producer_worker_blocked"));
        assert_eq!(
            error.message,
            "challenge required upstream body: {\"detail\":\"Unauthorized\"}"
        );
        assert_eq!(error.http_status, Some(422));
    }

    #[test]
    fn parse_producer_image_generation_response_builds_success_body() {
        let req = normalize_image_generations(json!({
            "prompt": "holographic album cover",
            "type": "clip"
        }))
        .unwrap();

        let parsed = parse_producer_image_generation_response(
            "{\"image_id\":\"img-42\"}",
            &req,
            PRODUCER_IMAGE_DEFAULT_MODEL,
            Some("e30.eyJpc3MiOiJodHRwczovL2RlbW8tcHJvamVjdC5zdXBhYmFzZS5jby9hdXRoL3YxIiwic3ViIjoidXNlci0xMjMifQ.sig"),
            "producer_compatible",
        )
        .expect("producer image generation response");

        assert_eq!(parsed["object"], "image.generation");
        assert_eq!(parsed["image_id"], "img-42");
        assert_eq!(parsed["data"][0]["image_id"], "img-42");
    }

    #[test]
    fn parse_producer_image_generation_response_rejects_invalid_json_contract() {
        let req = normalize_image_generations(json!({
            "prompt": "holographic album cover",
            "type": "clip"
        }))
        .unwrap();

        let error = parse_producer_image_generation_response(
            "not-json",
            &req,
            PRODUCER_IMAGE_DEFAULT_MODEL,
            None,
            "producer_compatible",
        )
        .expect_err("invalid JSON should fail");

        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.code.as_deref(),
            Some("producer_invalid_image_response")
        );
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    }

    #[test]
    fn parse_producer_video_status_response_reads_json_object() {
        let parsed = parse_producer_video_status_response(
            "{\"status\":\"completed\",\"asset\":\"https://cdn.example.com/video.mp4\"}",
            "producer_compatible",
        )
        .expect("producer video status response");
        assert_eq!(parsed["status"], "completed");
        assert_eq!(parsed["asset"], "https://cdn.example.com/video.mp4");
    }

    #[test]
    fn parse_producer_video_status_response_rejects_invalid_json_contract() {
        let error = parse_producer_video_status_response("not-json", "producer_compatible")
            .expect_err("invalid JSON should fail");
        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.code.as_deref(),
            Some("producer_invalid_video_status_response")
        );
        assert_eq!(
            error.message.as_str(),
            "Producer.ai music-video status response was not valid JSON."
        );
    }

    #[test]
    fn parse_producer_video_status_http_response_reads_success_contract() {
        let parsed = parse_producer_video_status_http_response(
            200,
            "{\"status\":\"completed\",\"asset\":\"https://cdn.example.com/video.mp4\"}",
            "producer_compatible",
        )
        .expect("producer video status http response");

        assert_eq!(parsed["status"], "completed");
        assert_eq!(parsed["asset"], "https://cdn.example.com/video.mp4");
    }

    #[test]
    fn parse_producer_video_status_http_response_preserves_failure_contract() {
        let error = parse_producer_video_status_http_response(
            503,
            "service unavailable",
            "producer_compatible",
        )
        .expect_err("non-2xx should fail");

        assert_eq!(error.http_status, Some(503));
        assert_eq!(
            error.code.as_deref(),
            Some("producer_http_video_status_failed")
        );
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    }

    #[test]
    fn producer_invalid_image_response_error_matches_contract() {
        let error = producer_invalid_image_response_error("producer_compatible");
        assert_eq!(error.http_status, Some(500));
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
        assert_eq!(
            error.code.as_deref(),
            Some("producer_invalid_image_response")
        );
        assert_eq!(
            error.message.as_str(),
            "Producer.ai image response was not valid JSON."
        );
    }

    #[test]
    fn producer_image_retry_exhausted_error_matches_contract() {
        let error = producer_image_retry_exhausted_error("producer_compatible");
        assert_eq!(error.http_status, Some(500));
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
        assert_eq!(
            error.code.as_deref(),
            Some("producer_image_retry_exhausted")
        );
        assert_eq!(
            error.message.as_str(),
            "Producer.ai image request exhausted retry attempts."
        );
    }

    #[test]
    fn producer_invalid_conversation_response_error_matches_contract() {
        let error = producer_invalid_conversation_response_error("producer_compatible");
        assert_eq!(error.http_status, Some(500));
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
        assert_eq!(
            error.code.as_deref(),
            Some("producer_invalid_conversation_response")
        );
        assert_eq!(
            error.message.as_str(),
            "Producer.ai conversation response was not valid JSON."
        );
    }

    #[test]
    fn parse_producer_conversation_job_reads_snake_case_job_id() {
        let parsed = parse_producer_conversation_job(
            "{\"job_id\":\"job-123\",\"status\":\"queued\"}",
            "producer_compatible",
        )
        .expect("producer conversation response");
        assert_eq!(parsed.job_id, "job-123");
        assert_eq!(parsed.body["status"], "queued");
    }

    #[test]
    fn parse_producer_conversation_job_rejects_invalid_json_contract() {
        let error = parse_producer_conversation_job("not-json", "producer_compatible")
            .expect_err("invalid JSON should fail");
        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.code.as_deref(),
            Some("producer_invalid_conversation_response")
        );
        assert_eq!(
            error.message.as_str(),
            "Producer.ai conversation response was not valid JSON."
        );
    }

    #[test]
    fn parse_producer_conversation_http_job_reads_success_contract() {
        let parsed = parse_producer_conversation_http_job(
            202,
            "{\"job_id\":\"job-123\",\"status\":\"queued\"}",
            "producer_compatible",
        )
        .expect("conversation HTTP response");

        assert_eq!(parsed.job_id, "job-123");
        assert_eq!(parsed.body["status"], "queued");
    }

    #[test]
    fn parse_producer_conversation_http_job_preserves_failure_contract() {
        let error = parse_producer_conversation_http_job(
            502,
            "{\"error\":\"gateway timeout\"}",
            "producer_compatible",
        )
        .expect_err("non-2xx should fail");

        assert_eq!(error.http_status, Some(502));
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    }

    #[test]
    fn parse_producer_conversation_http_job_rejects_invalid_json_contract() {
        let error = parse_producer_conversation_http_job(202, "not-json", "producer_compatible")
            .expect_err("invalid JSON should fail");

        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.code.as_deref(),
            Some("producer_invalid_conversation_response")
        );
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    }

    #[test]
    fn send_producer_conversation_returns_shared_job_data_type() {
        fn assert_future_output<F>(_future: F)
        where
            F: Future<Output = Result<ProducerConversationJobData, GatewayError>>,
        {
        }

        let http = rquest::Client::new();
        let headers = HeaderMap::new();
        let client_context = json!({});
        assert_future_output(send_producer_conversation(
            &http,
            "https://www.flowmusic.app",
            &headers,
            "make a video",
            None,
            &client_context,
            crate::protocol::producer::PRODUCER_DEFAULT_MODEL,
            "https://www.flowmusic.app/",
            std::time::Duration::from_secs(1),
        ));
    }

    #[test]
    fn read_producer_message_stream_returns_string_result() {
        fn assert_future_output<F>(_future: F)
        where
            F: Future<Output = Result<String, GatewayError>>,
        {
        }

        let http = rquest::Client::new();
        let headers = HeaderMap::new();
        assert_future_output(read_producer_message_stream(
            &http,
            "https://www.flowmusic.app",
            &headers,
            "job-123",
            "https://www.flowmusic.app/",
            std::time::Duration::from_secs(1),
        ));
    }

    #[test]
    fn fetch_producer_video_status_returns_json_value_result() {
        fn assert_future_output<F>(_future: F)
        where
            F: Future<Output = Result<serde_json::Value, GatewayError>>,
        {
        }

        let http = rquest::Client::new();
        let headers = HeaderMap::new();
        assert_future_output(fetch_producer_video_status(
            &http,
            "https://www.flowmusic.app",
            &headers,
            "video-job-123",
            "https://www.flowmusic.app/",
            std::time::Duration::from_secs(1),
        ));
    }

    #[test]
    fn execute_producer_browser_worker_returns_json_value_result() {
        fn assert_future_output<F>(_future: F)
        where
            F: Future<Output = Result<serde_json::Value, GatewayError>>,
        {
        }

        let prepared = PreparedProducerBrowserExecutorServiceInput {
            base_url: "https://www.flowmusic.app".to_string(),
            headers: HeaderMap::new(),
            request_body: json!({"prompt": "make a video"}),
            model: PRODUCER_IMAGE_DEFAULT_MODEL.to_string(),
            timeout: std::time::Duration::from_secs(1),
        };

        assert_future_output(execute_producer_browser_worker(
            "producer_compatible",
            &prepared,
            false,
        ));
    }

    #[test]
    fn execute_producer_video_http_returns_json_value_result() {
        fn assert_future_output<F>(_future: F)
        where
            F: Future<Output = Result<serde_json::Value, GatewayError>>,
        {
        }

        let http = rquest::Client::new();
        let headers = HeaderMap::new();
        let request_body = json!({"prompt": "make a video"});
        assert_future_output(execute_producer_video_http(
            &http,
            "https://www.flowmusic.app",
            &headers,
            &request_body,
            "producer-video",
            std::time::Duration::from_secs(1),
        ));
    }

    #[test]
    fn execute_producer_image_http_returns_json_value_result() {
        fn assert_future_output<F>(_future: F)
        where
            F: Future<Output = Result<serde_json::Value, GatewayError>>,
        {
        }

        let http = rquest::Client::new();
        let headers = HeaderMap::new();
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ImagesGenerations,
            requested_model: Some(PRODUCER_IMAGE_DEFAULT_MODEL.to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "make an image".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({"prompt": "make an image"}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        };

        assert_future_output(execute_producer_image_http(
            &http,
            "https://www.flowmusic.app",
            &headers,
            &req,
            PRODUCER_IMAGE_DEFAULT_MODEL,
            std::time::Duration::from_secs(1),
        ));
    }

    #[test]
    fn execute_producer_music_http_returns_json_value_result() {
        fn assert_future_output<F>(_future: F)
        where
            F: Future<Output = Result<serde_json::Value, GatewayError>>,
        {
        }

        let http = rquest::Client::new();
        let headers = HeaderMap::new();
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::MusicGenerations,
            requested_model: Some(PRODUCER_DEFAULT_MODEL.to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "make a song".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({"prompt": "make a song"}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        };

        assert_future_output(execute_producer_music_http(
            &http,
            "https://www.flowmusic.app",
            &headers,
            &req,
            PRODUCER_DEFAULT_MODEL,
            std::time::Duration::from_secs(1),
        ));
    }

    #[test]
    fn extract_producer_send_message_job_id_reads_success_contract() {
        let job_id = extract_producer_send_message_job_id(
            202,
            &json!({
                "job_id": "job-123",
                "status": "queued"
            }),
            "producer_compatible",
        )
        .expect("send response should expose job id");

        assert_eq!(job_id, "job-123");
    }

    #[test]
    fn extract_producer_send_message_job_id_preserves_failure_contract() {
        let error = extract_producer_send_message_job_id(
            502,
            &json!({
                "error": "gateway timeout"
            }),
            "producer_compatible",
        )
        .expect_err("non-2xx should surface upstream error");

        assert_eq!(error.http_status, Some(502));
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    }

    #[test]
    fn parse_producer_send_message_job_reads_success_contract() {
        let job_id = parse_producer_send_message_job(
            202,
            "{\"job_id\":\"job-123\",\"status\":\"queued\"}",
            "producer_compatible",
        )
        .expect("send response should parse to job id");

        assert_eq!(job_id, "job-123");
    }

    #[test]
    fn parse_producer_send_message_job_rejects_invalid_json_contract() {
        let error = parse_producer_send_message_job(202, "not-json", "producer_compatible")
            .expect_err("invalid JSON should fail");

        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.code.as_deref(),
            Some("producer_invalid_conversation_response")
        );
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    }

    #[test]
    fn producer_http_video_failed_error_matches_contract() {
        let error = producer_http_video_failed_error(
            "producer_compatible",
            "failed",
            "{\"status\":\"failed\",\"reason\":\"upstream\"}",
        );
        assert_eq!(error.http_status, Some(500));
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
        assert_eq!(error.code.as_deref(), Some("producer_http_video_failed"));
        assert_eq!(
            error.message.as_str(),
            "Producer video job entered terminal status 'failed'. status payload: {\"status\":\"failed\",\"reason\":\"upstream\"}"
        );
    }

    #[test]
    fn producer_runtime_headers_add_origin_and_referer_defaults() {
        let headers = producer_runtime_headers(&HeaderMap::new(), "https://www.producer.ai/create");
        assert_eq!(
            headers.get("accept").and_then(|value| value.to_str().ok()),
            Some("application/json, text/plain, */*")
        );
        assert_eq!(
            headers.get("origin").and_then(|value| value.to_str().ok()),
            Some("https://www.flowmusic.app")
        );
        assert_eq!(
            headers.get("referer").and_then(|value| value.to_str().ok()),
            Some("https://www.producer.ai/create")
        );
    }

    #[test]
    fn producer_stream_headers_force_event_stream_accept() {
        let headers = producer_stream_headers(&HeaderMap::new(), "https://www.producer.ai/watch");
        assert_eq!(
            headers.get("accept").and_then(|value| value.to_str().ok()),
            Some("text/event-stream")
        );
        assert_eq!(
            headers.get("referer").and_then(|value| value.to_str().ok()),
            Some("https://www.producer.ai/watch")
        );
    }

    #[test]
    fn producer_summary_helpers_detect_tool_name_and_job_id() {
        let summary = json!({
            "tool_calls": [
                { "tool_name": " video__propose_music_video " }
            ],
            "tool_returns": [
                {
                    "tool_name": "video__create_music_video",
                    "content": { "jobId": "job-camel" }
                },
                {
                    "tool_name": "video__create_music_video_alt",
                    "content": { "job_id": "job-snake" }
                }
            ]
        });
        assert!(producer_summary_has_tool_name(
            &summary,
            "tool_calls",
            "video__propose_music_video"
        ));
        assert_eq!(
            producer_summary_find_tool_return_job_id(&summary, "video__create_music_video")
                .as_deref(),
            Some("job-camel")
        );
        assert_eq!(
            producer_summary_find_tool_return_job_id(&summary, "video__create_music_video_alt")
                .as_deref(),
            Some("job-snake")
        );
    }

    #[test]
    fn producer_collect_media_urls_filters_video_assets() {
        let payload = json!({
            "audioUrl": "https://cdn.example.com/audio.mp3",
            "items": [
                "https://cdn.example.com/render.mp4",
                { "preview": "https://cdn.example.com/movie.mov" },
                { "page": "https://cdn.example.com/music-video/123" }
            ]
        });
        assert_eq!(
            producer_collect_media_urls(&payload),
            vec![
                "https://cdn.example.com/render.mp4".to_string(),
                "https://cdn.example.com/movie.mov".to_string(),
                "https://cdn.example.com/music-video/123".to_string(),
            ]
        );
    }

    #[test]
    fn extract_producer_conversation_id_trims_valid_contract() {
        let summary = json!({
            "conversation_id": "  convo-123  "
        });

        assert_eq!(
            extract_producer_conversation_id(&summary).as_deref(),
            Some("convo-123")
        );
    }

    #[test]
    fn extract_producer_conversation_id_rejects_blank_contract() {
        let summary = json!({
            "conversation_id": "   "
        });

        assert_eq!(extract_producer_conversation_id(&summary), None);
    }

    #[test]
    fn require_producer_conversation_id_reads_valid_contract() {
        let summary = json!({
            "conversation_id": " conv-123 "
        });

        let conversation_id =
            require_producer_conversation_id(&summary, "producer_compatible").expect("id");

        assert_eq!(conversation_id, "conv-123");
    }

    #[test]
    fn require_producer_conversation_id_preserves_missing_contract() {
        let summary = json!({});

        let error = require_producer_conversation_id(&summary, "producer_compatible")
            .expect_err("missing conversation id should fail");

        assert_eq!(
            error.code.as_deref(),
            Some("producer_http_video_missing_conversation_id")
        );
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    }

    #[test]
    fn producer_summary_has_video_proposal_detects_tool_call_or_return_contract() {
        let summary = json!({
            "tool_calls": [
                { "tool_name": "video__propose_music_video" }
            ],
            "tool_returns": []
        });

        assert!(producer_summary_has_video_proposal(&summary));

        let summary = json!({
            "tool_calls": [],
            "tool_returns": [
                { "tool_name": "video__propose_music_video" }
            ]
        });

        assert!(producer_summary_has_video_proposal(&summary));
    }

    #[test]
    fn ensure_producer_video_proposal_seen_accepts_detected_contract() {
        let summary = json!({
            "tool_calls": [
                { "tool_name": "video__propose_music_video" }
            ]
        });

        ensure_producer_video_proposal_seen(&summary, "producer_compatible")
            .expect("proposal should be accepted");
    }

    #[test]
    fn ensure_producer_video_proposal_seen_preserves_missing_contract() {
        let summary = json!({
            "tool_calls": [],
            "tool_returns": []
        });

        let error = ensure_producer_video_proposal_seen(&summary, "producer_compatible")
            .expect_err("missing proposal should fail");

        assert_eq!(
            error.code.as_deref(),
            Some("producer_http_video_missing_video_proposal")
        );
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    }

    #[test]
    fn extract_producer_video_create_job_id_reads_canonical_tool_return_contract() {
        let summary = json!({
            "tool_returns": [
                {
                    "tool_name": "video__create_music_video",
                    "content": { "job_id": "job-123" }
                }
            ]
        });

        assert_eq!(
            extract_producer_video_create_job_id(&summary).as_deref(),
            Some("job-123")
        );
    }

    #[test]
    fn require_producer_video_create_job_id_reads_valid_contract() {
        let job_id = require_producer_video_create_job_id(
            Some("job-123".to_string()),
            "producer_compatible",
        )
        .expect("job id should be accepted");

        assert_eq!(job_id, "job-123");
    }

    #[test]
    fn require_producer_video_create_job_id_preserves_missing_contract() {
        let error = require_producer_video_create_job_id(None, "producer_compatible")
            .expect_err("missing job id should fail");

        assert_eq!(
            error.code.as_deref(),
            Some("producer_http_video_missing_video_job_id")
        );
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    }

    #[test]
    fn build_producer_video_bootstrap_plan_builds_default_context_contract() {
        let plan = build_producer_video_bootstrap_plan(
            "https://www.flowmusic.app",
            &json!({
                "clip_id": "clip-123",
                "project_id": "proj-9"
            }),
        )
        .expect("bootstrap plan");

        assert_eq!(plan.clip_id, "clip-123");
        assert_eq!(
            plan.bootstrap_prompt,
            "Let's make a music video with the song https://www.flowmusic.app/song/clip-123"
        );
        assert_eq!(plan.bootstrap_referer, "https://www.flowmusic.app/");
        assert_eq!(plan.client_context["current_song_id"], "clip-123");
        assert_eq!(plan.client_context["song_queue"][0]["id"], "clip-123");
        assert_eq!(plan.client_context["project_id"], "proj-9");
        assert_eq!(
            plan.client_context["selected_model"],
            crate::protocol::producer::PRODUCER_DEFAULT_MODEL
        );
    }

    #[test]
    fn build_producer_video_bootstrap_plan_preserves_existing_context_contract() {
        let plan = build_producer_video_bootstrap_plan(
            "https://www.flowmusic.app/",
            &json!({
                "songId": "clip-xyz",
                "client_context": {
                    "current_song_id": "custom-song",
                    "selected_model": "custom-model"
                }
            }),
        )
        .expect("bootstrap plan");

        assert_eq!(plan.clip_id, "clip-xyz");
        assert_eq!(
            plan.bootstrap_prompt,
            "Let's make a music video with the song https://www.flowmusic.app/song/clip-xyz"
        );
        assert_eq!(plan.bootstrap_referer, "https://www.flowmusic.app/");
        assert_eq!(plan.client_context["current_song_id"], "custom-song");
        assert_eq!(plan.client_context["selected_model"], "custom-model");
    }

    #[test]
    fn resolve_producer_video_session_plan_reads_valid_contract() {
        let plan = resolve_producer_video_session_plan(
            "https://www.flowmusic.app",
            &json!({
                "conversation_id": " conv-123 "
            }),
            "producer_compatible",
        )
        .expect("session plan");

        assert_eq!(plan.conversation_id, "conv-123");
        assert_eq!(
            plan.session_referer,
            "https://www.flowmusic.app/session/conv-123"
        );
    }

    #[test]
    fn resolve_producer_video_session_plan_preserves_missing_contract() {
        let error = resolve_producer_video_session_plan(
            "https://www.flowmusic.app",
            &json!({}),
            "producer_compatible",
        )
        .expect_err("missing conversation id should fail");

        assert_eq!(
            error.code.as_deref(),
            Some("producer_http_video_missing_conversation_id")
        );
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    }

    #[test]
    fn resolve_producer_video_confirmation_prompt_skips_when_create_job_already_present_contract() {
        let creative_summary = json!({
            "tool_returns": [
                {
                    "tool_name": "video__create_music_video",
                    "content": { "job_id": "job-123" }
                }
            ]
        });

        let prompt = resolve_producer_video_confirmation_prompt(
            &json!({
                "confirm_prompt": "use this only if needed"
            }),
            &creative_summary,
        )
        .expect("confirmation prompt resolution");

        assert!(prompt.is_none());
    }

    #[test]
    fn resolve_producer_video_confirmation_prompt_reads_protocol_contract_when_needed() {
        let creative_summary = json!({
            "tool_returns": [
                {
                    "tool_name": "video__propose_music_video",
                    "content": { "status": "ok" }
                }
            ]
        });

        let prompt = resolve_producer_video_confirmation_prompt(
            &json!({
                "confirm_prompt": "approve the generated storyboard"
            }),
            &creative_summary,
        )
        .expect("confirmation prompt resolution");

        assert_eq!(prompt.as_deref(), Some("approve the generated storyboard"));
    }

    #[test]
    fn resolve_producer_video_create_job_id_from_summaries_reads_creative_contract() {
        let creative_summary = json!({
            "tool_returns": [
                {
                    "tool_name": "video__propose_music_video",
                    "content": { "status": "ok" }
                },
                {
                    "tool_name": "video__create_music_video",
                    "content": { "job_id": "job-creative" }
                }
            ]
        });

        let job_id = resolve_producer_video_create_job_id_from_summaries(
            &creative_summary,
            None,
            "producer_compatible",
        )
        .expect("creative summary should resolve job id");

        assert_eq!(job_id, "job-creative");
    }

    #[test]
    fn resolve_producer_video_create_job_id_from_summaries_reads_confirmation_fallback_contract() {
        let creative_summary = json!({
            "tool_returns": [
                {
                    "tool_name": "video__propose_music_video",
                    "content": { "status": "ok" }
                }
            ]
        });
        let confirmation_summary = json!({
            "tool_returns": [
                {
                    "tool_name": "video__create_music_video",
                    "content": { "jobId": "job-confirm" }
                }
            ]
        });

        let job_id = resolve_producer_video_create_job_id_from_summaries(
            &creative_summary,
            Some(&confirmation_summary),
            "producer_compatible",
        )
        .expect("confirmation summary should resolve job id");

        assert_eq!(job_id, "job-confirm");
    }

    #[test]
    fn select_producer_video_final_url_prefers_job_specific_contract() {
        let payload = json!({
            "items": [
                "https://cdn.example.com/music-video/other-job/render.mp4",
                "https://cdn.example.com/music-video/job-123/render.mp4"
            ]
        });

        assert_eq!(
            select_producer_video_final_url(&payload, "job-123").as_deref(),
            Some("https://cdn.example.com/music-video/job-123/render.mp4")
        );
    }

    #[test]
    fn select_producer_video_final_url_falls_back_to_generic_music_video_contract() {
        let payload = json!({
            "items": [
                "https://cdn.example.com/movie.mov",
                "https://cdn.example.com/music-video/other-job/render.mp4"
            ]
        });

        assert_eq!(
            select_producer_video_final_url(&payload, "job-123").as_deref(),
            Some("https://cdn.example.com/music-video/other-job/render.mp4")
        );
    }

    #[test]
    fn extract_producer_video_final_status_prefers_top_level_contract() {
        let payload = json!({
            "status": "  COMPLETED  ",
            "state": { "status": "failed" }
        });

        assert_eq!(
            extract_producer_video_final_status(&payload),
            "completed".to_string()
        );
    }

    #[test]
    fn extract_producer_video_final_status_falls_back_to_nested_state_contract() {
        let payload = json!({
            "state": { "status": "  CANCELED  " }
        });

        assert_eq!(
            extract_producer_video_final_status(&payload),
            "canceled".to_string()
        );
    }

    #[test]
    fn extract_producer_video_final_status_defaults_to_accepted_contract() {
        let payload = json!({
            "state": {}
        });

        assert_eq!(
            extract_producer_video_final_status(&payload),
            "accepted".to_string()
        );
    }

    #[test]
    fn ensure_successful_producer_video_final_status_accepts_non_terminal_contract() {
        let payload = json!({
            "status": "processing"
        });

        assert!(ensure_successful_producer_video_final_status(
            "producer_compatible",
            "processing",
            &payload
        )
        .is_ok());
    }

    #[test]
    fn ensure_successful_producer_video_final_status_rejects_terminal_contract() {
        let payload = json!({
            "status": "failed",
            "reason": "upstream"
        });

        let error = ensure_successful_producer_video_final_status(
            "producer_compatible",
            "failed",
            &payload,
        )
        .expect_err("terminal status should fail");

        assert_eq!(error.http_status, Some(500));
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
        assert_eq!(error.code.as_deref(), Some("producer_http_video_failed"));
        assert_eq!(
            error.message,
            "Producer video job entered terminal status 'failed'. status payload: {\"reason\":\"upstream\",\"status\":\"failed\"}"
        );
    }

    #[test]
    fn build_producer_video_http_response_preserves_completed_contract() {
        let body = build_producer_video_http_response(
            "producer_compatible",
            "https://www.producer.ai",
            &json!({
                "prompt": "launch trailer",
                "aspect_ratio": "16:9",
                "resolution": "1080p",
                "duration_s": 12
            }),
            "producer-video-model",
            "clip-1",
            "conv-video-1",
            "bootstrap-job-1",
            "creative-job-1",
            Some("confirm-job-1"),
            "video-job-1",
            &json!({
                "status": "completed",
                "preview": { "video": "https://cdn.example.com/preview.mp4" },
                "items": [
                    "https://cdn.example.com/music-video/video-job-1/final.mp4"
                ]
            }),
            &json!({ "tool_calls": [] }),
            Some(&json!({ "tool_returns": [] })),
        )
        .expect("completed response should build");

        assert_eq!(body["object"], "video.generation");
        assert_eq!(body["completed"], true);
        assert_eq!(body["job_id"], "video-job-1");
        assert_eq!(
            body["data"][0]["url"],
            "https://cdn.example.com/music-video/video-job-1/final.mp4"
        );
        assert_eq!(body["state"], "completed");
    }

    #[test]
    fn build_producer_video_http_response_rejects_terminal_status_contract() {
        let error = build_producer_video_http_response(
            "producer_compatible",
            "https://www.producer.ai",
            &json!({}),
            "producer-video-model",
            "clip-1",
            "conv-video-1",
            "bootstrap-job-1",
            "creative-job-1",
            None,
            "video-job-1",
            &json!({
                "status": "failed",
                "reason": "upstream"
            }),
            &json!({}),
            None,
        )
        .expect_err("terminal status should fail");

        assert_eq!(error.http_status, Some(500));
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
        assert_eq!(error.code.as_deref(), Some("producer_http_video_failed"));
    }

    #[test]
    fn classify_producer_image_request_failure_preserves_upstream_contract() {
        let error = classify_producer_image_request_failure(502, "gateway time-out");

        assert_eq!(error.http_status, Some(502));
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    }

    #[test]
    fn finalize_producer_image_retry_error_falls_back_to_exhausted_contract() {
        let error = finalize_producer_image_retry_error(None, "producer_compatible");

        assert_eq!(error.http_status, Some(500));
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
        assert_eq!(
            error.code.as_deref(),
            Some("producer_image_retry_exhausted")
        );
    }

    #[test]
    fn finalize_producer_image_retry_error_preserves_last_error_contract() {
        let result = finalize_producer_image_retry_error(
            Some(
                GatewayError::service_unavailable("upstream unavailable")
                    .with_code("producer_upstream_failed"),
            ),
            "producer_compatible",
        );

        assert_eq!(result.http_status, Some(503));
        assert_eq!(result.code.as_deref(), Some("producer_upstream_failed"));
        assert_eq!(result.message, "upstream unavailable");
    }

    #[test]
    fn resolve_producer_image_attempt_preserves_success_contract() {
        let req = normalize_image_generations(json!({
            "prompt": "holographic album cover",
            "type": "clip"
        }))
        .unwrap();

        let result = resolve_producer_image_attempt(
            200,
            "{\"image_id\":\"img-42\"}",
            &req,
            PRODUCER_IMAGE_DEFAULT_MODEL,
            Some("e30.eyJpc3MiOiJodHRwczovL2RlbW8tcHJvamVjdC5zdXBhYmFzZS5jby9hdXRoL3YxIiwic3ViIjoidXNlci0xMjMifQ.sig"),
            "producer_compatible",
            true,
        );

        match result {
            ProducerImageAttemptResolution::Success(body) => {
                assert_eq!(body["object"], "image.generation");
                assert_eq!(body["image_id"], "img-42");
            }
            ProducerImageAttemptResolution::Retry(error)
            | ProducerImageAttemptResolution::Fail(error) => {
                panic!("expected success, got error: {}", error.message)
            }
        }
    }

    #[test]
    fn resolve_producer_image_attempt_marks_retryable_failure_contract() {
        let req = normalize_image_generations(json!({
            "prompt": "holographic album cover",
            "type": "clip"
        }))
        .unwrap();

        let result = resolve_producer_image_attempt(
            503,
            "service unavailable",
            &req,
            PRODUCER_IMAGE_DEFAULT_MODEL,
            None,
            "producer_compatible",
            true,
        );

        match result {
            ProducerImageAttemptResolution::Retry(error) => {
                assert_eq!(error.http_status, Some(503));
                assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
            }
            ProducerImageAttemptResolution::Success(_) => panic!("expected retry, got success"),
            ProducerImageAttemptResolution::Fail(error) => {
                panic!("expected retry, got fail: {}", error.message)
            }
        }
    }

    #[test]
    fn resolve_producer_image_attempt_preserves_terminal_failure_contract() {
        let req = normalize_image_generations(json!({
            "prompt": "holographic album cover",
            "type": "clip"
        }))
        .unwrap();

        let result = resolve_producer_image_attempt(
            400,
            "validation failed",
            &req,
            PRODUCER_IMAGE_DEFAULT_MODEL,
            None,
            "producer_compatible",
            true,
        );

        match result {
            ProducerImageAttemptResolution::Fail(error) => {
                assert_eq!(error.http_status, Some(400));
                assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
            }
            ProducerImageAttemptResolution::Success(_) => panic!("expected fail, got success"),
            ProducerImageAttemptResolution::Retry(error) => {
                panic!("expected fail, got retry: {}", error.message)
            }
        }
    }

    #[test]
    fn parse_producer_music_stream_http_response_reads_success_contract() {
        let body = parse_producer_music_stream_http_response(
            200,
            [
                "event: conversation_id",
                "data: {\"id\":\"conv-music-1\"}",
                "",
                "event: generated-title",
                "data: {\"title\":\"Neon Dreams\"}",
                "",
                "event: complete",
                "data: {}",
                "",
            ]
            .join("\n")
            .as_str(),
            "producer_compatible",
            PRODUCER_IMAGE_DEFAULT_MODEL,
            "music-job-1",
        )
        .expect("stream response should parse");

        assert_eq!(body["object"], "music.generation");
        assert_eq!(body["job_id"], "music-job-1");
        assert_eq!(body["generated_title"], "Neon Dreams");
        assert_eq!(body["completed"], true);
    }

    #[test]
    fn parse_producer_music_stream_http_response_preserves_failure_contract() {
        let error = parse_producer_music_stream_http_response(
            503,
            "service unavailable",
            "producer_compatible",
            PRODUCER_IMAGE_DEFAULT_MODEL,
            "music-job-1",
        )
        .expect_err("non-2xx should fail");

        assert_eq!(error.http_status, Some(503));
        assert!(error.message.contains("service unavailable"));
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    }

    #[test]
    fn should_fallback_producer_video_http_error_respects_auth_and_status_contract() {
        assert!(!should_fallback_producer_video_http_error(
            &GatewayError::unauthorized("auth blocked").with_code("producer_http_video_timeout")
        ));
        assert!(should_fallback_producer_video_http_error(
            &GatewayError::service_unavailable("upstream unavailable")
                .with_code("producer_http_video_timeout")
        ));
        assert!(!should_fallback_producer_video_http_error(
            &GatewayError::server_error("final producer failure")
                .with_code("producer_http_video_failed")
        ));
    }

    #[test]
    fn should_retry_producer_image_request_matches_timeout_contract() {
        assert!(should_retry_producer_image_request(
            503,
            "service unavailable"
        ));
        assert!(should_retry_producer_image_request(
            500,
            "Gateway time-out while rendering"
        ));
        assert!(should_retry_producer_image_request(
            500,
            "temporarily unavailable"
        ));
        assert!(!should_retry_producer_image_request(
            400,
            "validation failed"
        ));
    }

    #[test]
    fn producer_image_max_attempts_matches_retry_contract() {
        assert_eq!(PRODUCER_IMAGE_MAX_ATTEMPTS, 2);
    }

    #[test]
    fn ensure_successful_producer_http_status_accepts_2xx_contract() {
        let result = ensure_successful_producer_http_status(204, "", "producer_compatible", None);

        assert!(result.is_ok());
    }

    #[test]
    fn ensure_successful_producer_http_status_preserves_upstream_error_contract() {
        let error = ensure_successful_producer_http_status(
            503,
            "service unavailable",
            "producer_compatible",
            None,
        )
        .expect_err("expected non-2xx to fail");

        assert_eq!(error.http_status, Some(503));
        assert!(error.message.contains("service unavailable"));
        assert_eq!(error.code, None);
    }

    #[test]
    fn ensure_successful_producer_http_status_attaches_override_code_contract() {
        let error = ensure_successful_producer_http_status(
            500,
            "status failed",
            "producer_compatible",
            Some("producer_http_video_status_failed"),
        )
        .expect_err("expected non-2xx to fail");

        assert_eq!(
            error.code.as_deref(),
            Some("producer_http_video_status_failed")
        );
        assert_eq!(error.http_status, Some(500));
    }

    #[test]
    fn ensure_successful_producer_message_stream_status_accepts_2xx_contract() {
        let result =
            ensure_successful_producer_message_stream_status(204, "", "producer_compatible");

        assert!(result.is_ok());
    }

    #[test]
    fn ensure_successful_producer_message_stream_status_preserves_failure_contract() {
        let error = ensure_successful_producer_message_stream_status(
            503,
            "service unavailable",
            "producer_compatible",
        )
        .expect_err("expected stream status to fail");

        assert_eq!(error.http_status, Some(503));
        assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
        assert!(error.message.contains("service unavailable"));
    }
}
