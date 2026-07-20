use crate::routing::candidate::ProviderExecutionMode;

use super::constants::{
    GEMINI_API_MODULAR_ADAPTER, GEMINI_API_MODULAR_PROFILE,
    GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER,
    GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_PROFILE, GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
    GEMINI_CANVAS_WEB_REVERSE_MODULAR_PROFILE, GEMINI_WEB_REVERSE_MODULAR_ADAPTER,
    GEMINI_WEB_REVERSE_MODULAR_PROFILE,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeminiSurfaceKind {
    OfficialApi,
    WebReverse,
    CanvasWebReverse,
    CanvasProgramWebReverse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeminiSurfaceDescriptor {
    pub protocol_profile: &'static str,
    pub adapter: &'static str,
    pub default_execution_mode: ProviderExecutionMode,
    pub kind: GeminiSurfaceKind,
    pub browser_owned: bool,
}

pub const GEMINI_API_MODULAR_SURFACE: GeminiSurfaceDescriptor = GeminiSurfaceDescriptor {
    protocol_profile: GEMINI_API_MODULAR_PROFILE,
    adapter: GEMINI_API_MODULAR_ADAPTER,
    default_execution_mode: ProviderExecutionMode::DirectHttp,
    kind: GeminiSurfaceKind::OfficialApi,
    browser_owned: false,
};

pub const GEMINI_WEB_REVERSE_MODULAR_SURFACE: GeminiSurfaceDescriptor = GeminiSurfaceDescriptor {
    protocol_profile: GEMINI_WEB_REVERSE_MODULAR_PROFILE,
    adapter: GEMINI_WEB_REVERSE_MODULAR_ADAPTER,
    default_execution_mode: ProviderExecutionMode::DirectHttp,
    kind: GeminiSurfaceKind::WebReverse,
    browser_owned: false,
};

pub const GEMINI_CANVAS_WEB_REVERSE_MODULAR_SURFACE: GeminiSurfaceDescriptor =
    GeminiSurfaceDescriptor {
        protocol_profile: GEMINI_CANVAS_WEB_REVERSE_MODULAR_PROFILE,
        adapter: GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        default_execution_mode: ProviderExecutionMode::BrowserBacked,
        kind: GeminiSurfaceKind::CanvasWebReverse,
        browser_owned: true,
    };

pub const GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_SURFACE: GeminiSurfaceDescriptor =
    GeminiSurfaceDescriptor {
        protocol_profile: GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_PROFILE,
        adapter: GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER,
        default_execution_mode: ProviderExecutionMode::BrowserBacked,
        kind: GeminiSurfaceKind::CanvasProgramWebReverse,
        browser_owned: true,
    };

pub fn modular_surface_descriptors() -> [GeminiSurfaceDescriptor; 4] {
    [
        GEMINI_API_MODULAR_SURFACE,
        GEMINI_WEB_REVERSE_MODULAR_SURFACE,
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_SURFACE,
        GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_SURFACE,
    ]
}
