use super::*;

#[test]
fn build_text_stream_generate_heavy_request_uses_expected_slots() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: Some(GEMINI_CANVAS_DEFAULT_TEXT_MODEL.to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "Reply with exactly: heavy ok".to_string(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({}),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    };
    let bootstrap = gemini_web::GeminiWebBootstrap {
        access_token: Some("at-1".to_string()),
        build_label: Some("bl-1".to_string()),
        session_id: Some("sid-1".to_string()),
        language: "zh-CN".to_string(),
        push_id: None,
        client_pctx: None,
        app_page_path: None,
    };
    let request_uuid = "CDCF4DAC-9B46-4912-9D99-D52134A006A5";

    let request = build_text_stream_generate_heavy_request(&req, &bootstrap, request_uuid).unwrap();
    assert!(request
        .query
        .iter()
        .any(|(key, value)| key == "bl" && value == "bl-1"));
    assert!(request
        .query
        .iter()
        .any(|(key, value)| key == "f.sid" && value == "sid-1"));
    assert!(!request.form.iter().any(|(key, _)| key == "at"));

    let f_req = request
        .form
        .iter()
        .find(|(key, _)| key == "f.req")
        .map(|(_, value)| value)
        .unwrap();
    let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
    let inner = serde_json::from_str::<Vec<Value>>(outer[1].as_str().unwrap()).unwrap();
    assert_eq!(inner.len(), 80);
    assert_eq!(
        inner[0]
            .as_array()
            .and_then(|items| items.first())
            .and_then(Value::as_str),
        Some("Reply with exactly: heavy ok")
    );
    assert_eq!(
        inner[1]
            .as_array()
            .and_then(|items| items.first())
            .and_then(Value::as_str),
        Some("zh-CN")
    );
    assert_eq!(
        inner[3].as_str(),
        Some(GEMINI_CANVAS_TEXT_STREAM_GENERATE_OPAQUE_STATE)
    );
    assert_eq!(
        inner[6]
            .as_array()
            .and_then(|items| items.first())
            .and_then(Value::as_i64),
        Some(0)
    );
    assert_eq!(
        inner[41]
            .as_array()
            .and_then(|items| items.first())
            .and_then(Value::as_i64),
        Some(2)
    );
    assert_eq!(
        inner[49].as_i64(),
        Some(GEMINI_CANVAS_TEXT_STREAM_GENERATE_TEXT_MODE_INDEX)
    );
    assert_eq!(inner[59].as_str(), Some(request_uuid));
    assert_eq!(inner[68].as_i64(), Some(1));
    assert_eq!(inner[79].as_i64(), Some(1));
    let request_hex = inner[4].as_str().unwrap();
    assert_eq!(request_hex.len(), 32);
    assert!(request_hex.chars().all(|ch| ch.is_ascii_hexdigit()));
}

