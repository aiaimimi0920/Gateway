use super::test_support::*;
use super::*;
use serde_json::json;

#[test]
fn normalize_image_generations_uses_virtual_model_default() {
    let req = normalize_image_generations(json!({
        "prompt": "draw a tiny banana robot"
    }))
    .unwrap();

    assert_eq!(req.endpoint_kind, EndpointKind::ImagesGenerations);
    assert_eq!(req.requested_model.as_deref(), Some(NANO_BANANA_PRO_MODEL));
    assert_eq!(req.messages[0].text_content(), "draw a tiny banana robot");
}

#[test]
fn normalize_image_edits_collects_prompt_and_images() {
    let req = normalize_image_edits(json!({
        "prompt": "turn this into a watercolor poster",
        "model": "nano-banana-pro",
        "images": [{
            "mime_type": "image/png",
            "base64": "abcd"
        }]
    }))
    .unwrap();

    assert_eq!(req.endpoint_kind, EndpointKind::ImagesEdits);
    assert_eq!(req.messages.len(), 1);
    assert_eq!(req.messages[0].content.len(), 2);
    assert_eq!(
        req.messages[0].text_content(),
        "turn this into a watercolor poster"
    );
}

#[test]
fn normalize_image_edits_requires_input_image() {
    let error = normalize_image_edits(json!({
        "prompt": "turn this into a watercolor poster"
    }))
    .expect_err("missing image uploads should be rejected");

    assert_eq!(error.http_status, Some(400));
    assert_eq!(error.code.as_deref(), Some("missing_input_image"));
    assert_eq!(
        error.message.as_str(),
        "Image edit requests require at least one input image."
    );
}

#[test]
fn runtime_from_payload_reads_required_runtime_fields() {
    let payload = ProviderAccountPayload {
        adapter: "gemini_business_compatible".to_string(),
        base_url: "https://biz-discoveryengine.googleapis.com/v1alpha".to_string(),
        api_key: "jwt".to_string(),
        credential_id: None,
        expires_at: None,
        runtime_state_object_key: None,
        account_name: None,
        execution_mode: None,
        endpoint_execution_modes: None,
        default_model: Some(NANO_BANANA_PRO_MODEL.to_string()),
        headers: std::collections::HashMap::new(),
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
        extra_body: Some(
            [
                ("configId".to_string(), json!("cfg-123")),
                ("session".to_string(), json!("projects/x/sessions/y")),
            ]
            .into_iter()
            .collect(),
        ),
        session_auth: None,
        keepalive: None,
    };

    let runtime = runtime_from_payload(&payload).unwrap();
    assert_eq!(runtime.config_id, "cfg-123");
    assert_eq!(runtime.session, "projects/x/sessions/y");
    assert_eq!(runtime.language_code, DEFAULT_LANGUAGE_CODE);
}

#[test]
fn build_context_file_upload_plan_uses_upload_endpoint() {
    let payload = make_payload(
        "gemini_business_compatible",
        "https://biz-discoveryengine.googleapis.com/v1alpha",
    );
    let runtime = make_runtime();
    let upload = make_upload();
    let plan = build_context_file_upload_plan(
        &payload,
        &runtime,
        &upload,
        EndpointKind::ImagesGenerations,
    );
    assert_request_plan(
        &plan,
        "https://biz-discoveryengine.googleapis.com/v1alpha/locations/global/widgetAddContextFile",
        EndpointKind::ImagesGenerations,
    );
    let body = plan.body.as_ref().expect("upload plan body");
    assert_eq!(body["configId"], "cfg-123");
    assert_eq!(body["additionalParams"]["token"], "-");
    assert_eq!(
        body["addContextFileRequest"]["name"],
        "projects/demo/sessions/abc"
    );
    assert_eq!(body["addContextFileRequest"]["mimeType"], "image/png");
    assert_eq!(body["addContextFileRequest"]["fileContents"], "aGVsbG8=");
    assert!(
        body["addContextFileRequest"]["fileName"]
            .as_str()
            .is_some_and(|value| value.starts_with("upload-") && value.ends_with(".png")),
        "upload plan should synthesize a file name with png extension"
    );
}

