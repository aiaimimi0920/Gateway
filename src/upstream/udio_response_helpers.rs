//! Udio execution plan ownership, song recovery and response dispatch.

mod media_outputs;
mod upstream_errors;
mod worker_results;

#[cfg(test)]
mod tests;

// Preserve the crate-visible entry paths, including helpers used within their new owners.
#[allow(unused_imports)]
pub(crate) use upstream_errors::{
    classify_udio_media_fetch_error, classify_udio_upstream_error,
    ensure_successful_udio_media_fetch_status,
};

#[allow(unused_imports)]
pub(crate) use worker_results::{
    build_udio_browser_executor_service_result, classify_udio_browser_worker_failure,
    extract_udio_browser_worker_success, parse_udio_browser_worker_output,
    parse_udio_browser_worker_verified_output, parse_udio_remote_browser_worker_success,
    parse_udio_remote_browser_worker_verified_result, resolve_udio_browser_worker_result,
};

#[allow(unused_imports)]
pub(crate) use media_outputs::{
    build_udio_downloaded_images_response, materialize_udio_downloaded_image,
    resolve_udio_downloaded_image_mime_type, resolve_udio_image_generation_plan,
};

use crate::error::GatewayError;
use crate::protocol::udio::UdioOutputKind;
use crate::protocol::udio::UdioSong;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::browser_worker_types::UdioBrowserWorkerSuccess;
use rquest::header::HeaderMap;
use serde_json::json;
use std::time::Duration;

#[derive(Debug)]
pub(crate) struct PreparedUdioGenerationPlan {
    pub(crate) output_kind: UdioOutputKind,
    pub(crate) prompt: String,
    pub(crate) generate_request: serde_json::Value,
    pub(crate) wait_audio: bool,
    pub(crate) wait_timeout: Duration,
    pub(crate) poll_interval: Duration,
}

#[derive(Debug)]
pub(crate) struct PreparedUdioExecutionContext {
    pub(crate) base_url: String,
    pub(crate) runtime_state_object_key: Option<String>,
    pub(crate) headers: HeaderMap,
    pub(crate) generation_plan: PreparedUdioGenerationPlan,
    pub(crate) request_timeout: Duration,
}

pub(crate) fn resolve_udio_worker_latest_songs(
    worker_result: &UdioBrowserWorkerSuccess,
) -> Result<Vec<UdioSong>, GatewayError> {
    if worker_result.songs.is_empty() {
        return Ok(crate::protocol::udio::pending_songs(
            &worker_result.track_ids,
        ));
    }

    crate::protocol::udio::extract_songs_from_feed(&json!({
        "songs": worker_result.songs
    }))
}

pub(crate) fn prepare_udio_generation_plan(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    model: &str,
    browser_runtime_available: bool,
) -> Result<PreparedUdioGenerationPlan, GatewayError> {
    if !browser_runtime_available {
        return Err(crate::protocol::udio::missing_browser_runtime_error());
    }

    let output_kind = crate::protocol::udio::UdioOutputKind::from_endpoint_kind(req.endpoint_kind)?;
    let prompt = crate::protocol::udio::prompt_from_request(req)?;
    let generate_request = crate::protocol::udio::build_generate_request(req, model)?;
    let wait_audio = match output_kind {
        crate::protocol::udio::UdioOutputKind::Music => crate::protocol::udio::wait_audio(req),
        _ => true,
    };

    Ok(PreparedUdioGenerationPlan {
        output_kind,
        prompt,
        generate_request,
        wait_audio,
        wait_timeout: Duration::from_secs(crate::protocol::udio::wait_timeout_secs(req)),
        poll_interval: Duration::from_millis(crate::protocol::udio::poll_interval_ms(req)),
    })
}

pub(crate) fn prepare_udio_execution_context(
    payload: &ProviderAccountPayload,
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&std::collections::HashMap<String, String>>,
    default_timeout: Duration,
) -> Result<PreparedUdioExecutionContext, GatewayError> {
    let headers = crate::upstream::headers::build_upstream_headers_with(payload, extra_headers);
    let generation_plan = prepare_udio_generation_plan(
        req,
        model,
        headers.contains_key(rquest::header::COOKIE) || payload.runtime_state_object_key.is_some(),
    )?;
    let request_timeout =
        crate::upstream::browser_worker_runtime_helpers::udio_browser_request_timeout(
            default_timeout,
            generation_plan.wait_timeout,
        );

    Ok(PreparedUdioExecutionContext {
        base_url: payload.base_url.trim_end_matches('/').to_string(),
        runtime_state_object_key: payload.runtime_state_object_key.clone(),
        headers,
        generation_plan,
        request_timeout,
    })
}

pub(crate) fn build_udio_non_image_generation_response(
    output_kind: UdioOutputKind,
    model: &str,
    prompt: &str,
    songs: &[UdioSong],
    completed: bool,
    message: Option<&str>,
) -> Result<serde_json::Value, GatewayError> {
    match output_kind {
        UdioOutputKind::Music => Ok(crate::protocol::udio::build_music_generation_response(
            model, prompt, songs, completed, message,
        )),
        UdioOutputKind::Video => crate::protocol::udio::build_video_generation_response(
            model, prompt, songs, completed, message,
        ),
        UdioOutputKind::Image => unreachable!("image responses are built on a different path"),
    }
}
