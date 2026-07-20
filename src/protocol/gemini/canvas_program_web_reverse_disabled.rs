use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::GatewayError;
use crate::implementation_lines::{compiled_out_error_for_line, RefactoredImplementationLine};
use crate::protocol::gemini::shared::GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER;
use crate::routing::candidate::ProviderAccountPayload;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct GeminiCanvasProgramInvokeTargetCandidate {
    pub url: Option<String>,
    pub source: Option<String>,
    pub mime_type: Option<String>,
    pub kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct GeminiCanvasProgramInvokeContract {
    pub operation: Option<String>,
    pub transport_kind: Option<String>,
    pub target: Option<String>,
    pub target_source: Option<String>,
    pub target_mime_type: Option<String>,
    pub target_candidates: Vec<GeminiCanvasProgramInvokeTargetCandidate>,
    pub ws_url: Option<String>,
    pub api_style: Option<String>,
    pub request_path: Option<String>,
    pub request_envelope_kind: Option<String>,
    pub request_url: Option<String>,
    pub request_body: Option<String>,
    pub request_rpc_id: Option<String>,
    pub response_rpc_id: Option<String>,
    pub source_path: Option<String>,
    pub model_hint: Option<String>,
    pub cookie_header: Option<String>,
    pub action_name: Option<String>,
    pub action_input: Option<String>,
    pub prompt: Option<String>,
    pub duration_seconds: Option<f64>,
    pub aspect_ratio: Option<String>,
    pub ui_state: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct GeminiCanvasProgramAppEndpointContract {
    pub canvas_program_url: Option<String>,
    pub page_url: Option<String>,
    pub app_path: Option<String>,
    pub conversation_id: Option<String>,
    pub response_id: Option<String>,
    pub invoke_base_url: Option<String>,
    pub music_ws_url: Option<String>,
    pub video_invoke_path: Option<String>,
    pub canvas_program_action: Option<String>,
    pub canvas_program_action_input: Option<String>,
    pub canvas_program_invoke_contract: Option<GeminiCanvasProgramInvokeContract>,
}

impl GeminiCanvasProgramAppEndpointContract {
    pub fn has_concrete_handle(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct GeminiCanvasProgramBootstrapContext {
    pub runtime_state_object_key: String,
    pub share_id: String,
    pub api_base_url: String,
    pub relay_ws_endpoint: Option<String>,
    pub client_label: Option<String>,
    pub canvas_program_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct GeminiCanvasProgramRelayConfig {
    pub bootstrap: GeminiCanvasProgramBootstrapContext,
    pub app_endpoint: GeminiCanvasProgramAppEndpointContract,
}

impl GeminiCanvasProgramRelayConfig {
    pub fn has_concrete_handle(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiCanvasProgramBootstrapProbe {
    pub share_url: String,
    pub app_url: String,
    pub expect_canvas_program: bool,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

fn compiled_out(payload: Option<&ProviderAccountPayload>) -> GatewayError {
    let error = compiled_out_error_for_line(RefactoredImplementationLine::GeminiCanvasProgram);
    match payload {
        Some(payload) => error.with_provider(payload.adapter.clone()),
        None => error.with_provider(GEMINI_CANVAS_PROGRAM_WEB_REVERSE_MODULAR_ADAPTER),
    }
}

pub fn relay_config_from_payload(
    payload: &ProviderAccountPayload,
) -> Result<GeminiCanvasProgramRelayConfig, GatewayError> {
    Err(compiled_out(Some(payload)))
}

pub fn build_program_bootstrap_probe(
    payload: &ProviderAccountPayload,
) -> Result<GeminiCanvasProgramBootstrapProbe, GatewayError> {
    Err(compiled_out(Some(payload)))
}

pub fn is_concrete_gemini_canvas_app_path(_value: &str) -> bool {
    false
}

pub fn is_concrete_gemini_canvas_conversation_id(_value: &str) -> bool {
    false
}

pub fn is_concrete_gemini_canvas_program_url(_value: &str) -> bool {
    false
}

pub fn normalize_gemini_canvas_program_bootstrap_operation(operation: &str) -> &str {
    match operation {
        "tts" => "text",
        "text" | "image" | "music" | "video" => operation,
        _ => "image",
    }
}

pub fn gemini_canvas_program_payload_conversation_id(
    _payload: &ProviderAccountPayload,
) -> Option<String> {
    None
}

pub fn gemini_canvas_program_payload_handle_matches_operation(
    _payload: &ProviderAccountPayload,
    _operation: &str,
) -> bool {
    false
}

pub fn gemini_canvas_program_payload_has_concrete_handle(
    _payload: &ProviderAccountPayload,
) -> bool {
    false
}

pub fn gemini_canvas_program_payload_locator(
    _payload: &ProviderAccountPayload,
    _stream_body: Option<&str>,
) -> Option<crate::protocol::gemini_canvas::GeminiCanvasStreamGenerateLocator> {
    None
}

pub fn gemini_canvas_program_payload_operation(
    _payload: &ProviderAccountPayload,
) -> Option<String> {
    None
}

pub fn gemini_canvas_program_payload_page_url(
    _payload: &ProviderAccountPayload,
    _base_url: &str,
) -> Option<String> {
    None
}

pub fn gemini_canvas_program_payload_response_id(
    _payload: &ProviderAccountPayload,
) -> Option<String> {
    None
}

pub fn gemini_canvas_program_payload_source_path(
    _payload: &ProviderAccountPayload,
) -> Option<String> {
    None
}

pub fn strip_gemini_canvas_program_handle_hints_from_payload(
    payload: &ProviderAccountPayload,
) -> ProviderAccountPayload {
    payload.clone()
}
