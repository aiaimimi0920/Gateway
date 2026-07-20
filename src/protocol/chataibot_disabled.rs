use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;

pub const CHATAIBOT_DEFAULT_MODEL: &str = "google-nano-banana-2";
pub const CHATAIBOT_QUOTA_PROBE_PATH: &str = "/api/user/answers-count/v2";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChataibotModelSpec {
    pub provider: &'static str,
    pub version: Option<&'static str>,
    pub generation_cost: u32,
    pub edit_mode: Option<&'static str>,
    pub edit_cost: Option<u32>,
    pub merge_mode: Option<&'static str>,
    pub merge_cost: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChataibotUpload {
    pub mime_type: String,
    pub base64_data: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultipartBody {
    pub content_type: String,
    pub bytes: Vec<u8>,
}

fn compiled_out_error() -> GatewayError {
    crate::implementation_lines::compiled_out_error_for_line(
        crate::implementation_lines::RefactoredImplementationLine::ChataibotWebReverse,
    )
}

pub fn free_tier_model_ids() -> &'static [&'static str] {
    &[]
}

pub fn supported_model_ids() -> &'static [&'static str] {
    &[]
}

pub fn resolve_model_spec(_model: &str) -> Option<ChataibotModelSpec> {
    None
}

pub fn build_edit_multipart_body(
    _prompt: &str,
    _upload: &ChataibotUpload,
    _mode: &str,
) -> Result<MultipartBody, GatewayError> {
    Err(compiled_out_error())
}

pub fn build_merge_multipart_body(
    _prompt: &str,
    _uploads: &[ChataibotUpload],
    _merge_type: &str,
) -> Result<MultipartBody, GatewayError> {
    Err(compiled_out_error())
}

pub fn aspect_ratio_from_request(_req: &CanonicalRelayRequest) -> String {
    "auto".to_string()
}

pub fn extract_uploads_from_request_body(
    _body: &Value,
) -> Result<Vec<ChataibotUpload>, GatewayError> {
    Err(compiled_out_error())
}

pub fn prefers_url_response(_req: &CanonicalRelayRequest) -> Result<bool, GatewayError> {
    Err(compiled_out_error())
}

pub fn prompt_from_request(_req: &CanonicalRelayRequest) -> Result<String, GatewayError> {
    Err(compiled_out_error())
}

pub fn requested_image_count(_req: &CanonicalRelayRequest) -> usize {
    1
}

pub fn build_openai_images_response_from_bytes(
    _req: &CanonicalRelayRequest,
    _prompt: &str,
    _images: &[(String, Vec<u8>)],
) -> Value {
    serde_json::json!({"error": "gateway_provider_line_compiled_out"})
}

pub fn build_openai_images_response_from_urls(
    _req: &CanonicalRelayRequest,
    _prompt: &str,
    _image_urls: &[String],
) -> Result<Value, GatewayError> {
    Err(compiled_out_error())
}

pub fn extract_image_urls(_body: &Value) -> Result<Vec<String>, GatewayError> {
    Err(compiled_out_error())
}

pub fn build_update_settings_request(_ratio: &str) -> Value {
    serde_json::json!({})
}

pub fn build_generation_request(_prompt: &str, _spec: ChataibotModelSpec) -> Value {
    serde_json::json!({})
}

pub fn estimate_required_quota(_model: &str) -> i64 {
    0
}
