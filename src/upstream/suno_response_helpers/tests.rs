mod http_responses;
mod media_outputs;
mod upstream_errors;
mod worker_results;
use super::{
    build_suno_browser_executor_service_result, build_suno_downloaded_images_response,
    build_suno_non_image_generation_response, classify_suno_browser_worker_failure,
    classify_suno_media_fetch_error, classify_suno_upstream_error,
    ensure_successful_suno_media_fetch_status, ensure_successful_suno_upstream_status,
    extract_suno_browser_worker_success, materialize_suno_downloaded_image,
    parse_suno_browser_worker_output, parse_suno_browser_worker_verified_output,
    parse_suno_challenge_probe_body, parse_suno_challenge_probe_http_response,
    parse_suno_challenge_probe_verified_response, parse_suno_feed_body,
    parse_suno_feed_http_clips_response, parse_suno_feed_http_response, parse_suno_generation_body,
    parse_suno_generation_http_clips_response, parse_suno_generation_http_poll_seed,
    parse_suno_generation_http_response, parse_suno_remote_browser_worker_success,
    parse_suno_remote_browser_worker_verified_result, resolve_suno_browser_worker_result,
    resolve_suno_downloaded_image_mime_type, resolve_suno_image_generation_plan,
    suno_challenge_required_error,
};
use crate::upstream::browser_worker_types::SunoBrowserWorkerResult;
use serde_json::json;

fn image_generation_request(
    raw_body: serde_json::Value,
) -> crate::protocol::canonical::CanonicalRelayRequest {
    crate::protocol::canonical::CanonicalRelayRequest {
        protocol_family: crate::protocol::canonical::ProtocolFamily::OpenAi,
        endpoint_kind: crate::protocol::canonical::EndpointKind::ImagesGenerations,
        requested_model: None,
        stream: false,
        messages: Vec::new(),
        tools: Vec::new(),
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body,
        previous_response_id: None,
        explicit_session_key: None,
        extra: std::collections::HashMap::new(),
    }
}
