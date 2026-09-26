//! Suno verified response orchestration and endpoint response dispatch.

mod http_responses;
mod media_outputs;
mod upstream_errors;
mod worker_results;

#[cfg(test)]
mod tests;

// Preserve the crate-visible entry paths, including helpers used within their new owners.
#[allow(unused_imports)]
pub(crate) use http_responses::{
    parse_suno_challenge_probe_body, parse_suno_challenge_probe_http_response,
    parse_suno_feed_body, parse_suno_feed_http_clips_response, parse_suno_feed_http_response,
    parse_suno_generation_body, parse_suno_generation_http_clips_response,
    parse_suno_generation_http_poll_seed, parse_suno_generation_http_response,
    parse_suno_json_response,
};

#[allow(unused_imports)]
pub(crate) use upstream_errors::{
    classify_suno_media_fetch_error, classify_suno_upstream_error,
    ensure_successful_suno_media_fetch_status, ensure_successful_suno_upstream_status,
    suno_challenge_required_error,
};

#[allow(unused_imports)]
pub(crate) use worker_results::{
    build_suno_browser_executor_service_result, classify_suno_browser_worker_failure,
    extract_suno_browser_worker_success, parse_suno_browser_worker_output,
    parse_suno_remote_browser_worker_success, parse_suno_remote_browser_worker_verified_result,
    resolve_suno_browser_worker_result,
};

#[allow(unused_imports)]
pub(crate) use media_outputs::{
    build_suno_downloaded_images_response, materialize_suno_downloaded_image,
    resolve_suno_downloaded_image_mime_type, resolve_suno_image_generation_plan,
};

use crate::error::GatewayError;
use crate::upstream::browser_worker_types::SunoBrowserWorkerSuccess;
use serde_json::Value;

pub(crate) fn parse_suno_challenge_probe_verified_response(
    status: u16,
    headers: &rquest::header::HeaderMap,
    body_text: &str,
    missing_challenge_token: bool,
) -> Result<Value, GatewayError> {
    let body = parse_suno_challenge_probe_http_response(
        status,
        headers,
        body_text,
        missing_challenge_token,
    )?;
    if crate::protocol::suno::challenge_required(&body)? && missing_challenge_token {
        return Err(suno_challenge_required_error());
    }
    Ok(body)
}

pub(crate) fn parse_suno_browser_worker_verified_output(
    stdout: &str,
    stderr: &str,
    provider: &str,
) -> Result<SunoBrowserWorkerSuccess, GatewayError> {
    let result = parse_suno_browser_worker_output(stdout, stderr)?;
    resolve_suno_browser_worker_result(result, provider)
}

pub(crate) fn build_suno_non_image_generation_response(
    endpoint_kind: crate::protocol::canonical::EndpointKind,
    model: &str,
    prompt: &str,
    clips: &[crate::protocol::suno::SunoClip],
    completed: bool,
    message: Option<&str>,
) -> Result<Value, GatewayError> {
    match endpoint_kind {
        crate::protocol::canonical::EndpointKind::VideosGenerations => {
            crate::protocol::suno::build_video_generation_response(
                model, prompt, clips, completed, message,
            )
        }
        crate::protocol::canonical::EndpointKind::MusicGenerations => {
            Ok(crate::protocol::suno::build_music_generation_response(
                model, prompt, clips, completed, message,
            ))
        }
        _ => Err(crate::protocol::suno::unsupported_media_endpoint_error()),
    }
}