#[test]
fn build_stream_assist_plan_uses_stream_assist_endpoint() {
    let payload = make_payload(
        "gemini_business_compatible",
        "https://biz-discoveryengine.googleapis.com/v1alpha",
    );
    let runtime = make_runtime();
    let mut req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesGenerations);
    req.raw_body = json!({
        "prompt": "tiny watercolor cat astronaut"
    });
    let file_ids = vec!["file-1".to_string(), "file-2".to_string()];
    let plan = build_stream_assist_plan(&payload, &req, NANO_BANANA_PRO_MODEL, &runtime, &file_ids)
        .expect("stream assist plan");
    assert_request_plan(
        &plan,
        "https://biz-discoveryengine.googleapis.com/v1alpha/locations/global/widgetStreamAssist",
        EndpointKind::ImagesGenerations,
    );
    let body = plan.body.as_ref().expect("stream assist plan body");
    assert_eq!(body["configId"], "cfg-123");
    assert_eq!(
        body["streamAssistRequest"]["session"],
        "projects/demo/sessions/abc"
    );
    assert_eq!(
        body["streamAssistRequest"]["query"]["parts"][0]["text"],
        "tiny watercolor cat astronaut"
    );
    assert_eq!(
        body["streamAssistRequest"]["fileIds"],
        json!(["file-1", "file-2"])
    );
    assert_eq!(
        body["streamAssistRequest"]["assistGenerationConfig"]["modelId"],
        GEMINI_3_PRO_IMAGE_PREVIEW_MODEL
    );
}

#[test]
fn runtime_from_payload_requires_extra_body_runtime_material() {
    let payload = ProviderAccountPayload {
        adapter: "gemini_business_compatible".to_string(),
        base_url: "https://biz-discoveryengine.googleapis.com/v1alpha".to_string(),
        api_key: "jwt".to_string(),
        credential_id: None,
        expires_at: None,
        runtime_state_object_key: None,
        account_name: None,
        execution_mode: None,
        endpoint_execution_modes: None,
        default_model: Some(NANO_BANANA_PRO_MODEL.to_string()),
        headers: std::collections::HashMap::new(),
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
        extra_body: None,
        session_auth: None,
        keepalive: None,
    };

    let error = runtime_from_payload(&payload)
        .expect_err("missing extra_body runtime material should fail");
    assert_eq!(error.http_status, Some(400));
    assert_eq!(
        error.code.as_deref(),
        Some("missing_gemini_business_runtime")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Business credentials require extra_body runtime material (configId + session)."
    );
}

#[test]
fn runtime_from_payload_requires_config_id() {
    let payload = ProviderAccountPayload {
        adapter: "gemini_business_compatible".to_string(),
        base_url: "https://biz-discoveryengine.googleapis.com/v1alpha".to_string(),
        api_key: "jwt".to_string(),
        credential_id: None,
        expires_at: None,
        runtime_state_object_key: None,
        account_name: None,
        execution_mode: None,
        endpoint_execution_modes: None,
        default_model: Some(NANO_BANANA_PRO_MODEL.to_string()),
        headers: std::collections::HashMap::new(),
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
        extra_body: Some(
            [("session".to_string(), json!("projects/x/sessions/y"))]
                .into_iter()
                .collect(),
        ),
        session_auth: None,
        keepalive: None,
    };

    let error = runtime_from_payload(&payload).expect_err("missing configId should fail");
    assert_eq!(error.http_status, Some(400));
    assert_eq!(
        error.code.as_deref(),
        Some("missing_gemini_business_runtime_field")
    );
    assert_eq!(
        error.message.as_str(),
        "Missing required runtime field `configId`."
    );
}
