//! Suno clip schema, canonical music normalization and public protocol API.

mod clips;
mod field_values;
mod generation;
mod request_options;
mod responses;
mod worker_contracts;

#[cfg(test)]
mod tests;

pub use request_options::{
    poll_interval_ms, prefers_url_response, prompt_from_request, requested_output_count,
    resolve_model, wait_audio, wait_for_completion, wait_timeout_secs,
};

pub use generation::{
    build_browser_token_header_value, build_challenge_check_request, build_generate_request,
    challenge_required, has_challenge_token,
};

pub use clips::{
    build_feed_poll_request, clips_ready_for_endpoint, extract_clip_ids, extract_clips_from_feed,
};

pub use responses::{
    build_music_generation_response, build_openai_images_response_from_bytes,
    build_openai_images_response_from_urls, build_video_generation_response,
    image_urls_for_requested_count, infer_mime_type_from_url,
};

pub use worker_contracts::{
    browser_worker_input_serialize_error, browser_worker_output_parse_error,
    browser_worker_spawn_failed_error, browser_worker_stdin_error, browser_worker_timeout_error,
    browser_worker_wait_failed_error, empty_browser_worker_output_error,
    missing_browser_worker_clips_error, missing_browser_worker_result_error,
    remote_browser_worker_result_parse_error, unsupported_media_endpoint_error,
};

use self::field_values::read_optional_string;
use self::request_options::prompt_from_request_body;
use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalMessage;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::canonical::EndpointKind;
use crate::protocol::canonical::MessageRole;
use crate::protocol::canonical::ProtocolFamily;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const SUNO_DEFAULT_MODEL: &str = "chirp-v3-5";
pub const SUNO_LEGACY_MODEL: &str = "chirp-v3-0";
pub const SUNO_DEFAULT_UPSTREAM_MODEL: &str = "chirp-auk-turbo";
pub const SUNO_LEGACY_UPSTREAM_MODEL: &str = "chirp-auk";
const DEFAULT_WAIT_TIMEOUT_SECS: u64 = 150;
const DEFAULT_POLL_INTERVAL_MS: u64 = 3_000;
const DEFAULT_IMAGE_MIME_TYPE: &str = "image/png";
const DEFAULT_VIDEO_MIME_TYPE: &str = "video/mp4";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SunoClip {
    pub id: String,
    pub title: Option<String>,
    pub image_url: Option<String>,
    pub lyric: Option<String>,
    pub audio_url: Option<String>,
    pub video_url: Option<String>,
    pub created_at: Option<String>,
    pub model_name: Option<String>,
    pub prompt: Option<String>,
    pub gpt_description_prompt: Option<String>,
    pub status: String,
    pub clip_type: Option<String>,
    pub tags: Option<String>,
    pub negative_tags: Option<String>,
    pub duration: Option<String>,
    pub error_message: Option<String>,
}

pub fn normalize_music_generations(body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    let body_obj = body.as_object().ok_or_else(|| {
        GatewayError::bad_request("Music generation request body must be a JSON object.")
    })?;

    if body_obj
        .get("stream")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
    {
        return Err(GatewayError::bad_request(
            "Music generation endpoints do not support `stream: true`.",
        )
        .with_code("music_streaming_not_supported"));
    }

    let prompt = prompt_from_request_body(body_obj).ok_or_else(|| {
        GatewayError::bad_request(
            "Suno music requests require one of: prompt, input, lyrics, or parts[].content.",
        )
        .with_code("missing_music_prompt")
    })?;

    let requested_model = body_obj
        .get("model")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| Some(SUNO_DEFAULT_MODEL.to_string()));

    let explicit_session_key =
        read_optional_string(body_obj, &["user", "session_key", "sessionKey"]);

    Ok(CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::MusicGenerations,
        requested_model,
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![crate::protocol::canonical::ContentPart::Text { text: prompt }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: body,
        previous_response_id: None,
        explicit_session_key,
        extra: std::collections::HashMap::new(),
    })
}
