mod app_endpoint;
mod bootstrap_context;
mod bootstrap_probe;
mod relay_config;

pub use super::shared::{
    GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER,
    GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_PROFILE,
};
pub use app_endpoint::{
    GeminiCanvasProgramAppEndpointContract, GeminiCanvasProgramInvokeContract,
    GeminiCanvasProgramInvokeTargetCandidate,
};
pub use bootstrap_context::GeminiCanvasProgramBootstrapContext;
pub use bootstrap_probe::{build_program_bootstrap_probe, GeminiCanvasProgramBootstrapProbe};
pub use relay_config::{relay_config_from_payload, GeminiCanvasProgramRelayConfig};

#[cfg(test)]
mod tests;
