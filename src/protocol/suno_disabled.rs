use serde::{Deserialize, Serialize};

use crate::error::GatewayError;
use crate::implementation_lines::{compiled_out_error_for_line, RefactoredImplementationLine};

pub const SUNO_DEFAULT_MODEL: &str = "chirp-v3-5";
pub const SUNO_LEGACY_MODEL: &str = "chirp-v3-0";
pub const SUNO_DEFAULT_UPSTREAM_MODEL: &str = "chirp-auk-turbo";
pub const SUNO_LEGACY_UPSTREAM_MODEL: &str = "chirp-auk";

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

pub fn compiled_out_error() -> GatewayError {
    compiled_out_error_for_line(RefactoredImplementationLine::SunoWebReverseApi)
}

pub fn unsupported_request_plan_error() -> GatewayError {
    compiled_out_error()
}

pub fn unsupported_media_endpoint_error() -> GatewayError {
    compiled_out_error()
}
