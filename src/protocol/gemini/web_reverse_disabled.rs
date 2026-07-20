use bytes::Bytes;
use serde_json::Value;
use std::collections::HashMap;

use crate::error::GatewayError;
use crate::implementation_lines::{compiled_out_error_for_line, RefactoredImplementationLine};
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse, EndpointKind};
pub use crate::protocol::gemini::shared::{
    GEMINI_WEB_REVERSE_MODULAR_ADAPTER, GEMINI_WEB_REVERSE_MODULAR_PROFILE,
};
use crate::protocol::gemini_canvas::GeminiCanvasMediaOperation;

pub const GEMINI_WEB_BOOTSTRAP_FAILED_CODE: &str = "gemini_web_bootstrap_failed";
pub const GEMINI_WEB_BROWSER_CHALLENGE_REQUIRED_CODE: &str =
    "gemini_web_browser_challenge_required";
pub const GEMINI_WEB_SESSION_INVALID_CODE: &str = "gemini_web_session_invalid";
pub const GEMINI_WEB_DEFAULT_ACCEPT_LANGUAGE: &str = "en-US,en;q=0.9";
pub const GEMINI_WEB_DEFAULT_APP_PATH: &str = "/app";
pub const GEMINI_WEB_DEFAULT_SAME_DOMAIN_HEADER: &str = "same-origin";
pub const GEMINI_WEB_DEFAULT_SEC_CH_UA: &str = "\"Not.A/Brand\";v=\"8\"";
pub const GEMINI_WEB_DEFAULT_SEC_CH_UA_ARCH: &str = "\"x86\"";
pub const GEMINI_WEB_DEFAULT_SEC_CH_UA_BITNESS: &str = "\"64\"";
pub const GEMINI_WEB_DEFAULT_SEC_CH_UA_FORM_FACTORS: &str = "\"Desktop\"";
pub const GEMINI_WEB_DEFAULT_SEC_CH_UA_FULL_VERSION: &str = "\"126.0.0.0\"";
pub const GEMINI_WEB_DEFAULT_SEC_CH_UA_FULL_VERSION_LIST: &str = "\"Chromium\";v=\"126.0.0.0\"";
pub const GEMINI_WEB_DEFAULT_SEC_CH_UA_MOBILE: &str = "?0";
pub const GEMINI_WEB_DEFAULT_SEC_CH_UA_MODEL: &str = "\"\"";
pub const GEMINI_WEB_DEFAULT_SEC_CH_UA_PLATFORM: &str = "\"Windows\"";
pub const GEMINI_WEB_DEFAULT_SEC_CH_UA_PLATFORM_VERSION: &str = "\"10.0.0\"";
pub const GEMINI_WEB_DEFAULT_SEC_CH_UA_WOW64: &str = "?0";
pub const GEMINI_WEB_DEFAULT_STREAM_GENERATE_PATH: &str =
    "/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate";
pub const GEMINI_WEB_DEFAULT_USER_AGENT: &str = "Mozilla/5.0";
pub const GEMINI_WEB_MODEL_HEADER_KEY: &str = "x-goog-ext-525001261-jspb";
pub const GEMINI_WEB_MODEL_HEADER_2_KEY: &str = "x-goog-ext-525001261-jspb-2";
pub const GEMINI_WEB_MODEL_HEADER_3_KEY: &str = "x-goog-ext-525001261-jspb-3";
pub const GEMINI_WEB_REQUEST_CONTEXT_HEADER_KEY: &str = "x-goog-request-context";

#[derive(Debug, Clone, Default)]
pub struct GeminiWebBootstrap {
    pub access_token: Option<String>,
    pub build_label: Option<String>,
    pub session_id: Option<String>,
    pub language: String,
    pub push_id: Option<String>,
    pub client_pctx: Option<String>,
    pub app_page_path: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct GeminiWebRequest {
    pub query: Vec<(String, String)>,
    pub form: Vec<(String, String)>,
}

fn compiled_out_error() -> GatewayError {
    compiled_out_error_for_line(RefactoredImplementationLine::GeminiWebReverse)
        .with_provider("gemini_web_compatible")
}

pub fn bootstrap_from_payload_cache(
    _extra_body: Option<&HashMap<String, Value>>,
) -> Option<GeminiWebBootstrap> {
    None
}

pub fn extract_app_page_path_from_html(_html: &str) -> Option<String> {
    None
}

pub fn extract_app_page_path_from_url(_url: &str) -> Option<String> {
    None
}

pub fn merge_bootstrap_from_fallback(
    primary: GeminiWebBootstrap,
    _fallback: Option<&GeminiWebBootstrap>,
) -> GeminiWebBootstrap {
    primary
}

pub fn normalize_app_page_path_candidate(candidate: &str) -> Option<String> {
    let trimmed = candidate.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

pub fn parse_bootstrap_from_app_html(
    _html: &str,
    fallback_language: Option<&str>,
) -> Result<GeminiWebBootstrap, GatewayError> {
    Ok(GeminiWebBootstrap {
        language: fallback_language.unwrap_or("en-US").to_string(),
        ..GeminiWebBootstrap::default()
    })
}

pub fn extract_model_headers(
    _extra_body: Option<&HashMap<String, Value>>,
    _model: &str,
) -> Vec<(String, String)> {
    Vec::new()
}

pub fn pack_request(
    _req: &CanonicalRelayRequest,
    _model: &str,
    _bootstrap: &GeminiWebBootstrap,
) -> Result<GeminiWebRequest, GatewayError> {
    Err(compiled_out_error())
}

pub fn classify_gemini_web_http_error(
    _status: u16,
    _content_type: Option<&str>,
    body: &str,
) -> GatewayError {
    GatewayError::server_error(body.to_string())
        .with_provider("gemini_web_compatible")
        .with_code(GEMINI_WEB_BOOTSTRAP_FAILED_CODE)
}

pub fn response_indicates_browser_challenge(
    _status: u16,
    _content_type: Option<&str>,
    _body: &str,
) -> bool {
    false
}

pub fn response_indicates_session_invalid(
    _status: u16,
    _content_type: Option<&str>,
    _body: &str,
) -> bool {
    false
}

pub fn accumulate_gemini_web_response(
    _body: &str,
    _model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    Err(compiled_out_error())
}

pub fn translate_gemini_web_to_openai_sse(
    _body: &str,
    _model: &str,
) -> Result<Vec<Bytes>, GatewayError> {
    Err(compiled_out_error())
}

pub fn prompt_for_legacy_mixed_lane_media_request(
    _req: &CanonicalRelayRequest,
    _operation: GeminiCanvasMediaOperation,
) -> Result<String, GatewayError> {
    Err(compiled_out_error())
}

pub fn supports_endpoint(endpoint_kind: EndpointKind) -> bool {
    matches!(
        endpoint_kind,
        EndpointKind::ChatCompletions
            | EndpointKind::Messages
            | EndpointKind::Responses
            | EndpointKind::Completions
    )
}
