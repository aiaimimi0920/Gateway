use crate::error::{sanitize_provider_error_message, GatewayError};
use serde_json::Value;

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

pub(crate) fn producer_http_video_failed_error(
    provider: &str,
    final_status: &str,
    status_payload: &str,
) -> GatewayError {
    GatewayError::server_error(sanitize_provider_error_message(&format!(
        "Producer video job entered terminal status '{final_status}'. status payload: {status_payload}"
    )))
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
