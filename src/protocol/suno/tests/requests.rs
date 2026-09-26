use super::*;

#[test]
fn normalize_music_generations_defaults_model() {
    let request = normalize_music_generations(json!({
        "prompt": "warm synthwave duet"
    }))
    .unwrap();
    assert_eq!(request.endpoint_kind, EndpointKind::MusicGenerations);
    assert_eq!(request.requested_model.as_deref(), Some(SUNO_DEFAULT_MODEL));
}

#[test]
fn build_generate_request_uses_description_mode_by_default() {
    let request = normalize_music_generations(json!({
        "prompt": "warm synthwave duet"
    }))
    .unwrap();
    let body = build_generate_request(&request, SUNO_DEFAULT_MODEL, None).unwrap();
    assert_eq!(body["gpt_description_prompt"], "warm synthwave duet");
    assert_eq!(body["prompt"], "");
    assert_eq!(body["mv"], SUNO_DEFAULT_UPSTREAM_MODEL);
    assert_eq!(body["metadata"]["create_mode"], "simple");
    assert_eq!(body["override_fields"], json!([]));
    assert!(body["transaction_uuid"].as_str().is_some());
}

#[test]
fn build_generate_request_uses_custom_mode_for_lyrics() {
    let request = normalize_music_generations(json!({
        "lyrics": "hello from the bridge",
        "tags": "lofi",
        "title": "Bridge"
    }))
    .unwrap();
    let body = build_generate_request(&request, SUNO_DEFAULT_MODEL, Some("tier-123")).unwrap();
    assert_eq!(body["prompt"], "hello from the bridge");
    assert_eq!(body["gpt_description_prompt"], "");
    assert_eq!(body["tags"], "lofi");
    assert_eq!(body["metadata"]["create_mode"], "custom");
    assert_eq!(body["metadata"]["user_tier"], "tier-123");
}

#[test]
fn response_format_defaults_to_url_for_images() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ImagesGenerations,
        requested_model: Some(SUNO_DEFAULT_MODEL.to_string()),
        stream: false,
        messages: vec![],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({"prompt": "cover art"}),
        previous_response_id: None,
        explicit_session_key: None,
        extra: std::collections::HashMap::new(),
    };
    assert!(prefers_url_response(&req).unwrap());
}

#[test]
fn resolve_model_maps_public_models_to_live_web_mv_values() {
    assert_eq!(
        resolve_model(SUNO_DEFAULT_MODEL).unwrap(),
        SUNO_DEFAULT_UPSTREAM_MODEL
    );
    assert_eq!(
        resolve_model(SUNO_LEGACY_MODEL).unwrap(),
        SUNO_LEGACY_UPSTREAM_MODEL
    );
    assert_eq!(
        resolve_model(SUNO_DEFAULT_UPSTREAM_MODEL).unwrap(),
        SUNO_DEFAULT_UPSTREAM_MODEL
    );
}

#[test]
fn build_feed_poll_request_uses_feed_v3_shape() {
    let body = build_feed_poll_request(&["clip-a".to_string(), "clip-b".to_string()]);
    assert_eq!(body["filters"]["ids"]["presence"], "True");
    assert_eq!(
        body["filters"]["ids"]["clipIds"],
        json!(["clip-a", "clip-b"])
    );
    assert_eq!(body["limit"], 2);
}

#[test]
fn browser_token_header_value_matches_live_shape() {
    let value = build_browser_token_header_value(1_775_875_135_731);
    assert_eq!(
        value,
        "{\"token\":\"eyJ0aW1lc3RhbXAiOjE3NzU4NzUxMzU3MzF9\"}"
    );
}
