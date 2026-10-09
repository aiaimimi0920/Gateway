#[cfg(test)]
use base64::Engine;
#[cfg(test)]
use image::ImageEncoder;
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::error::GatewayError;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
};
use crate::protocol::gemini::canvas_web_reverse as gemini_canvas_modular;
use crate::protocol::gemini_web;
use crate::routing::candidate::ProviderAccountPayload;

mod api_keys;
mod audio_requests;
mod batchexecute;
mod conversation_requests;
mod defaults;
mod errors;
mod frames;
mod media_candidates;
mod media_debug;
mod media_mime;
mod media_responses;
mod media_url_candidates;
mod page_media;
mod prompt_request;
mod request_ids;
mod runtime_options;
mod session_cookies;
mod signaler;
mod stream_request;
mod stream_response;
mod stream_template;
mod template_headers;
mod template_refresh;
mod types;
pub use api_keys::*;
pub use audio_requests::*;
pub use batchexecute::*;
pub use conversation_requests::*;
pub use defaults::*;
pub use errors::*;
#[cfg(test)]
use media_candidates::{
    extract_music_asset_from_candidate_data, extract_video_asset_from_candidate_data,
};
pub use media_responses::*;
pub use page_media::*;
pub use prompt_request::*;
pub use request_ids::*;
pub use runtime_options::*;
pub use session_cookies::*;
pub use signaler::*;
pub use stream_request::*;
pub use stream_response::*;
pub use stream_template::*;
pub use template_headers::*;
pub use template_refresh::*;
pub use types::*;

pub fn runtime_from_payload(
    payload: &ProviderAccountPayload,
) -> Result<GeminiCanvasRuntime, GatewayError> {
    gemini_canvas_modular::runtime_from_payload(payload)
}

pub fn resolve_image_model(model: &str) -> Result<GeminiCanvasImageModel, GatewayError> {
    gemini_canvas_modular::resolve_image_model(model)
}

pub fn resolve_official_image_model(model: &str) -> Result<&'static str, GatewayError> {
    crate::protocol::gemini::api::resolve_official_image_model(model)
}

pub fn resolve_direct_http_image_model(model: &str) -> Result<&'static str, GatewayError> {
    gemini_canvas_modular::resolve_direct_http_image_model(model)
}

pub fn resolve_music_model(model: &str) -> Result<&'static str, GatewayError> {
    gemini_canvas_modular::resolve_music_model(model)
}

pub fn resolve_official_music_model(model: &str) -> Result<&'static str, GatewayError> {
    crate::protocol::gemini::api::resolve_official_music_model(model)
}

pub fn resolve_video_model(model: &str) -> Result<&'static str, GatewayError> {
    gemini_canvas_modular::resolve_video_model(model)
}

pub fn resolve_official_video_model(model: &str) -> Result<&'static str, GatewayError> {
    crate::protocol::gemini::api::resolve_official_video_model(model)
}

pub fn resolve_text_model(model: &str) -> Result<&str, GatewayError> {
    gemini_canvas_modular::resolve_text_model(model)
}

pub fn prompt_from_request(
    req: &CanonicalRelayRequest,
    missing_message: &str,
    missing_code: &str,
) -> Result<String, GatewayError> {
    gemini_canvas_modular::prompt_from_request(req, missing_message, missing_code)
}

pub(crate) fn latest_nonempty_user_text(req: &CanonicalRelayRequest) -> Option<String> {
    gemini_canvas_modular::latest_nonempty_user_text(req)
}

pub fn prompt_for_text_request(
    req: &CanonicalRelayRequest,
    missing_message: &str,
    missing_code: &str,
) -> Result<String, GatewayError> {
    gemini_canvas_modular::prompt_for_text_request(req, missing_message, missing_code)
}

pub fn requested_output_count(req: &CanonicalRelayRequest) -> usize {
    gemini_canvas_modular::requested_output_count(req)
}

pub fn prefers_url_response(req: &CanonicalRelayRequest) -> Result<bool, GatewayError> {
    gemini_canvas_modular::prefers_url_response(req)
}

pub fn aspect_ratio_from_request(req: &CanonicalRelayRequest) -> String {
    gemini_canvas_modular::aspect_ratio_from_request(req)
}

pub fn locale_from_payload(payload: &ProviderAccountPayload) -> String {
    gemini_canvas_modular::locale_from_payload(payload)
}

pub fn build_text_fetch_url(runtime: &GeminiCanvasRuntime, model: &str) -> String {
    gemini_canvas_modular::build_text_fetch_url(runtime, model)
}

#[path = "gemini_canvas_image_edit_uploads.rs"]
mod image_edit_uploads;
pub use image_edit_uploads::extract_image_edit_uploads;
#[cfg(test)]
use image_edit_uploads::normalize_image_edit_upload_to_jpeg;
pub(crate) use image_edit_uploads::normalize_image_edit_uploads;

pub fn build_image_request_body(req: &CanonicalRelayRequest, model: &str) -> Value {
    crate::protocol::gemini::api::build_image_request_body(req, model)
}

pub fn build_direct_http_image_request_body(req: &CanonicalRelayRequest, model: &str) -> Value {
    gemini_canvas_modular::build_direct_http_image_request_body(req, model)
}

pub fn build_imagen_predict_request(req: &CanonicalRelayRequest, prompt: &str) -> Value {
    gemini_canvas_modular::build_imagen_predict_request(req, prompt)
}

