use super::*;

#[test]
fn build_udio_non_image_generation_response_preserves_music_contract() {
    let response = build_udio_non_image_generation_response(
        crate::protocol::udio::UdioOutputKind::Music,
        "udio-music-model",
        "dreamy synthpop",
        &[crate::protocol::udio::UdioSong {
            id: "track-1".to_string(),
            title: Some("Dream Waves".to_string()),
            image_url: None,
            audio_url: Some("https://cdn.example.com/song.mp3".to_string()),
            video_url: None,
            created_at: None,
            duration_seconds: Some(42.0),
            prompt: None,
            lyrics: None,
            lyric_input: None,
            finished: true,
            ready_to_stream: true,
            estimated_duration_seconds: None,
            status: "finished".to_string(),
            error_message: None,
        }],
        true,
        Some("done"),
    )
    .expect("music response");

    assert_eq!(response["object"], "music.generation");
    assert_eq!(response["model"], "udio-music-model");
    assert_eq!(response["prompt"], "dreamy synthpop");
    assert_eq!(response["completed"], true);
    assert_eq!(response["message"], "done");
    assert_eq!(response["data"][0]["id"], "track-1");
}

#[test]
fn build_udio_non_image_generation_response_preserves_video_contract() {
    let response = build_udio_non_image_generation_response(
        crate::protocol::udio::UdioOutputKind::Video,
        "udio-video-model",
        "cinematic trailer",
        &[crate::protocol::udio::UdioSong {
            id: "track-2".to_string(),
            title: Some("Trailer Cut".to_string()),
            image_url: None,
            audio_url: None,
            video_url: Some("https://cdn.example.com/video.mp4".to_string()),
            created_at: None,
            duration_seconds: Some(30.0),
            prompt: None,
            lyrics: None,
            lyric_input: None,
            finished: true,
            ready_to_stream: true,
            estimated_duration_seconds: None,
            status: "finished".to_string(),
            error_message: None,
        }],
        false,
        None,
    )
    .expect("video response");

    assert_eq!(response["object"], "video.generation");
    assert_eq!(response["model"], "udio-video-model");
    assert_eq!(response["prompt"], "cinematic trailer");
    assert_eq!(response["completed"], false);
    assert_eq!(
        response["data"][0]["url"],
        "https://cdn.example.com/video.mp4"
    );
}

#[test]
fn resolve_udio_image_generation_plan_preserves_url_response_contract() {
    let req = image_generation_request(serde_json::json!({}));
    let songs = vec![crate::protocol::udio::UdioSong {
        id: "track-3".to_string(),
        title: Some("Cover Art".to_string()),
        image_url: Some("https://cdn.example.com/cover.jpg".to_string()),
        audio_url: None,
        video_url: None,
        created_at: None,
        duration_seconds: None,
        prompt: None,
        lyrics: None,
        lyric_input: None,
        finished: true,
        ready_to_stream: true,
        estimated_duration_seconds: None,
        status: "finished".to_string(),
        error_message: None,
    }];

    let (image_urls, response) =
        resolve_udio_image_generation_plan(&req, "album art", &songs).expect("image plan");

    assert_eq!(
        image_urls,
        vec!["https://cdn.example.com/cover.jpg".to_string()]
    );
    let response = response.expect("url response");
    assert_eq!(
        response["data"][0]["url"],
        "https://cdn.example.com/cover.jpg"
    );
    assert_eq!(response["data"][0]["revised_prompt"], "album art");
}

#[test]
fn resolve_udio_image_generation_plan_preserves_b64_contract() {
    let req = image_generation_request(serde_json::json!({ "response_format": "b64_json" }));
    let songs = vec![crate::protocol::udio::UdioSong {
        id: "track-4".to_string(),
        title: Some("Poster".to_string()),
        image_url: Some("https://cdn.example.com/poster.jpg".to_string()),
        audio_url: None,
        video_url: None,
        created_at: None,
        duration_seconds: None,
        prompt: None,
        lyrics: None,
        lyric_input: None,
        finished: true,
        ready_to_stream: true,
        estimated_duration_seconds: None,
        status: "finished".to_string(),
        error_message: None,
    }];

    let (image_urls, response) =
        resolve_udio_image_generation_plan(&req, "poster art", &songs).expect("image plan");

    assert_eq!(
        image_urls,
        vec!["https://cdn.example.com/poster.jpg".to_string()]
    );
    assert!(response.is_none());
}

