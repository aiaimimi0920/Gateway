use crate::error::GatewayError;
use crate::implementation_lines::{compiled_out_error_for_line, RefactoredImplementationLine};

pub const LUMALABS_DEFAULT_MODEL: &str = "uni-1";
pub const LUMALABS_DEFAULT_IMAGE_MODEL: &str = "uni-1";
pub const LUMALABS_DEFAULT_VIDEO_MODEL: &str = "ray3.14";
pub const LUMALABS_DEFAULT_AUDIO_MODEL: &str = "elevenlabs-music-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LumalabsMediaOperation {
    Image,
    Video,
    Audio,
}

pub fn compiled_out_error() -> GatewayError {
    compiled_out_error_for_line(RefactoredImplementationLine::LumaLabsWebReverseApi)
}

pub fn unsupported_request_plan_error() -> GatewayError {
    compiled_out_error()
}