pub fn direct_http_image_api_base_url(runtime: &GeminiCanvasRuntime) -> String {
    gemini_canvas_modular::direct_http_image_api_base_url(runtime)
}

pub fn build_music_client_content(prompt: &str) -> Value {
    crate::protocol::gemini::api::build_music_client_content(prompt)
}

pub fn build_music_generation_config(req: &CanonicalRelayRequest) -> Value {
    crate::protocol::gemini::api::build_music_generation_config(req)
}

pub fn build_text_request_body(req: &CanonicalRelayRequest, model: &str) -> Value {
    crate::protocol::gemini::api::build_text_request_body(req, model)
}

pub fn build_tts_request_body(req: &CanonicalRelayRequest, model: &str) -> Value {
    crate::protocol::gemini::api::build_tts_request_body(req, model)
}

pub fn extract_audio_from_generate_content_response(
    body: &Value,
) -> Result<GeminiCanvasAudio, GatewayError> {
    crate::protocol::gemini::api::extract_audio_from_generate_content_response(body)
}

pub fn extract_inline_image_from_generate_content_response(
    body: &Value,
) -> Result<GeminiCanvasImage, GatewayError> {
    crate::protocol::gemini::api::extract_inline_image_from_generate_content_response(body)
}

pub fn requested_tts_response_format(req: &CanonicalRelayRequest) -> Result<String, GatewayError> {
    gemini_canvas_modular::requested_tts_response_format(req)
}

pub fn build_audio_binary_response(
    req: &CanonicalRelayRequest,
    audio: &GeminiCanvasAudio,
) -> Result<(Vec<u8>, String), GatewayError> {
    gemini_canvas_modular::build_audio_binary_response(req, audio)
}

pub fn extract_audio_from_tts_export_response(
    body: &str,
) -> Result<GeminiCanvasAudio, GatewayError> {
    gemini_canvas_modular::extract_audio_from_tts_export_response(body)
}

pub fn extract_stream_generate_locator(
    body: &str,
) -> Result<GeminiCanvasStreamGenerateLocator, GatewayError> {
    gemini_canvas_modular::extract_stream_generate_locator(body)
}

pub fn extract_stream_generate_response_id(body: &str) -> Result<String, GatewayError> {
    gemini_canvas_modular::extract_stream_generate_response_id(body)
}

pub fn extract_conversation_list_entries(body: &str) -> Vec<GeminiCanvasConversationListEntry> {
    gemini_canvas_modular::extract_conversation_list_entries(body)
}

pub fn extract_video_generation_job_id(body: &str) -> Option<String> {
    gemini_canvas_modular::extract_video_generation_job_id(body)
}

pub fn response_indicates_video_generation_pending(body: &str) -> bool {
    gemini_canvas_modular::response_indicates_video_generation_pending(body)
}

pub fn response_indicates_video_generation_quota_reached(body: &str) -> bool {
    gemini_canvas_modular::response_indicates_video_generation_quota_reached(body)
}

pub fn build_openai_images_response_from_urls(
    req: &CanonicalRelayRequest,
    prompt: &str,
    images: &[GeminiCanvasMediaAsset],
) -> Result<Value, GatewayError> {
    gemini_canvas_modular::build_openai_images_response_from_urls(req, prompt, images)
}

pub fn build_openai_images_response_from_bytes(
    req: &CanonicalRelayRequest,
    prompt: &str,
    images: &[GeminiCanvasImage],
) -> Result<Value, GatewayError> {
    gemini_canvas_modular::build_openai_images_response_from_bytes(req, prompt, images)
}

pub fn extract_images_from_imagen_predict_response(
    body: &Value,
) -> Result<Vec<GeminiCanvasImage>, GatewayError> {
    gemini_canvas_modular::extract_images_from_imagen_predict_response(body)
}

#[cfg(test)]
mod tests {
    mod api_keys;
    mod batchexecute;
    mod conversations;
    mod errors;
    mod image_frames;
    mod image_uploads;
    mod media_candidates;
    mod media_responses;
    mod model_requests;
    mod page_media;
    mod prompts;
    mod runtime_options;
    mod session_cookies;
    mod signaler;
    mod stream_requests;
    mod stream_templates;
    mod template_headers;
    mod template_refresh;
    mod tts_requests;
    use super::*;
    use crate::protocol::canonical::{CanonicalTool, CanonicalToolCall};

    fn pure_http_mode_payload(mode: Option<&str>) -> ProviderAccountPayload {
        let mut extra_body = HashMap::new();
        if let Some(value) = mode {
            extra_body.insert("pureHttpMode".to_string(), Value::String(value.to_string()));
        }

        ProviderAccountPayload {
            discovered_protocols: Vec::new(),
            adapter: "gemini_canvas_compatible".to_string(),
            base_url: "https://gemini.google.com".to_string(),
            api_key: String::new(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: Some(
                "credential-runtime/gemini-canvas-profile/test/user-data".to_string(),
            ),
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: Some(GEMINI_CANVAS_DEFAULT_MODEL.to_string()),
            headers: HashMap::new(),
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: None,
            chat_completions_path: None,
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            extra_body: Some(extra_body),
            session_auth: None,
            keepalive: None,
        }
    }

    fn make_request(endpoint_kind: EndpointKind) -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind,
            requested_model: Some(GEMINI_CANVAS_DEFAULT_MODEL.to_string()),
            stream: false,
            messages: vec![],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({ "input": "say hello" }),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }
}
