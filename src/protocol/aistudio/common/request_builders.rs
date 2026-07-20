use serde_json::Value;

use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind};
use crate::protocol::{gemini_api, gemini_canvas};

pub fn build_generate_content_url(base_url: &str, model: &str) -> String {
    format!(
        "{}/models/{}:generateContent",
        base_url.trim_end_matches('/'),
        model
    )
}

pub fn build_text_request_body(req: &CanonicalRelayRequest, model: &str) -> Value {
    gemini_api::pack_gemini(req, model, false)
}

pub fn build_image_request_body(req: &CanonicalRelayRequest, model: &str) -> Value {
    if req.endpoint_kind == EndpointKind::ImagesEdits {
        gemini_canvas::build_image_request_body(req, model)
    } else {
        gemini_canvas::build_direct_http_image_request_body(req, model)
    }
}

pub fn build_tts_request_body(req: &CanonicalRelayRequest, model: &str) -> Value {
    gemini_canvas::build_tts_request_body(req, model)
}
