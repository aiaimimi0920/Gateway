use super::*;

#[test]
fn payload_model_family_matrix_overrides_global_family_list() {
    let payload = json!({
        "supportedProtocolFamilies": ["openai_chat", "openai_responses"],
        "protocolFamiliesByModel": {
            "gpt-4o": ["openai_responses"]
        }
    });
    let resolved = resolve_supported_wire_protocol_families_for_model(
        Some(&payload),
        Some("gpt-4o"),
        Some("gpt-4o"),
        "openai_compatible",
        "openai",
    );
    assert_eq!(resolved, vec![OPENAI_RESPONSES_FAMILY.to_string()]);
}

#[test]
fn finalize_candidate_prefers_same_family_when_supported() {
    let req = make_request(ProtocolFamily::OpenAi, EndpointKind::Responses);
    let mut candidate = make_candidate("openai");
    let same_family = finalize_candidate_protocol_family(&mut candidate, &req);
    assert_eq!(same_family, Some(true));
    assert_eq!(candidate.protocol_family, OPENAI_RESPONSES_FAMILY);
    assert_eq!(candidate.priority, 10 + SAME_PROTOCOL_FAMILY_PRIORITY_BONUS);
}

#[test]
fn finalize_candidate_filters_out_surface_with_no_matching_supported_family() {
    let req = make_request(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
    let mut candidate = make_candidate("openai");
    candidate.supported_protocol_families = vec![ANTHROPIC_MESSAGES_FAMILY.to_string()];
    let same_family = finalize_candidate_protocol_family(&mut candidate, &req);
    assert_eq!(same_family, None);
}

#[test]
fn route_policy_generic_search_family_matches_explicit_search_surface() {
    assert!(route_policy_family_matches_surface(
        "search_api",
        "search_api_compatible",
        "linkup_search",
    ));
    assert!(route_policy_family_matches_candidate(
        "search_api",
        "tavily_search",
    ));
}

#[test]
fn requested_wire_protocol_family_maps_openai_modality_endpoints() {
    let req = make_request(ProtocolFamily::OpenAi, EndpointKind::Embeddings);
    assert_eq!(
        requested_wire_protocol_family(&req).as_deref(),
        Some(OPENAI_EMBEDDINGS_FAMILY)
    );

    let req = make_request(ProtocolFamily::OpenAi, EndpointKind::AudioSpeech);
    assert_eq!(
        requested_wire_protocol_family(&req).as_deref(),
        Some(OPENAI_AUDIO_SPEECH_FAMILY)
    );
}

#[test]
fn finalize_candidate_selects_special_media_family_for_native_adapter() {
    let req = make_request(ProtocolFamily::OpenAi, EndpointKind::MusicGenerations);
    let mut candidate = make_candidate(PRODUCER_MUSIC_FAMILY);
    candidate.adapter = "producer_compatible".to_string();
    candidate.payload.adapter = "producer_compatible".to_string();
    candidate.protocol_family = PRODUCER_MUSIC_FAMILY.to_string();
    candidate.supported_protocol_families = vec![
        PRODUCER_MUSIC_FAMILY.to_string(),
        PRODUCER_VIDEOS_FAMILY.to_string(),
    ];

    let same_family = finalize_candidate_protocol_family(&mut candidate, &req);
    assert_eq!(same_family, Some(false));
    assert_eq!(candidate.protocol_family, PRODUCER_MUSIC_FAMILY);
}

#[test]
fn lumalabs_surface_supports_image_video_and_audio_families() {
    let families =
        surface_supported_wire_protocol_families("lumalabs_compatible", LUMALABS_IMAGES_FAMILY);
    assert!(families.contains(&LUMALABS_IMAGES_FAMILY.to_string()));
    assert!(families.contains(&LUMALABS_AUDIO_FAMILY.to_string()));
    assert!(families.contains(&LUMALABS_VIDEOS_FAMILY.to_string()));
}

#[test]
fn suno_surface_supports_image_video_and_audio_families() {
    let families = surface_supported_wire_protocol_families("suno_compatible", SUNO_MUSIC_FAMILY);
    assert!(families.contains(&SUNO_IMAGES_FAMILY.to_string()));
    assert!(families.contains(&SUNO_MUSIC_FAMILY.to_string()));
    assert!(families.contains(&SUNO_VIDEOS_FAMILY.to_string()));
}

#[test]
fn producer_surface_supports_image_video_and_audio_families() {
    let families =
        surface_supported_wire_protocol_families("producer_compatible", PRODUCER_MUSIC_FAMILY);
    assert!(families.contains(&PRODUCER_IMAGES_FAMILY.to_string()));
    assert!(families.contains(&PRODUCER_MUSIC_FAMILY.to_string()));
    assert!(families.contains(&PRODUCER_VIDEOS_FAMILY.to_string()));
}

#[test]
fn gemini_api_surface_supports_text_tts_and_media_families() {
    let families = surface_supported_wire_protocol_families(
        "gemini_api_modular_compatible",
        GEMINI_GENERATE_CONTENT_FAMILY,
    );
    assert!(families.contains(&GEMINI_GENERATE_CONTENT_FAMILY.to_string()));
    assert!(families.contains(&GEMINI_CANVAS_IMAGES_FAMILY.to_string()));
    assert!(families.contains(&GEMINI_CANVAS_MUSIC_FAMILY.to_string()));
    assert!(families.contains(&GEMINI_CANVAS_VIDEOS_FAMILY.to_string()));
}

#[test]
fn udio_surface_supports_image_video_and_audio_families() {
    let families = surface_supported_wire_protocol_families("udio_compatible", UDIO_MUSIC_FAMILY);
    assert!(families.contains(&UDIO_IMAGES_FAMILY.to_string()));
    assert!(families.contains(&UDIO_MUSIC_FAMILY.to_string()));
    assert!(families.contains(&UDIO_VIDEOS_FAMILY.to_string()));
}

#[test]
fn finalize_candidate_selects_lumalabs_video_family_for_video_requests() {
    let req = make_request(ProtocolFamily::OpenAi, EndpointKind::VideosGenerations);
    let mut candidate = make_candidate(LUMALABS_IMAGES_FAMILY);
    candidate.adapter = "lumalabs_compatible".to_string();
    candidate.payload.adapter = "lumalabs_compatible".to_string();
    candidate.protocol_family = LUMALABS_IMAGES_FAMILY.to_string();
    candidate.supported_protocol_families = vec![
        LUMALABS_IMAGES_FAMILY.to_string(),
        LUMALABS_AUDIO_FAMILY.to_string(),
        LUMALABS_VIDEOS_FAMILY.to_string(),
    ];

    let same_family = finalize_candidate_protocol_family(&mut candidate, &req);
    assert_eq!(same_family, Some(false));
    assert_eq!(candidate.protocol_family, LUMALABS_VIDEOS_FAMILY);
}

#[test]
fn finalize_candidate_selects_suno_video_family_for_video_requests() {
    let req = make_request(ProtocolFamily::OpenAi, EndpointKind::VideosGenerations);
    let mut candidate = make_candidate(SUNO_MUSIC_FAMILY);
    candidate.adapter = "suno_compatible".to_string();
    candidate.payload.adapter = "suno_compatible".to_string();
    candidate.protocol_family = SUNO_MUSIC_FAMILY.to_string();
    candidate.supported_protocol_families = vec![
        SUNO_IMAGES_FAMILY.to_string(),
        SUNO_MUSIC_FAMILY.to_string(),
        SUNO_VIDEOS_FAMILY.to_string(),
    ];

    let same_family = finalize_candidate_protocol_family(&mut candidate, &req);
    assert_eq!(same_family, Some(false));
    assert_eq!(candidate.protocol_family, SUNO_VIDEOS_FAMILY);
}

#[test]
fn finalize_candidate_selects_producer_image_family_for_image_requests() {
    let req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesGenerations);
    let mut candidate = make_candidate(PRODUCER_MUSIC_FAMILY);
    candidate.adapter = "producer_compatible".to_string();
    candidate.payload.adapter = "producer_compatible".to_string();
    candidate.protocol_family = PRODUCER_MUSIC_FAMILY.to_string();
    candidate.supported_protocol_families = vec![
        PRODUCER_IMAGES_FAMILY.to_string(),
        PRODUCER_MUSIC_FAMILY.to_string(),
        PRODUCER_VIDEOS_FAMILY.to_string(),
    ];

    let same_family = finalize_candidate_protocol_family(&mut candidate, &req);
    assert_eq!(same_family, Some(false));
    assert_eq!(candidate.protocol_family, PRODUCER_IMAGES_FAMILY);
}

