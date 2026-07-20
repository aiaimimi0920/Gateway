use serde_json::Value;

use crate::error::GatewayError;

mod models;
mod multipart;
mod request;
mod response;

pub use models::{
    free_tier_model_ids, resolve_model_spec, supported_model_ids, ChataibotModelSpec,
    CHATAIBOT_DEFAULT_MODEL,
};
pub use multipart::{build_edit_multipart_body, build_merge_multipart_body, MultipartBody};
pub use request::{
    aspect_ratio_from_request, extract_uploads_from_request_body, prefers_url_response,
    prompt_from_request, requested_image_count, ChataibotUpload,
};
pub use response::{
    build_openai_images_response_from_bytes, build_openai_images_response_from_urls,
    extract_image_urls,
};

pub const CHATAIBOT_QUOTA_PROBE_PATH: &str = "/api/user/answers-count/v2";

const DEFAULT_RATIO: &str = "auto";
const DEFAULT_LANGUAGE: &str = "en";
const DEFAULT_FROM: &str = "1";
const DEFAULT_CHAT_CONTEXT_ID: &str = "-2";

pub fn build_update_settings_request(ratio: &str) -> Value {
    serde_json::json!({
        "settings": {
            "imageAspectRatio": ratio,
        },
    })
}

pub fn build_generation_request(prompt: &str, spec: ChataibotModelSpec) -> Value {
    let mut body = serde_json::json!({
        "text": prompt,
        "from": 1,
        "generationType": spec.provider,
        "isInternational": true,
    });

    if let Some(version) = spec.version {
        if let Some(object) = body.as_object_mut() {
            object.insert("version".to_string(), Value::String(version.to_string()));
        }
    }

    body
}

pub fn estimate_required_quota(model: &str) -> i64 {
    match model.trim() {
        "gpt-image-1.5" => 12,
        "gpt-image-1.5-high" => 40,
        "ideogram" => 8,
        "google-nano-banana-pro" => 60,
        "google-nano-banana" => 15,
        "google-nano-banana-2" => 30,
        "midjourney-7" => 20,
        "qwen-lora" => 2,
        "bytedance-seedream" => 14,
        _ => 1,
    }
}

pub(super) fn chataibot_error(message: &str, code: &str) -> GatewayError {
    GatewayError::server_error(message)
        .with_provider("chataibot_compatible")
        .with_code(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::{
        CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole,
        ProtocolFamily,
    };
    use serde_json::json;
    use std::collections::HashMap;

    fn make_request(raw_body: Value) -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ImagesGenerations,
            requested_model: Some("google-nano-banana".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "merge them".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body,
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }

    #[test]
    fn resolve_model_spec_supports_google_virtual_models() {
        let spec = resolve_model_spec("google-nano-banana-pro").unwrap();
        assert_eq!(spec.provider, "GOOGLE");
        assert_eq!(spec.version, Some("nano-banana-pro"));
        assert_eq!(spec.generation_cost, 60);
        assert!(spec.edit_mode.is_none());
    }

    #[test]
    fn default_model_tracks_free_tier_visible_google_model() {
        assert_eq!(CHATAIBOT_DEFAULT_MODEL, "google-nano-banana-2");
    }

    #[test]
    fn free_tier_model_ids_pin_generation_safe_catalog() {
        assert_eq!(
            free_tier_model_ids(),
            &["qwen-lora", "google-nano-banana-2", "gpt-image-1.5"]
        );
    }

    #[test]
    fn extract_uploads_supports_data_uri_strings() {
        let uploads = extract_uploads_from_request_body(&json!({
            "image": "data:image/png;base64,aGVsbG8="
        }))
        .unwrap();
        assert_eq!(uploads.len(), 1);
        assert_eq!(uploads[0].mime_type, "image/png");
        assert_eq!(uploads[0].base64_data, "aGVsbG8=");
    }

    #[test]
    fn extract_uploads_supports_object_uploads() {
        let uploads = extract_uploads_from_request_body(&json!({
            "images": [{
                "mime_type": "image/jpeg",
                "base64": "aGVsbG8="
            }]
        }))
        .unwrap();
        assert_eq!(uploads.len(), 1);
        assert_eq!(uploads[0].mime_type, "image/jpeg");
        assert_eq!(uploads[0].base64_data, "aGVsbG8=");
    }

    #[test]
    fn aspect_ratio_maps_openai_sizes_to_supported_ratios() {
        let req = make_request(json!({ "prompt": "banana", "size": "1792x1024" }));
        assert_eq!(aspect_ratio_from_request(&req), "16:9");
    }

    #[test]
    fn prefers_url_response_by_default() {
        let req = make_request(json!({ "prompt": "banana" }));
        assert!(prefers_url_response(&req).unwrap());
    }

    #[test]
    fn build_generation_request_includes_provider_and_version() {
        let request = build_generation_request(
            "banana on the moon",
            resolve_model_spec("google-nano-banana").unwrap(),
        );
        assert_eq!(request.get("generationType"), Some(&json!("GOOGLE")));
        assert_eq!(request.get("version"), Some(&json!("nano-banana")));
    }

    #[test]
    fn estimate_required_quota_tracks_free_tier_models() {
        assert_eq!(estimate_required_quota("qwen-lora"), 2);
        assert_eq!(estimate_required_quota("google-nano-banana-2"), 30);
        assert_eq!(estimate_required_quota("gpt-image-1.5"), 12);
    }

    #[test]
    fn build_openai_images_response_from_urls_uses_requested_count() {
        let req = make_request(json!({
            "prompt": "banana",
            "n": 1,
        }));
        let response = build_openai_images_response_from_urls(
            &req,
            "banana",
            &[
                "https://img.example.com/1.png".to_string(),
                "https://img.example.com/2.png".to_string(),
            ],
        )
        .unwrap();

        assert_eq!(
            response
                .get("data")
                .and_then(|v| v.as_array())
                .map(Vec::len),
            Some(1)
        );
    }
}
