//! Udio song schema, output kinds and public protocol API.

mod field_values;
mod generation;
mod outputs;
mod request_options;
mod songs;
mod worker_contracts;

#[cfg(test)]
mod tests;

pub use request_options::{
    poll_interval_ms, prefers_url_response, prompt_from_request, requested_output_count,
    wait_audio, wait_timeout_secs,
};

pub use generation::build_generate_request;

pub use songs::{extract_songs_from_feed, extract_track_ids, pending_songs};

pub use outputs::{
    build_music_generation_response, build_openai_images_response_from_bytes,
    build_openai_images_response_from_urls, build_video_generation_response, collect_output_urls,
    songs_ready, songs_ready_for_output,
};

pub use worker_contracts::{
    browser_worker_input_serialize_error, browser_worker_output_parse_error,
    browser_worker_spawn_failed_error, browser_worker_stdin_error, browser_worker_timeout_error,
    browser_worker_wait_failed_error, empty_browser_worker_output_error,
    missing_browser_runtime_error, missing_browser_worker_result_error,
    missing_browser_worker_track_ids_error, remote_browser_worker_result_parse_error,
    unsupported_request_plan_error,
};

use crate::error::GatewayError;
use crate::protocol::canonical::EndpointKind;
use serde::{Deserialize, Serialize};

pub const UDIO_DEFAULT_MODEL: &str = "udio-music";
const UDIO_DEFAULT_MODEL_TYPE: &str = "udio32-v1.5";
const DEFAULT_WAIT_TIMEOUT_SECS: u64 = 240;
const DEFAULT_POLL_INTERVAL_MS: u64 = 3_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UdioSong {
    pub id: String,
    pub title: Option<String>,
    pub image_url: Option<String>,
    pub audio_url: Option<String>,
    pub video_url: Option<String>,
    pub created_at: Option<String>,
    pub duration_seconds: Option<f64>,
    pub prompt: Option<String>,
    pub lyrics: Option<String>,
    pub lyric_input: Option<String>,
    pub finished: bool,
    pub ready_to_stream: bool,
    pub estimated_duration_seconds: Option<f64>,
    pub status: String,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UdioOutputKind {
    Image,
    Music,
    Video,
}

impl UdioOutputKind {
    pub fn from_endpoint_kind(endpoint_kind: EndpointKind) -> Result<Self, GatewayError> {
        match endpoint_kind {
            EndpointKind::ImagesGenerations => Ok(Self::Image),
            EndpointKind::MusicGenerations => Ok(Self::Music),
            EndpointKind::VideosGenerations => Ok(Self::Video),
            _ => Err(GatewayError::bad_request(
                "Udio adapters support only image, music, and video generation endpoints.",
            )
            .with_provider("udio_compatible")
            .with_code("unsupported_udio_endpoint")),
        }
    }

    pub fn browser_target_key(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Music => "audio",
            Self::Video => "video",
        }
    }

    fn object_name(self) -> &'static str {
        match self {
            Self::Image => "image.generation",
            Self::Music => "music.generation",
            Self::Video => "video.generation",
        }
    }

    fn media_kind(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Music => "audio",
            Self::Video => "video",
        }
    }

    fn missing_output_code(self) -> &'static str {
        match self {
            Self::Image => "udio_missing_image_url",
            Self::Music => "udio_missing_audio_url",
            Self::Video => "udio_missing_video_url",
        }
    }

    fn missing_output_message(self) -> &'static str {
        match self {
            Self::Image => "Udio songs did not produce any image URLs.",
            Self::Music => "Udio songs did not produce any audio URLs.",
            Self::Video => "Udio songs did not produce any video URLs.",
        }
    }

    pub fn timeout_message(self) -> &'static str {
        match self {
            Self::Image => "Timed out waiting for Udio cover art to reach a terminal state.",
            Self::Music => "Timed out waiting for Udio audio to reach a terminal state.",
            Self::Video => "Timed out waiting for Udio video to reach a terminal state.",
        }
    }
}
