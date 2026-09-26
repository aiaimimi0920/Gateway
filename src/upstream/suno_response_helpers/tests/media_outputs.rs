use super::*;

#[test]
fn resolve_suno_image_generation_plan_preserves_url_response_contract() {
    let req = image_generation_request(json!({}));
    let clips = vec![crate::protocol::suno::SunoClip {
        id: "clip-1".to_string(),
        title: Some("Cover".to_string()),
        image_url: Some("https://cdn.example.com/cover.png".to_string()),
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
    }];

    let (image_urls, response) =
        resolve_suno_image_generation_plan(&req, "cover prompt", &clips).expect("image plan");

    assert_eq!(
        image_urls,
        vec!["https://cdn.example.com/cover.png".to_string()]
    );
    let response = response.expect("url response");
    assert_eq!(
        response["data"][0]["url"],
        "https://cdn.example.com/cover.png"
    );
    assert_eq!(response["data"][0]["revised_prompt"], "cover prompt");
}

#[test]
fn resolve_suno_image_generation_plan_preserves_b64_contract() {
    let req = image_generation_request(json!({ "response_format": "b64_json" }));
    let clips = vec![crate::protocol::suno::SunoClip {
        id: "clip-2".to_string(),
        title: Some("Poster".to_string()),
        image_url: Some("https://cdn.example.com/poster.png".to_string()),
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
    }];

    let (image_urls, response) =
        resolve_suno_image_generation_plan(&req, "poster prompt", &clips).expect("image plan");

    assert_eq!(
        image_urls,
        vec!["https://cdn.example.com/poster.png".to_string()]
    );
    assert!(response.is_none());
}

#[test]
fn resolve_suno_downloaded_image_mime_type_prefers_image_header_contract() {
    let mut headers = rquest::header::HeaderMap::new();
    headers.insert(
        rquest::header::CONTENT_TYPE,
        rquest::header::HeaderValue::from_static("image/webp; charset=utf-8"),
    );

    let mime_type =
        resolve_suno_downloaded_image_mime_type(&headers, "https://cdn.example.com/cover.png");

    assert_eq!(mime_type, "image/webp");
}

#[test]
fn resolve_suno_downloaded_image_mime_type_falls_back_to_url_contract() {
    let headers = rquest::header::HeaderMap::new();

    let mime_type =
        resolve_suno_downloaded_image_mime_type(&headers, "https://cdn.example.com/cover.jpg");

    assert_eq!(mime_type, "image/jpeg");
}

#[test]
fn materialize_suno_downloaded_image_preserves_header_mime_contract() {
    let mut headers = rquest::header::HeaderMap::new();
    headers.insert(
        rquest::header::CONTENT_TYPE,
        rquest::header::HeaderValue::from_static("image/webp"),
    );

    let (mime_type, bytes) = materialize_suno_downloaded_image(
        &headers,
        "https://cdn.example.com/cover.png",
        b"webp-bytes",
    );

    assert_eq!(mime_type, "image/webp");
    assert_eq!(bytes, b"webp-bytes".to_vec());
}

#[test]
fn materialize_suno_downloaded_image_falls_back_to_url_contract() {
    let headers = rquest::header::HeaderMap::new();

    let (mime_type, bytes) = materialize_suno_downloaded_image(
        &headers,
        "https://cdn.example.com/cover.jpg",
        b"jpg-bytes",
    );

    assert_eq!(mime_type, "image/jpeg");
    assert_eq!(bytes, b"jpg-bytes".to_vec());
}

#[test]
fn build_suno_downloaded_images_response_preserves_b64_contract() {
    let req = image_generation_request(json!({
        "response_format": "b64_json"
    }));

    let response = build_suno_downloaded_images_response(
        &req,
        "cover prompt",
        &[("image/webp".to_string(), b"webp-bytes".to_vec())],
    )
    .expect("downloaded image response");

    assert_eq!(response["data"][0]["mime_type"], "image/webp");
    assert_eq!(response["data"][0]["revised_prompt"], "cover prompt");
    assert!(response["data"][0]["b64_json"].as_str().is_some());
    assert!(response["data"][0].get("url").is_none());
}

#[test]
fn build_suno_downloaded_images_response_limits_requested_count_contract() {
    let req = image_generation_request(json!({ "n": 1 }));

    let response = build_suno_downloaded_images_response(
        &req,
        "poster prompt",
        &[
            ("image/png".to_string(), b"first".to_vec()),
            ("image/jpeg".to_string(), b"second".to_vec()),
        ],
    )
    .expect("downloaded image response");

    assert_eq!(response["data"].as_array().map(Vec::len), Some(1));
    assert_eq!(response["data"][0]["mime_type"], "image/png");
}

#[test]
fn build_suno_non_image_generation_response_preserves_music_contract() {
    let clips = vec![crate::protocol::suno::SunoClip {
        id: "clip-music-1".to_string(),
        title: Some("Dream Song".to_string()),
        image_url: None,
        lyric: None,
        audio_url: Some("https://cdn.example.com/song.mp3".to_string()),
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
    }];

    let body = build_suno_non_image_generation_response(
        crate::protocol::canonical::EndpointKind::MusicGenerations,
        "chirp-v3-5",
        "dream song",
        &clips,
        true,
        Some("done"),
    )
    .expect("music response");

    assert_eq!(body["object"], "music.generation");
    assert_eq!(body["model"], "chirp-v3-5");
    assert_eq!(body["prompt"], "dream song");
    assert_eq!(body["completed"], true);
    assert_eq!(body["message"], "done");
}

#[test]
fn build_suno_non_image_generation_response_preserves_video_contract() {
    let clips = vec![crate::protocol::suno::SunoClip {
        id: "clip-video-1".to_string(),
        title: Some("Dream Video".to_string()),
        image_url: None,
        lyric: None,
        audio_url: None,
        video_url: Some("https://cdn.example.com/video.mp4".to_string()),
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
    }];

    let body = build_suno_non_image_generation_response(
        crate::protocol::canonical::EndpointKind::VideosGenerations,
        "chirp-v3-5",
        "dream video",
        &clips,
        false,
        None,
    )
    .expect("video response");

    assert_eq!(body["object"], "video.generation");
    assert_eq!(body["model"], "chirp-v3-5");
    assert_eq!(body["prompt"], "dream video");
    assert_eq!(body["completed"], false);
}

#[test]
fn build_suno_non_image_generation_response_rejects_unsupported_contract() {
    let error = build_suno_non_image_generation_response(
        crate::protocol::canonical::EndpointKind::ImagesGenerations,
        "chirp-v3-5",
        "dream image",
        &[],
        false,
        None,
    )
    .expect_err("image endpoint should be unsupported here");

    assert_eq!(error.code.as_deref(), Some("unsupported_suno_endpoint"));
    assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
}
