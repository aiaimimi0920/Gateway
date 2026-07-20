mod constants;
mod surface;

pub use constants::{
    GEMINI_API_MODULAR_ADAPTER, GEMINI_API_MODULAR_PROFILE,
    GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER,
    GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_PROFILE, GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
    GEMINI_CANVAS_WEB_REVERSE_MODULAR_PROFILE, GEMINI_WEB_REVERSE_MODULAR_ADAPTER,
    GEMINI_WEB_REVERSE_MODULAR_PROFILE,
};
pub use surface::{
    modular_surface_descriptors, GeminiSurfaceDescriptor, GeminiSurfaceKind,
    GEMINI_API_MODULAR_SURFACE, GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_SURFACE,
    GEMINI_CANVAS_WEB_REVERSE_MODULAR_SURFACE, GEMINI_WEB_REVERSE_MODULAR_SURFACE,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::candidate::ProviderExecutionMode;

    #[test]
    fn modular_canvas_surface_is_browser_owned() {
        assert!(GEMINI_CANVAS_WEB_REVERSE_MODULAR_SURFACE.browser_owned);
        assert_eq!(
            GEMINI_CANVAS_WEB_REVERSE_MODULAR_SURFACE.default_execution_mode,
            ProviderExecutionMode::BrowserBacked
        );
    }

    #[test]
    fn modular_canvas_program_surface_is_browser_owned() {
        assert!(GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_SURFACE.browser_owned);
        assert_eq!(
            GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_SURFACE.default_execution_mode,
            ProviderExecutionMode::BrowserBacked
        );
    }
}
