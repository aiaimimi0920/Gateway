use super::*;
use serde_json::json;

#[test]
fn extract_generated_files_requires_generated_files() {
    let error = extract_generated_files(&[json!({})], "projects/demo/sessions/fallback")
        .expect_err("missing generated files should fail");
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_business_compatible")
    );
    assert_eq!(error.code.as_deref(), Some("gemini_business_no_images"));
    assert_eq!(
        error.message.as_str(),
        "Gemini Business image request completed without any generated files."
    );
}

#[test]
fn extract_generated_files_preserves_rate_limited_contract() {
    let error = extract_generated_files(
        &[json!({
            "error": {
                "message": "too many requests",
                "code": 429
            }
        })],
        "projects/demo/sessions/fallback",
    )
    .expect_err("429 error should surface as rate limited");
    assert_eq!(error.http_status, Some(429));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_business_compatible")
    );
    assert_eq!(error.code.as_deref(), Some("gemini_business_rate_limited"));
    assert_eq!(error.message.as_str(), "too many requests");
}

#[test]
fn extract_generated_files_preserves_generic_upstream_error_contract() {
    let error = extract_generated_files(
        &[json!({
            "error": {
                "message": "internal upstream failure",
                "code": 500
            }
        })],
        "projects/demo/sessions/fallback",
    )
    .expect_err("non-429 upstream errors should surface as generic upstream errors");
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_business_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_business_upstream_error")
    );
    assert_eq!(error.message.as_str(), "internal upstream failure");
}

#[test]
fn extract_uploads_from_request_body_rejects_non_object_images() {
    let error = extract_uploads_from_request_body(&json!({
        "images": ["not-an-object"]
    }))
    .expect_err("non-object uploads should fail");
    assert_eq!(error.http_status, Some(400));
    assert_eq!(error.code.as_deref(), Some("invalid_image_upload"));
    assert_eq!(
        error.message.as_str(),
        "Gemini Business image uploads must be JSON objects."
    );
}

#[test]
fn normalize_image_generations_requires_prompt_field() {
    let error =
        normalize_image_generations(json!({})).expect_err("missing prompt field should fail");
    assert_eq!(error.http_status, Some(400));
    assert_eq!(error.code.as_deref(), Some("missing_required_field"));
    assert_eq!(error.message.as_str(), "Missing required field `prompt`.");
}

#[test]
fn prompt_from_request_requires_prompt() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ImagesGenerations,
        requested_model: Some(NANO_BANANA_PRO_MODEL.to_string()),
        stream: false,
        messages: Vec::new(),
        tools: Vec::new(),
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({}),
        previous_response_id: None,
        explicit_session_key: None,
        extra: std::collections::HashMap::new(),
    };

    let error = prompt_from_request(&req).expect_err("missing prompt should fail");
    assert_eq!(error.http_status, Some(400));
    assert_eq!(error.code.as_deref(), Some("missing_prompt"));
    assert_eq!(
        error.message.as_str(),
        "Gemini Business image requests require a prompt."
    );
}

#[test]
fn response_format_rejects_non_string_values() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ImagesGenerations,
        requested_model: Some(NANO_BANANA_PRO_MODEL.to_string()),
        stream: false,
        messages: Vec::new(),
        tools: Vec::new(),
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({
            "response_format": 123
        }),
        previous_response_id: None,
        explicit_session_key: None,
        extra: std::collections::HashMap::new(),
    };

    let error = response_format(&req).expect_err("non-string response_format should fail");
    assert_eq!(error.http_status, Some(400));
    assert_eq!(error.code.as_deref(), Some("invalid_image_response_format"));
    assert_eq!(
        error.message.as_str(),
        "response_format must be a string when provided."
    );
}

#[test]
fn response_format_rejects_unsupported_string_values() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ImagesGenerations,
        requested_model: Some(NANO_BANANA_PRO_MODEL.to_string()),
        stream: false,
        messages: Vec::new(),
        tools: Vec::new(),
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({
            "response_format": "json"
        }),
        previous_response_id: None,
        explicit_session_key: None,
        extra: std::collections::HashMap::new(),
    };

    let error = response_format(&req).expect_err("unsupported response_format should fail");
    assert_eq!(error.http_status, Some(400));
    assert_eq!(
        error.code.as_deref(),
        Some("unsupported_image_response_format")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Business image endpoints currently support response_format=b64_json or url."
    );
}

#[test]
fn extract_generated_files_deduplicates_file_ids() {
    let (session, files) = extract_generated_files(
            &[json!({
                "streamAssistResponse": {
                    "sessionInfo": { "session": "projects/demo/sessions/123" },
                    "answer": {
                        "replies": [
                            { "groundedContent": { "content": { "file": { "fileId": "file-1", "mimeType": "image/png" } } } },
                            { "groundedContent": { "content": { "file": { "fileId": "file-1", "mimeType": "image/png" } } } }
                        ]
                    }
                }
            })],
            "projects/demo/sessions/fallback",
        )
        .unwrap();

    assert_eq!(session, "projects/demo/sessions/123");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].file_id, "file-1");
}

#[test]
fn build_openai_images_response_uses_b64_by_default() {
    let req = normalize_image_generations(json!({
        "prompt": "banana"
    }))
    .unwrap();

    let body =
        build_openai_images_response(&req, "banana", &[("image/png".to_string(), vec![1, 2, 3])])
            .unwrap();

    assert!(body["data"][0].get("b64_json").is_some());
    assert_eq!(body["data"][0]["mime_type"], "image/png");
}

#[test]
fn resolve_image_model_maps_virtual_alias() {
    assert_eq!(
        resolve_image_model(NANO_BANANA_PRO_MODEL),
        GEMINI_3_PRO_IMAGE_PREVIEW_MODEL
    );
    assert_eq!(
        resolve_image_model(GEMINI_3_PRO_IMAGE_PREVIEW_MODEL),
        GEMINI_3_PRO_IMAGE_PREVIEW_MODEL
    );
}
