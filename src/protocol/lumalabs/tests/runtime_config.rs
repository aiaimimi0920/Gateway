use super::*;

#[test]
fn runtime_reads_realm_id_from_payload_extra_body() {
    let payload = ProviderAccountPayload {
        discovered_protocols: Vec::new(),
        adapter: "lumalabs_compatible".to_string(),
        base_url: "https://app.lumalabs.ai".to_string(),
        api_key: "wos-session".to_string(),
        credential_id: None,
        expires_at: None,
        runtime_state_object_key: None,
        account_name: None,
        execution_mode: None,
        endpoint_execution_modes: None,
        default_model: Some(LUMALABS_DEFAULT_MODEL.to_string()),
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
        extra_body: Some(
            [(
                "realmId".to_string(),
                json!("4675f69b-4aa5-4b25-bfc7-c480d8efd537"),
            )]
            .into_iter()
            .collect(),
        ),
        session_auth: None,
        keepalive: None,
    };

    let runtime = runtime_from_payload(&payload).unwrap();
    assert_eq!(runtime.realm_id, "4675f69b-4aa5-4b25-bfc7-c480d8efd537");
}

#[test]
fn runtime_reads_optional_video_and_audio_contract_fields() {
    let payload = ProviderAccountPayload {
        discovered_protocols: Vec::new(),
        adapter: "lumalabs_compatible".to_string(),
        base_url: "https://app.lumalabs.ai".to_string(),
        api_key: "wos-session".to_string(),
        credential_id: None,
        expires_at: None,
        runtime_state_object_key: None,
        account_name: None,
        execution_mode: None,
        endpoint_execution_modes: None,
        default_model: Some(LUMALABS_DEFAULT_IMAGE_MODEL.to_string()),
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
        extra_body: Some(
            [
                (
                    "realmId".to_string(),
                    json!("4675f69b-4aa5-4b25-bfc7-c480d8efd537"),
                ),
                ("videoActionType".to_string(), json!("create_video_ray3_14")),
                ("videoArtifactField".to_string(), json!("video")),
                (
                    "audioActionType".to_string(),
                    json!("text_to_music_elevenlabs_v1"),
                ),
                ("audioArtifactField".to_string(), json!("audio")),
            ]
            .into_iter()
            .collect(),
        ),
        session_auth: None,
        keepalive: None,
    };

    let runtime = runtime_from_payload(&payload).unwrap();
    assert_eq!(
        runtime.video_action_type.as_deref(),
        Some("create_video_ray3_14")
    );
    assert_eq!(runtime.video_artifact_field, "video");
    assert_eq!(
        runtime.audio_action_type.as_deref(),
        Some("text_to_music_elevenlabs_v1")
    );
    assert_eq!(runtime.audio_artifact_field, "audio");
}

#[test]
fn media_operation_from_endpoint_kind_rejects_edits_and_chat() {
    assert_eq!(
        LumalabsMediaOperation::from_endpoint_kind(
            crate::protocol::canonical::EndpointKind::ImagesGenerations
        )
        .unwrap(),
        LumalabsMediaOperation::Image
    );
    assert_eq!(
        LumalabsMediaOperation::from_endpoint_kind(
            crate::protocol::canonical::EndpointKind::VideosGenerations
        )
        .unwrap(),
        LumalabsMediaOperation::Video
    );
    assert_eq!(
        LumalabsMediaOperation::from_endpoint_kind(
            crate::protocol::canonical::EndpointKind::MusicGenerations
        )
        .unwrap(),
        LumalabsMediaOperation::Audio
    );

    let edit_err = LumalabsMediaOperation::from_endpoint_kind(
        crate::protocol::canonical::EndpointKind::ImagesEdits,
    )
    .expect_err("image edits should be rejected");
    assert_eq!(
        edit_err.code.as_deref(),
        Some("unsupported_lumalabs_edit_endpoint")
    );

    let chat_err = LumalabsMediaOperation::from_endpoint_kind(
        crate::protocol::canonical::EndpointKind::ChatCompletions,
    )
    .expect_err("chat should be rejected");
    assert_eq!(
        chat_err.code.as_deref(),
        Some("unsupported_lumalabs_endpoint")
    );
}