#[test]
fn finalize_candidate_selects_gemini_canvas_image_family_for_image_edit_requests() {
    let req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesEdits);
    let mut candidate = make_candidate(GEMINI_CANVAS_MUSIC_FAMILY);
    candidate.adapter = "gemini_canvas_compatible".to_string();
    candidate.payload.adapter = "gemini_canvas_compatible".to_string();
    candidate.protocol_family = GEMINI_CANVAS_MUSIC_FAMILY.to_string();
    candidate.supported_protocol_families = vec![
        GEMINI_CANVAS_IMAGES_FAMILY.to_string(),
        GEMINI_CANVAS_MUSIC_FAMILY.to_string(),
        GEMINI_CANVAS_VIDEOS_FAMILY.to_string(),
    ];

    let same_family = finalize_candidate_protocol_family(&mut candidate, &req);
    assert_eq!(same_family, Some(false));
    assert_eq!(candidate.protocol_family, GEMINI_CANVAS_IMAGES_FAMILY);
}

#[test]
fn finalize_candidate_selects_udio_video_family_for_video_requests() {
    let req = make_request(ProtocolFamily::OpenAi, EndpointKind::VideosGenerations);
    let mut candidate = make_candidate(UDIO_MUSIC_FAMILY);
    candidate.adapter = "udio_compatible".to_string();
    candidate.payload.adapter = "udio_compatible".to_string();
    candidate.protocol_family = UDIO_MUSIC_FAMILY.to_string();
    candidate.supported_protocol_families = vec![
        UDIO_IMAGES_FAMILY.to_string(),
        UDIO_MUSIC_FAMILY.to_string(),
        UDIO_VIDEOS_FAMILY.to_string(),
    ];

    let same_family = finalize_candidate_protocol_family(&mut candidate, &req);
    assert_eq!(same_family, Some(false));
    assert_eq!(candidate.protocol_family, UDIO_VIDEOS_FAMILY);
}