#[test]
fn resolve_udio_image_generation_plan_limits_download_urls_to_requested_count_contract() {
    let req = image_generation_request(serde_json::json!({ "n": 1 }));
    let songs = vec![
        crate::protocol::udio::UdioSong {
            id: "track-5".to_string(),
            title: Some("Poster One".to_string()),
            image_url: Some("https://cdn.example.com/poster-1.jpg".to_string()),
            audio_url: None,
            video_url: None,
            created_at: None,
            duration_seconds: None,
            prompt: None,
            lyrics: None,
            lyric_input: None,
            finished: true,
            ready_to_stream: true,
            estimated_duration_seconds: None,
            status: "finished".to_string(),
            error_message: None,
        },
        crate::protocol::udio::UdioSong {
            id: "track-6".to_string(),
            title: Some("Poster Two".to_string()),
            image_url: Some("https://cdn.example.com/poster-2.jpg".to_string()),
            audio_url: None,
            video_url: None,
            created_at: None,
            duration_seconds: None,
            prompt: None,
            lyrics: None,
            lyric_input: None,
            finished: true,
            ready_to_stream: true,
            estimated_duration_seconds: None,
            status: "finished".to_string(),
            error_message: None,
        },
    ];

    let (image_urls, response) =
        resolve_udio_image_generation_plan(&req, "poster art", &songs).expect("image plan");

    assert_eq!(
        image_urls,
        vec!["https://cdn.example.com/poster-1.jpg".to_string()]
    );
    let response = response.expect("url response");
    assert_eq!(response["data"].as_array().map(Vec::len), Some(1));
}

#[test]
fn resolve_udio_downloaded_image_mime_type_preserves_image_header_contract() {
    let mut headers = rquest::header::HeaderMap::new();
    headers.insert(
        rquest::header::CONTENT_TYPE,
        rquest::header::HeaderValue::from_static("image/png; charset=utf-8"),
    );

    let mime_type = resolve_udio_downloaded_image_mime_type(&headers);

    assert_eq!(mime_type, "image/png");
}

#[test]
fn resolve_udio_downloaded_image_mime_type_defaults_to_jpeg_contract() {
    let mut headers = rquest::header::HeaderMap::new();
    headers.insert(
        rquest::header::CONTENT_TYPE,
        rquest::header::HeaderValue::from_static("application/octet-stream"),
    );

    let mime_type = resolve_udio_downloaded_image_mime_type(&headers);

    assert_eq!(mime_type, "image/jpeg");
}

#[test]
fn materialize_udio_downloaded_image_preserves_header_mime_contract() {
    let mut headers = rquest::header::HeaderMap::new();
    headers.insert(
        rquest::header::CONTENT_TYPE,
        rquest::header::HeaderValue::from_static("image/png"),
    );

    let (mime_type, bytes) = materialize_udio_downloaded_image(&headers, b"png-bytes");

    assert_eq!(mime_type, "image/png");
    assert_eq!(bytes, b"png-bytes".to_vec());
}

#[test]
fn materialize_udio_downloaded_image_defaults_to_jpeg_contract() {
    let headers = rquest::header::HeaderMap::new();

    let (mime_type, bytes) = materialize_udio_downloaded_image(&headers, b"jpg-bytes");

    assert_eq!(mime_type, "image/jpeg");
    assert_eq!(bytes, b"jpg-bytes".to_vec());
}

#[test]
fn build_udio_downloaded_images_response_preserves_b64_contract() {
    let req = image_generation_request(serde_json::json!({
        "response_format": "b64_json"
    }));

    let response = build_udio_downloaded_images_response(
        &req,
        "poster art",
        &[("image/png".to_string(), b"png-bytes".to_vec())],
    );

    assert_eq!(response["data"][0]["mime_type"], "image/png");
    assert_eq!(response["data"][0]["revised_prompt"], "poster art");
    assert!(response["data"][0]["b64_json"].as_str().is_some());
    assert!(response["data"][0].get("url").is_none());
}

#[test]
fn build_udio_downloaded_images_response_limits_requested_count_contract() {
    let req = image_generation_request(serde_json::json!({ "n": 1 }));

    let response = build_udio_downloaded_images_response(
        &req,
        "cover art",
        &[
            ("image/png".to_string(), b"first".to_vec()),
            ("image/jpeg".to_string(), b"second".to_vec()),
        ],
    );

    assert_eq!(response["data"].as_array().map(Vec::len), Some(1));
    assert_eq!(response["data"][0]["mime_type"], "image/png");
}