#[test]
fn unsupported_output_count_error_is_operation_specific() {
    let image = LumalabsMediaOperation::Image.unsupported_output_count_error();
    assert_eq!(image.http_status, Some(400));
    assert_eq!(
        image.code.as_deref(),
        Some("unsupported_lumalabs_image_count")
    );

    let video = LumalabsMediaOperation::Video.unsupported_output_count_error();
    assert_eq!(video.http_status, Some(400));
    assert_eq!(
        video.code.as_deref(),
        Some("unsupported_lumalabs_video_count")
    );

    let audio = LumalabsMediaOperation::Audio.unsupported_output_count_error();
    assert_eq!(audio.http_status, Some(400));
    assert_eq!(
        audio.code.as_deref(),
        Some("unsupported_lumalabs_audio_count")
    );
}

#[test]
fn auto_discovery_is_enabled_only_for_default_action_resolution() {
    let runtime = LumalabsRuntime {
        realm_id: "realm-1".to_string(),
        image_action_type: None,
        video_action_type: None,
        audio_action_type: None,
        image_artifact_field: DEFAULT_IMAGE_ARTIFACT_FIELD.to_string(),
        video_artifact_field: DEFAULT_VIDEO_ARTIFACT_FIELD.to_string(),
        audio_artifact_field: DEFAULT_AUDIO_ARTIFACT_FIELD.to_string(),
    };
    let req = make_request(json!({ "prompt": "hello world" }));

    assert!(should_auto_discover_action_type(
        &req,
        &runtime,
        LumalabsMediaOperation::Video
    ));
    assert!(should_auto_discover_action_type(
        &req,
        &runtime,
        LumalabsMediaOperation::Audio
    ));
}

#[test]
fn auto_discovery_is_disabled_for_runtime_action_override() {
    let runtime = LumalabsRuntime {
        realm_id: "realm-1".to_string(),
        image_action_type: None,
        video_action_type: Some("custom_video_action".to_string()),
        audio_action_type: Some("custom_audio_action".to_string()),
        image_artifact_field: DEFAULT_IMAGE_ARTIFACT_FIELD.to_string(),
        video_artifact_field: DEFAULT_VIDEO_ARTIFACT_FIELD.to_string(),
        audio_artifact_field: DEFAULT_AUDIO_ARTIFACT_FIELD.to_string(),
    };
    let req = make_request(json!({ "prompt": "hello world" }));

    assert!(!should_auto_discover_action_type(
        &req,
        &runtime,
        LumalabsMediaOperation::Video
    ));
    assert!(!should_auto_discover_action_type(
        &req,
        &runtime,
        LumalabsMediaOperation::Audio
    ));
}

#[test]
fn auto_discovery_is_disabled_for_request_action_override() {
    let runtime = LumalabsRuntime {
        realm_id: "realm-1".to_string(),
        image_action_type: None,
        video_action_type: None,
        audio_action_type: None,
        image_artifact_field: DEFAULT_IMAGE_ARTIFACT_FIELD.to_string(),
        video_artifact_field: DEFAULT_VIDEO_ARTIFACT_FIELD.to_string(),
        audio_artifact_field: DEFAULT_AUDIO_ARTIFACT_FIELD.to_string(),
    };
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::VideosGenerations,
        requested_model: Some(LUMALABS_DEFAULT_VIDEO_MODEL.to_string()),
        stream: false,
        messages: vec![],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({
            "prompt": "cinematic dragon flight",
            "action_type": "request_video_override"
        }),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    };

    assert!(!should_auto_discover_action_type(
        &req,
        &runtime,
        LumalabsMediaOperation::Video
    ));
}