#[test]
fn build_stream_generate_heavy_request_sets_requested_mode_index() {
    let _req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: Some(GEMINI_CANVAS_DEFAULT_TEXT_MODEL.to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "Reply with exactly: media ok".to_string(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({}),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    };
    let bootstrap = gemini_web::GeminiWebBootstrap {
        access_token: Some("at-1".to_string()),
        build_label: Some("bl-1".to_string()),
        session_id: Some("sid-1".to_string()),
        language: "en-US".to_string(),
        push_id: None,
        client_pctx: None,
        app_page_path: None,
    };

    let request = build_stream_generate_heavy_request(
        "Reply with exactly: media ok",
        &bootstrap,
        "CDCF4DAC-9B46-4912-9D99-D52134A006A5",
        GEMINI_CANVAS_STREAM_GENERATE_VIDEO_MODE_INDEX,
    )
    .unwrap();
    let f_req = request
        .form
        .iter()
        .find(|(key, _)| key == "f.req")
        .map(|(_, value)| value)
        .unwrap();
    let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
    let inner = serde_json::from_str::<Vec<Value>>(outer[1].as_str().unwrap()).unwrap();
    assert_eq!(
        inner[49].as_i64(),
        Some(GEMINI_CANVAS_STREAM_GENERATE_VIDEO_MODE_INDEX)
    );
    assert_eq!(
        inner[3].as_str(),
        Some(GEMINI_CANVAS_MEDIA_STREAM_GENERATE_OPAQUE_STATE)
    );
    assert_eq!(
        inner[6]
            .as_array()
            .and_then(|items| items.first())
            .and_then(Value::as_i64),
        Some(1)
    );
    assert_eq!(inner[68].as_i64(), Some(2));
}

#[test]
fn build_stream_generate_image_request_uses_image_mode_shape() {
    let bootstrap = gemini_web::GeminiWebBootstrap {
        access_token: Some("at-1".to_string()),
        build_label: Some("bl-1".to_string()),
        session_id: Some("sid-1".to_string()),
        language: "zh-CN".to_string(),
        push_id: None,
        client_pctx: None,
        app_page_path: None,
    };

    let request = build_stream_generate_heavy_request(
        "Reply with exactly: image mode ok",
        &bootstrap,
        "3FCBC5C2-4C8B-468E-B715-B5392A5AED35",
        GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX,
    )
    .unwrap();
    let f_req = request
        .form
        .iter()
        .find(|(key, _)| key == "f.req")
        .map(|(_, value)| value)
        .unwrap();
    let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
    let inner = serde_json::from_str::<Vec<Value>>(outer[1].as_str().unwrap()).unwrap();
    assert_eq!(
        inner[6]
            .as_array()
            .and_then(|items| items.first())
            .and_then(Value::as_i64),
        Some(0)
    );
    assert_eq!(
        inner[49].as_i64(),
        Some(GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX)
    );
    assert_eq!(
        inner[3].as_str(),
        Some(GEMINI_CANVAS_IMAGE_STREAM_GENERATE_OPAQUE_STATE)
    );
    assert_eq!(inner[68].as_i64(), Some(1));
    assert_eq!(inner[79].as_i64(), Some(1));
}

#[test]
fn build_stream_generate_image_edit_request_includes_uploaded_refs() {
    let bootstrap = gemini_web::GeminiWebBootstrap {
        access_token: Some("at-1".to_string()),
        build_label: Some("bl-1".to_string()),
        session_id: Some("sid-1".to_string()),
        language: "zh-CN".to_string(),
        push_id: Some("push-1".to_string()),
        client_pctx: Some("pctx-1".to_string()),
        app_page_path: None,
    };

    let request = build_stream_generate_heavy_request_with_uploaded_files(
        "把上传的样例图编辑成一个霓虹徽章",
        &bootstrap,
        "3FCBC5C2-4C8B-468E-B715-B5392A5AED35",
        GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX,
        &[GeminiCanvasUploadedFileRef {
            resource_path: "/contrib_service/ttl_1d/example".to_string(),
            mime_type: "image/jpeg".to_string(),
            file_name: "edit-source.jpg".to_string(),
        }],
    )
    .unwrap();
    let f_req = request
        .form
        .iter()
        .find(|(key, _)| key == "f.req")
        .map(|(_, value)| value)
        .unwrap();
    let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
    let inner = serde_json::from_str::<Vec<Value>>(outer[1].as_str().unwrap()).unwrap();
    assert_eq!(
        inner[0][3][0][0][0].as_str(),
        Some("/contrib_service/ttl_1d/example")
    );
    assert_eq!(inner[0][3][0][0][1].as_i64(), Some(1));
    assert_eq!(inner[0][3][0][0][3].as_str(), Some("image/jpeg"));
    assert_eq!(inner[0][3][0][1].as_str(), Some("edit-source.jpg"));
    assert_eq!(inner[0][3][0][8][0].as_i64(), Some(0));
    assert_eq!(inner[6][0].as_i64(), Some(1));
    assert_eq!(inner[68].as_i64(), Some(2));
}

#[test]
fn build_stream_generate_image_edit_request_uses_seeded_state_when_provided() {
    let bootstrap = gemini_web::GeminiWebBootstrap {
        access_token: Some("at-1".to_string()),
        build_label: Some("bl-1".to_string()),
        session_id: Some("sid-1".to_string()),
        language: "zh-CN".to_string(),
        push_id: Some("push-1".to_string()),
        client_pctx: Some("pctx-1".to_string()),
        app_page_path: None,
    };
    let seed = GeminiCanvasStreamGenerateSeed {
        opaque_state: Some("seed-opaque-state".to_string()),
        request_hex: Some("0123456789abcdef0123456789abcdef".to_string()),
        request_uuid: Some("seed-request-id".to_string()),
    };

    let request = build_stream_generate_heavy_request_with_uploaded_files_seeded(
        "把上传的样例图编辑成一个霓虹徽章",
        &bootstrap,
        "seed-request-id",
        GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX,
        &[GeminiCanvasUploadedFileRef {
            resource_path: "/contrib_service/ttl_1d/example".to_string(),
            mime_type: "image/jpeg".to_string(),
            file_name: "edit-source.jpg".to_string(),
        }],
        Some(&seed),
    )
    .unwrap();
    let f_req = request
        .form
        .iter()
        .find(|(key, _)| key == "f.req")
        .map(|(_, value)| value)
        .unwrap();
    let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
    let inner = serde_json::from_str::<Vec<Value>>(outer[1].as_str().unwrap()).unwrap();
    assert_eq!(inner[3].as_str(), Some("seed-opaque-state"));
    assert_eq!(inner[4].as_str(), Some("0123456789abcdef0123456789abcdef"));
    assert_eq!(inner[59].as_str(), Some("seed-request-id"));
    assert_eq!(inner[6][0].as_i64(), Some(1));
    assert_eq!(inner[68].as_i64(), Some(2));
}

#[test]
fn build_stream_generate_text_request_uses_text_mode_shape() {
    let bootstrap = gemini_web::GeminiWebBootstrap {
        access_token: Some("at-1".to_string()),
        build_label: Some("bl-1".to_string()),
        session_id: Some("sid-1".to_string()),
        language: "en-US".to_string(),
        push_id: None,
        client_pctx: None,
        app_page_path: None,
    };

    let request = build_stream_generate_heavy_request(
        "Reply with exactly: pure http smoke ok",
        &bootstrap,
        "CDCF4DAC-9B46-4912-9D99-D52134A006A5",
        GEMINI_CANVAS_TEXT_STREAM_GENERATE_TEXT_MODE_INDEX,
    )
    .unwrap();
    let f_req = request
        .form
        .iter()
        .find(|(key, _)| key == "f.req")
        .map(|(_, value)| value)
        .unwrap();
    let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
    let inner = serde_json::from_str::<Vec<Value>>(outer[1].as_str().unwrap()).unwrap();
    assert_eq!(
        inner[6]
            .as_array()
            .and_then(|items| items.first())
            .and_then(Value::as_i64),
        Some(0)
    );
    assert_eq!(
        inner[3].as_str(),
        Some(GEMINI_CANVAS_TEXT_STREAM_GENERATE_OPAQUE_STATE)
    );
    assert_eq!(
        inner[41]
            .as_array()
            .and_then(|items| items.first())
            .and_then(Value::as_i64),
        Some(2)
    );
    assert_eq!(
        inner[49].as_i64(),
        Some(GEMINI_CANVAS_TEXT_STREAM_GENERATE_TEXT_MODE_INDEX)
    );
    assert_eq!(inner[68].as_i64(), Some(1));
    assert_eq!(inner[79].as_i64(), Some(1));
}
