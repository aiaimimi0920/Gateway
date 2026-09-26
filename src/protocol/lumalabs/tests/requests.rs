use super::*;

#[test]
fn aspect_ratio_maps_common_openai_sizes() {
    let req = make_request(json!({ "prompt": "island", "size": "1792x1024" }));
    assert_eq!(aspect_ratio_from_request(&req), "16:9");
}

#[test]
fn action_request_uses_uni_1_shape() {
    let body = build_action_request("hello", "16:9", "", "auto", "png", "abcd1234");
    assert_eq!(body["type"], DEFAULT_IMAGE_ACTION_TYPE);
    assert_eq!(body["fields"]["aspect_ratio"], "16:9");
    assert_eq!(body["optimistic_output_ids"][0], "abcd1234");
}

#[test]
fn response_format_defaults_to_url() {
    let req = make_request(json!({ "prompt": "hello" }));
    assert!(prefers_url_response(&req).unwrap());
}

#[test]
fn video_action_request_uses_runtime_action_type_and_video_fields() {
    let runtime = LumalabsRuntime {
        realm_id: "realm-1".to_string(),
        image_action_type: None,
        video_action_type: Some("create_video_ray3_14".to_string()),
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
            "size": "16:9",
            "durationSeconds": 10,
            "resolution": "1080p"
        }),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    };

    let body = build_action_request_for_operation(
        &req,
        &runtime,
        LumalabsMediaOperation::Video,
        LUMALABS_DEFAULT_VIDEO_MODEL,
        "cinematic dragon flight",
        "vid12345",
    );

    assert_eq!(body["type"], "create_video_ray3_14");
    assert!(body["fields"].get("model").is_none());
    assert_eq!(body["fields"]["aspect_ratio"], "16:9");
    assert_eq!(body["fields"]["duration_seconds"], 10);
    assert_eq!(body["fields"]["resolution"], "1080p");
    assert_eq!(body["optimistic_output_ids"][0], "vid12345");
}

#[test]
fn audio_action_request_supports_lyrics_and_duration() {
    let runtime = LumalabsRuntime {
        realm_id: "realm-1".to_string(),
        image_action_type: None,
        video_action_type: None,
        audio_action_type: Some("text_to_music_elevenlabs_v1".to_string()),
        image_artifact_field: DEFAULT_IMAGE_ARTIFACT_FIELD.to_string(),
        video_artifact_field: DEFAULT_VIDEO_ARTIFACT_FIELD.to_string(),
        audio_artifact_field: DEFAULT_AUDIO_ARTIFACT_FIELD.to_string(),
    };
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::MusicGenerations,
        requested_model: Some(LUMALABS_DEFAULT_AUDIO_MODEL.to_string()),
        stream: false,
        messages: vec![],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({
            "lyrics": "silver rain over neon streets",
            "duration": 24,
            "voice": "alto"
        }),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    };

    let prompt = prompt_from_request_for_operation(&req, LumalabsMediaOperation::Audio).unwrap();
    assert_eq!(prompt, "silver rain over neon streets");
    let body = build_action_request_for_operation(
        &req,
        &runtime,
        LumalabsMediaOperation::Audio,
        LUMALABS_DEFAULT_AUDIO_MODEL,
        &prompt,
        "aud12345",
    );

    assert_eq!(body["type"], "text_to_music_elevenlabs_v1");
    assert_eq!(body["fields"]["lyrics"], "silver rain over neon streets");
    assert!(body["fields"].get("voice").is_none());
    assert_eq!(body["fields"]["duration_seconds"], 24);
    assert_eq!(body["fields"]["output_format"], "mp3");
}
