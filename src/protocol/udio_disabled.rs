use crate::error::GatewayError;
use crate::implementation_lines::{compiled_out_error_for_line, RefactoredImplementationLine};

pub const UDIO_DEFAULT_MODEL: &str = "udio-music";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UdioOutputKind {
    Image,
    Music,
    Video,
}

pub fn compiled_out_error() -> GatewayError {
    compiled_out_error_for_line(RefactoredImplementationLine::UdioWebReverseApi)
}

pub fn unsupported_request_plan_error() -> GatewayError {
    compiled_out_error()
}
