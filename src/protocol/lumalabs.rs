//! LumaLabs media operation defaults and public protocol API.

mod actions;
mod event_urls;
mod request_options;
mod responses;
mod runtime;
mod worker_contracts;

#[cfg(test)]
mod tests;

pub use runtime::{runtime_from_payload, LumalabsRuntime};

pub use request_options::{
    action_type_for_operation, aspect_ratio_from_request, output_artifact_field_for_operation,
    output_format_from_request, prefers_url_response, prompt_from_request,
    prompt_from_request_for_operation, requested_image_count, requested_output_count,
    seed_from_request, should_auto_discover_action_type, style_from_request,
};

pub use actions::{
    build_action_request, build_action_request_for_operation, build_board_referer,
    build_client_context, extract_action_id, extract_action_output_id,
    extract_action_output_id_for_field, generate_optimistic_output_id,
};

pub use event_urls::extract_signed_url_from_event_payload;

pub use responses::{
    build_audio_generation_response, build_openai_images_response_from_bytes,
    build_openai_images_response_from_url, build_video_generation_response,
    infer_mime_type_from_url,
};

pub use worker_contracts::{
    browser_worker_input_serialize_error, browser_worker_output_parse_error,
    browser_worker_spawn_failed_error, browser_worker_stdin_error,
    browser_worker_wait_failed_error, empty_browser_worker_output_error,
    extract_browser_worker_signed_url, extract_remote_executor_signed_url,
    unsupported_image_inputs_error, unsupported_request_plan_error,
};

use crate::error::GatewayError;

pub const LUMALABS_DEFAULT_MODEL: &str = "uni-1";
pub const LUMALABS_DEFAULT_IMAGE_MODEL: &str = "uni-1";
pub const LUMALABS_DEFAULT_VIDEO_MODEL: &str = "ray3.14";
pub const LUMALABS_DEFAULT_AUDIO_MODEL: &str = "elevenlabs-music-v1";
const DEFAULT_IMAGE_ACTION_TYPE: &str = "create_image_uni_1";
const DEFAULT_VIDEO_ACTION_TYPE: &str = "create_video_ray3_14";
const DEFAULT_AUDIO_ACTION_TYPE: &str = "text_to_music_elevenlabs_v1";
const DEFAULT_ASPECT_RATIO: &str = "1:1";
const DEFAULT_STYLE: &str = "auto";
const DEFAULT_IMAGE_OUTPUT_FORMAT: &str = "png";
const DEFAULT_VIDEO_OUTPUT_FORMAT: &str = "mp4";
const DEFAULT_AUDIO_OUTPUT_FORMAT: &str = "mp3";
const DEFAULT_IMAGE_ARTIFACT_FIELD: &str = "image";
const DEFAULT_VIDEO_ARTIFACT_FIELD: &str = "video";
const DEFAULT_AUDIO_ARTIFACT_FIELD: &str = "audio";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LumalabsMediaOperation {
    Image,
    Video,
    Audio,
}

impl LumalabsMediaOperation {
    pub fn from_endpoint_kind(
        endpoint_kind: crate::protocol::canonical::EndpointKind,
    ) -> Result<Self, GatewayError> {
        match endpoint_kind {
            crate::protocol::canonical::EndpointKind::ImagesGenerations => Ok(Self::Image),
            crate::protocol::canonical::EndpointKind::ImagesEdits => Err(
                GatewayError::bad_request(
                    "LumaLabs adapters do not currently support /v1/images/edits.",
                )
                .with_provider("lumalabs_compatible")
                .with_code("unsupported_lumalabs_edit_endpoint"),
            ),
            crate::protocol::canonical::EndpointKind::VideosGenerations => Ok(Self::Video),
            crate::protocol::canonical::EndpointKind::MusicGenerations => Ok(Self::Audio),
            _ => Err(
                GatewayError::bad_request(
                    "LumaLabs adapters currently support /v1/images/generations, /v1/videos/generations, and /v1/music/generations.",
                )
                .with_code("unsupported_lumalabs_endpoint"),
            ),
        }
    }

    pub fn unsupported_output_count_error(self) -> GatewayError {
        let (message, code) = match self {
            Self::Image => (
                "LumaLabs image generation currently supports only n=1 requests.",
                "unsupported_lumalabs_image_count",
            ),
            Self::Video => (
                "LumaLabs video generation currently supports only n=1 requests.",
                "unsupported_lumalabs_video_count",
            ),
            Self::Audio => (
                "LumaLabs audio generation currently supports only n=1 requests.",
                "unsupported_lumalabs_audio_count",
            ),
        };
        GatewayError::bad_request(message)
            .with_provider("lumalabs_compatible")
            .with_code(code)
    }
}

pub fn default_model_for_operation(operation: LumalabsMediaOperation) -> &'static str {
    match operation {
        LumalabsMediaOperation::Image => LUMALABS_DEFAULT_IMAGE_MODEL,
        LumalabsMediaOperation::Video => LUMALABS_DEFAULT_VIDEO_MODEL,
        LumalabsMediaOperation::Audio => LUMALABS_DEFAULT_AUDIO_MODEL,
    }
}

pub fn default_action_type_for_operation(operation: LumalabsMediaOperation) -> &'static str {
    match operation {
        LumalabsMediaOperation::Image => DEFAULT_IMAGE_ACTION_TYPE,
        LumalabsMediaOperation::Video => DEFAULT_VIDEO_ACTION_TYPE,
        LumalabsMediaOperation::Audio => DEFAULT_AUDIO_ACTION_TYPE,
    }
}

pub fn media_operation_name(operation: LumalabsMediaOperation) -> &'static str {
    match operation {
        LumalabsMediaOperation::Image => "image",
        LumalabsMediaOperation::Video => "video",
        LumalabsMediaOperation::Audio => "audio",
    }
}
