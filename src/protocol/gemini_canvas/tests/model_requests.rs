use super::*;

#[test]
fn normalize_video_generations_defaults_model() {
    let req = normalize_video_generations(json!({
        "prompt": "a corgi running on the moon"
    }))
    .unwrap();

    assert_eq!(req.endpoint_kind, EndpointKind::VideosGenerations);
    assert_eq!(
        req.requested_model.as_deref(),
        Some(GEMINI_CANVAS_VIDEO_PREVIEW_MODEL)
    );
    assert_eq!(
        req.messages[0].text_content(),
        "a corgi running on the moon"
    );
}

#[test]
fn resolve_music_model_accepts_preview_id() {
    assert_eq!(
        resolve_music_model(GEMINI_CANVAS_MUSIC_PREVIEW_MODEL).unwrap(),
        GEMINI_CANVAS_MUSIC_PREVIEW_MODEL
    );
}

#[test]
fn resolve_official_media_models_map_canvas_aliases() {
    assert_eq!(
        resolve_official_image_model(GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL).unwrap(),
        GEMINI_CANVAS_OFFICIAL_IMAGE_MODEL
    );
    assert_eq!(
        resolve_official_image_model(GEMINI_31_FLASH_IMAGE_PREVIEW_MODEL).unwrap(),
        GEMINI_CANVAS_OFFICIAL_IMAGE_MODEL_PREVIEW
    );
    assert_eq!(
        resolve_official_music_model(GEMINI_CANVAS_MUSIC_PREVIEW_MODEL).unwrap(),
        GEMINI_CANVAS_OFFICIAL_MUSIC_MODEL
    );
    assert_eq!(
        resolve_official_video_model(GEMINI_CANVAS_VIDEO_PREVIEW_MODEL).unwrap(),
        GEMINI_CANVAS_OFFICIAL_VIDEO_MODEL
    );
}

#[test]
fn resolve_direct_http_image_model_preserves_canvas_preview_aliases() {
    assert_eq!(
        resolve_direct_http_image_model(GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL).unwrap(),
        GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL
    );
    assert_eq!(
        resolve_direct_http_image_model(GEMINI_25_FLASH_IMAGE_MODEL).unwrap(),
        GEMINI_25_FLASH_IMAGE_MODEL
    );
    assert_eq!(
        resolve_direct_http_image_model(GEMINI_31_FLASH_IMAGE_PREVIEW_MODEL).unwrap(),
        GEMINI_31_FLASH_IMAGE_PREVIEW_MODEL
    );
}

#[test]
fn build_direct_http_image_request_body_uses_text_and_image_modalities() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ImagesGenerations,
        requested_model: Some(GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL.to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "paint a skyline".to_string(),
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
            "model": GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL,
            "prompt": "paint a skyline",
            "response_format": "url",
            "size": "1024x1024",
        }),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    };

    let body = build_direct_http_image_request_body(&req, GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL);
    assert_eq!(
        body["generationConfig"]["responseModalities"],
        json!(["TEXT", "IMAGE"])
    );
    assert_eq!(
        body["generationConfig"]["imageConfig"]["aspectRatio"],
        json!("1:1")
    );
    assert!(body.get("model").is_none());
}

#[test]
fn build_image_request_body_rewrites_data_url_inputs_to_inline_data() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ImagesEdits,
        requested_model: Some(GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL.to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![
                ContentPart::ImageUrl {
                    image_url: "data:image/png;base64,aGVsbG8=".to_string(),
                    detail: None,
                },
                ContentPart::Text {
                    text: "edit the sample".to_string(),
                },
            ],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({
            "model": GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL,
            "prompt": "edit the sample",
            "images": [{
                "mime_type": "image/png",
                "base64": "aGVsbG8="
            }],
            "response_format": "url",
        }),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    };

    let body = build_image_request_body(&req, GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL);
    assert_eq!(
        body["contents"][0]["parts"][0]["inlineData"]["mimeType"],
        json!("image/png")
    );
    assert_eq!(
        body["contents"][0]["parts"][0]["inlineData"]["data"],
        json!("aGVsbG8=")
    );
    assert!(body["contents"][0]["parts"][0].get("fileData").is_none());
    assert_eq!(body["contents"][0]["parts"][1]["text"], "edit the sample");
}

#[test]
fn build_imagen_predict_request_uses_prompt_and_aspect_ratio() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ImagesGenerations,
        requested_model: Some(GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL.to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "paint a skyline".to_string(),
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
            "model": GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL,
            "prompt": "paint a skyline",
            "response_format": "url",
            "size": "1536x1024",
            "n": 2,
        }),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    };

    let body = build_imagen_predict_request(&req, "paint a skyline");
    assert_eq!(body["instances"][0]["prompt"], "paint a skyline");
    assert_eq!(body["parameters"]["sampleCount"], 2);
    assert_eq!(body["parameters"]["aspectRatio"], "3:2");
}

#[test]
fn direct_http_image_api_base_url_prefers_clients6_for_google_api_default() {
    let runtime = GeminiCanvasRuntime {
        runtime_state_object_key: "credential-runtime/gemini-canvas/example.json".to_string(),
        share_id: "share-demo".to_string(),
        api_base_url: GEMINI_CANVAS_DEFAULT_API_BASE_URL.to_string(),
    };
    assert_eq!(
        direct_http_image_api_base_url(&runtime),
        GEMINI_CANVAS_DIRECT_HTTP_IMAGE_API_BASE_URL
    );

    let custom_runtime = GeminiCanvasRuntime {
        runtime_state_object_key: runtime.runtime_state_object_key,
        share_id: runtime.share_id,
        api_base_url: "http://localhost:4219/v1beta".to_string(),
    };
    assert_eq!(
        direct_http_image_api_base_url(&custom_runtime),
        "http://localhost:4219/v1beta"
    );
}
