use super::*;

#[test]
fn build_openai_images_response_from_urls_uses_clip_images() {
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
        raw_body: json!({
            "prompt": "cover art",
            "n": 1,
            "response_format": "url"
        }),
        previous_response_id: None,
        explicit_session_key: None,
        extra: std::collections::HashMap::new(),
    };
    let response = build_openai_images_response_from_urls(
        &req,
        "cover art",
        &[SunoClip {
            id: "clip-1".to_string(),
            title: Some("Cover".to_string()),
            image_url: Some("https://cdn.example.com/cover.webp".to_string()),
            lyric: None,
            audio_url: None,
            video_url: None,
            created_at: None,
            model_name: None,
            prompt: None,
            gpt_description_prompt: None,
            status: "complete".to_string(),
            clip_type: None,
            tags: None,
            negative_tags: None,
            duration: None,
            error_message: None,
        }],
    )
    .unwrap();
    assert_eq!(
        response["data"][0]["url"].as_str(),
        Some("https://cdn.example.com/cover.webp")
    );
    assert_eq!(
        response["data"][0]["mime_type"].as_str(),
        Some("image/webp")
    );
}

#[test]
fn build_video_generation_response_uses_platform_media_shape() {
    let response = build_video_generation_response(
        SUNO_DEFAULT_MODEL,
        "cinematic performance clip",
        &[SunoClip {
            id: "clip-1".to_string(),
            title: Some("Performance".to_string()),
            image_url: Some("https://cdn.example.com/cover.png".to_string()),
            lyric: Some("silver rain".to_string()),
            audio_url: Some("https://cdn.example.com/track.mp3".to_string()),
            video_url: Some("https://cdn.example.com/track.mp4".to_string()),
            created_at: Some("2026-04-21T00:00:00Z".to_string()),
            model_name: Some(SUNO_DEFAULT_UPSTREAM_MODEL.to_string()),
            prompt: Some("cinematic performance clip".to_string()),
            gpt_description_prompt: None,
            status: "complete".to_string(),
            clip_type: None,
            tags: Some("pop".to_string()),
            negative_tags: None,
            duration: Some("120".to_string()),
            error_message: None,
        }],
        true,
        None,
    )
    .unwrap();
    assert_eq!(response["object"], "video.generation");
    assert_eq!(response["data"][0]["kind"], "video");
    assert_eq!(response["data"][0]["mime_type"], "video/mp4");
    assert_eq!(
        response["data"][0]["url"].as_str(),
        Some("https://cdn.example.com/track.mp4")
    );
}

#[test]
fn extract_clips_from_feed_maps_audio_fields() {
    let clips = extract_clips_from_feed(&json!({
        "clips": [{
            "id": "clip_1",
            "status": "streaming",
            "audio_url": "https://cdn.example/audio.mp3",
            "video_url": "https://cdn.example/video.mp4",
            "metadata": {
                "prompt": "hello",
                "tags": "pop"
            }
        }]
    }))
    .unwrap();
    assert_eq!(clips.len(), 1);
    assert_eq!(
        clips[0].audio_url.as_deref(),
        Some("https://cdn.example/audio.mp3")
    );
    assert_eq!(clips[0].tags.as_deref(), Some("pop"));
}

#[test]
fn clips_ready_for_endpoint_requires_completed_status_and_target_asset() {
    let mut clip = SunoClip {
        id: "clip".to_string(),
        title: None,
        image_url: None,
        lyric: None,
        audio_url: None,
        video_url: None,
        created_at: None,
        model_name: None,
        prompt: None,
        gpt_description_prompt: None,
        status: "complete".to_string(),
        clip_type: None,
        tags: None,
        negative_tags: None,
        duration: None,
        error_message: None,
    };

    assert!(!clips_ready_for_endpoint(
        EndpointKind::MusicGenerations,
        &[]
    ));
    assert!(!clips_ready_for_endpoint(
        EndpointKind::MusicGenerations,
        &[clip.clone()]
    ));

    clip.audio_url = Some("https://cdn.example/audio.mp3".to_string());
    clip.status = "streaming".to_string();
    assert!(!clips_ready_for_endpoint(
        EndpointKind::MusicGenerations,
        &[clip.clone()]
    ));

    clip.status = "complete".to_string();
    assert!(clips_ready_for_endpoint(
        EndpointKind::MusicGenerations,
        &[clip.clone()]
    ));
    assert!(!clips_ready_for_endpoint(
        EndpointKind::ImagesGenerations,
        &[clip.clone()]
    ));

    clip.image_url = Some("https://cdn.example/image.png".to_string());
    assert!(clips_ready_for_endpoint(
        EndpointKind::ImagesGenerations,
        &[clip]
    ));
}
