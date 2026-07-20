use futures::Stream;
use serde_json::{json, Value};

use crate::error::GatewayError;
use crate::implementation_lines::{compiled_out_error_for_line, RefactoredImplementationLine};
use crate::protocol::canonical::{
    CanonicalRelayRequest, CanonicalRelayResponse, CanonicalToolCall, TokenUsage,
};
use crate::protocol::gemini_canvas::{
    GeminiCanvasAudio, GeminiCanvasImage, GeminiCanvasMediaAsset,
};

pub const GEMINI_API_MODULAR_ADAPTER: &str = "gemini_api_modular_compatible";
pub const GEMINI_API_MODULAR_PROFILE: &str = "aistudio_official_api";

fn compiled_out_error() -> GatewayError {
    compiled_out_error_for_line(RefactoredImplementationLine::AIStudioOfficial)
        .with_provider(GEMINI_API_MODULAR_ADAPTER)
}

pub fn supports_endpoint(_endpoint_kind: crate::protocol::canonical::EndpointKind) -> bool {
    false
}

pub fn pack_request(req: &CanonicalRelayRequest, model: &str, stream: bool) -> Value {
    json!({
        "model": model,
        "stream": stream,
        "messages": req.messages,
    })
}

pub fn normalize_generate_content(
    _body: Value,
    _path_model: Option<String>,
    _stream: bool,
) -> Result<CanonicalRelayRequest, GatewayError> {
    Err(compiled_out_error())
}

pub fn build_generate_content_success(
    model: &str,
    text: &str,
    usage: Option<&TokenUsage>,
    tool_calls: &[CanonicalToolCall],
    finish_reason: Option<&str>,
) -> Value {
    json!({
        "model": model,
        "text": text,
        "usage": usage,
        "tool_calls": tool_calls,
        "finish_reason": finish_reason,
    })
}

pub fn parse_response(body: &Value, model: &str) -> Result<CanonicalRelayResponse, GatewayError> {
    Ok(CanonicalRelayResponse {
        model: model.to_string(),
        text: body
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        usage: None,
        tool_calls: Vec::new(),
        upstream_status: None,
        finish_reason: None,
    })
}

pub async fn accumulate_gemini_stream(
    _response: rquest::Response,
    _model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    Err(compiled_out_error())
}

pub fn translate_openai_sse_to_gemini_stream(
    inner: impl Stream<Item = Result<bytes::Bytes, rquest::Error>> + Send + 'static,
    _model: String,
) -> impl Stream<Item = Result<bytes::Bytes, rquest::Error>> + Send + 'static {
    inner
}

pub fn default_path(model: &str, stream: bool) -> String {
    if stream {
        format!("/models/{model}:streamGenerateContent")
    } else {
        format!("/models/{model}:generateContent")
    }
}

pub fn default_query(stream: bool) -> Vec<(String, String)> {
    stream
        .then(|| vec![("alt".to_string(), "sse".to_string())])
        .unwrap_or_default()
}

pub fn resolve_official_image_model(_model: &str) -> Result<&'static str, GatewayError> {
    Err(compiled_out_error())
}

pub fn resolve_official_music_model(_model: &str) -> Result<&'static str, GatewayError> {
    Err(compiled_out_error())
}

pub fn resolve_official_video_model(_model: &str) -> Result<&'static str, GatewayError> {
    Err(compiled_out_error())
}

pub fn build_text_request_body(req: &CanonicalRelayRequest, model: &str) -> Value {
    pack_request(req, model, false)
}

pub fn build_tts_request_body(req: &CanonicalRelayRequest, model: &str) -> Value {
    build_text_request_body(req, model)
}

pub fn build_image_request_body(req: &CanonicalRelayRequest, model: &str) -> Value {
    build_text_request_body(req, model)
}

pub fn requested_output_count(_req: &CanonicalRelayRequest) -> usize {
    1
}

pub fn video_aspect_ratio_from_request(_req: &CanonicalRelayRequest) -> String {
    "16:9".to_string()
}

pub fn build_music_client_content(prompt: &str) -> Value {
    json!({
        "weightedPrompts": [{
            "text": prompt,
            "weight": 1.0
        }]
    })
}

pub fn build_music_generation_config(_req: &CanonicalRelayRequest) -> Value {
    json!({})
}

pub fn extract_audio_from_generate_content_response(
    _body: &Value,
) -> Result<GeminiCanvasAudio, GatewayError> {
    Err(compiled_out_error())
}

pub fn extract_inline_image_from_generate_content_response(
    _body: &Value,
) -> Result<GeminiCanvasImage, GatewayError> {
    Err(compiled_out_error())
}

pub fn build_audio_binary_response(
    _req: &CanonicalRelayRequest,
    _audio: &GeminiCanvasAudio,
) -> Result<(Vec<u8>, String), GatewayError> {
    Err(compiled_out_error())
}

pub fn build_openai_images_response_from_bytes(
    _req: &CanonicalRelayRequest,
    _prompt: &str,
    _images: &[GeminiCanvasImage],
) -> Result<Value, GatewayError> {
    Err(compiled_out_error())
}

pub fn build_music_generation_response(
    model: &str,
    prompt: &str,
    asset: &GeminiCanvasMediaAsset,
    body_text: Option<&str>,
) -> Value {
    json!({
        "object": "music.generation",
        "provider": "google_gemini_api",
        "model": model,
        "prompt": prompt,
        "message": body_text,
        "data": [{
            "kind": asset.kind,
            "url": asset.url,
            "mime_type": asset.mime_type,
            "alt": asset.alt,
            "width": asset.width,
            "height": asset.height,
            "duration_seconds": asset.duration_seconds,
            "body_base64": asset.body_base64,
        }],
    })
}

pub fn build_music_generation_accepted_response(
    model: &str,
    prompt: &str,
    conversation_id: Option<&str>,
    response_id: Option<&str>,
    app_path: Option<&str>,
    duration_seconds: Option<f64>,
    body_text: Option<&str>,
) -> Value {
    json!({
        "object": "music.generation",
        "provider": "google_gemini_api",
        "accepted": true,
        "completed": false,
        "model": model,
        "prompt": prompt,
        "conversation_id": conversation_id,
        "response_id": response_id,
        "app_path": app_path,
        "duration_seconds": duration_seconds,
        "message": body_text,
    })
}

pub fn build_video_generation_response(
    model: &str,
    prompt: &str,
    asset: &GeminiCanvasMediaAsset,
    body_text: Option<&str>,
) -> Value {
    json!({
        "object": "video.generation",
        "provider": "google_gemini_api",
        "model": model,
        "prompt": prompt,
        "message": body_text,
        "data": [{
            "kind": asset.kind,
            "url": asset.url,
            "mime_type": asset.mime_type,
            "alt": asset.alt,
            "width": asset.width,
            "height": asset.height,
            "duration_seconds": asset.duration_seconds,
            "body_base64": asset.body_base64,
        }],
    })
}

pub fn build_video_generation_accepted_response(
    model: &str,
    prompt: &str,
    conversation_id: Option<&str>,
    response_id: Option<&str>,
    app_path: Option<&str>,
    job_id: Option<&str>,
    body_text: Option<&str>,
) -> Value {
    json!({
        "object": "video.generation",
        "provider": "google_gemini_api",
        "accepted": true,
        "completed": false,
        "model": model,
        "prompt": prompt,
        "conversation_id": conversation_id,
        "response_id": response_id,
        "app_path": app_path,
        "job_id": job_id,
        "message": body_text,
    })
}
