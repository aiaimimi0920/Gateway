use std::collections::HashMap;

use serde_json::json;

use super::*;
use crate::protocol::canonical::{
    CanonicalMessage, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
};
use crate::protocol::gemini_canvas::{
    GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL, GEMINI_CANVAS_MUSIC_PREVIEW_MODEL,
    GEMINI_CANVAS_VIDEO_PREVIEW_MODEL,
};

#[test]
fn prompt_for_media_request_adds_requested_aspect_ratio() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::VideosGenerations,
        requested_model: Some(GEMINI_CANVAS_VIDEO_PREVIEW_MODEL.to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "cinematic astronaut corgi".to_string(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({
            "prompt": "cinematic astronaut corgi",
            "size": "16:9"
        }),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    };

    let prompt = prompt_for_media_request(&req, GeminiCanvasMediaOperation::Video).unwrap();
    assert!(prompt.contains("Requested aspect ratio: 16:9."));
}

#[test]
fn prompt_for_media_request_prepends_image_style_guidance() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ImagesGenerations,
        requested_model: Some(GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL.to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "一个可爱的日本姑娘".to_string(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({
            "prompt": "一个可爱的日本姑娘",
            "style": "anime style / portrait / soft lighting",
            "size": "1:1"
        }),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    };

    let prompt = prompt_for_media_request(&req, GeminiCanvasMediaOperation::Image).unwrap();
    assert!(prompt.starts_with(
        "Generation guidance:\n- Style guidance: anime style / portrait / soft lighting"
    ));
    assert!(prompt.contains("一个可爱的日本姑娘"));
    assert!(prompt.contains("Requested aspect ratio: 1:1."));
}

#[test]
fn prompt_for_media_request_prepends_image_best_effort_control_fields() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ImagesGenerations,
        requested_model: Some(GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL.to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "一个可爱的日本姑娘".to_string(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({
            "prompt": "一个可爱的日本姑娘",
            "quality": "high",
            "background": "transparent",
            "output_format": "png",
            "output_compression": 85,
            "moderation": "low",
            "input_fidelity": "high"
        }),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    };

    let prompt = prompt_for_media_request(&req, GeminiCanvasMediaOperation::Image).unwrap();
    assert!(prompt.contains("Quality preference: high"));
    assert!(prompt.contains("Background preference: transparent"));
    assert!(prompt.contains("Output format preference: png"));
    assert!(prompt.contains("Output compression preference: 85"));
    assert!(prompt.contains("Moderation preference: low"));
    assert!(prompt.contains("Input fidelity preference: high"));
}

#[test]
fn prompt_for_media_request_prepends_video_best_effort_control_fields() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::VideosGenerations,
        requested_model: Some(GEMINI_CANVAS_VIDEO_PREVIEW_MODEL.to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "海边太阳升起的场景".to_string(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({
            "prompt": "海边太阳升起的场景",
            "negativePrompt": "city skyline, heavy rain",
            "durationSeconds": 8,
            "resolution": "1080p",
            "personGeneration": "allow_adult",
            "generateAudio": true,
            "size": "16:9"
        }),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    };

    let prompt = prompt_for_media_request(&req, GeminiCanvasMediaOperation::Video).unwrap();
    assert!(prompt.contains("Avoid the following elements: city skyline, heavy rain"));
    assert!(prompt.contains("Target duration: 8"));
    assert!(prompt.contains("Target resolution: 1080p"));
    assert!(prompt.contains("People generation preference: allow_adult"));
    assert!(prompt.contains("Audio generation preference: true"));
    assert!(prompt.contains("Requested aspect ratio: 16:9."));
}

#[test]
fn prompt_for_media_request_marks_music_requests_as_generation_intent() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::MusicGenerations,
        requested_model: Some(GEMINI_CANVAS_MUSIC_PREVIEW_MODEL.to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "canvas live ambient piano".to_string(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({
            "prompt": "canvas live ambient piano"
        }),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    };

    let prompt = prompt_for_media_request(&req, GeminiCanvasMediaOperation::Music).unwrap();
    assert!(prompt.starts_with("Create an original music clip that matches this request."));
    assert!(prompt.contains("Music request:\ncanvas live ambient piano"));
}
