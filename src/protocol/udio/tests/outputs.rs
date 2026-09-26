use super::*;

#[test]
fn songs_ready_requires_audio_url_even_when_ready_to_stream() {
    let songs = extract_songs_from_feed(&json!({
        "songs": [
            {
                "id": "song-1",
                "readyToStream": true,
                "estimatedDuration": 130
            }
        ]
    }))
    .unwrap();

    assert!(songs[0].ready_to_stream);
    assert_eq!(songs[0].estimated_duration_seconds, Some(130.0));
    assert!(!songs_ready(&songs));
}

#[test]
fn songs_ready_for_image_and_video_waits_for_target_artifact() {
    let songs = extract_songs_from_feed(&json!({
        "songs": [
            {
                "id": "song-1",
                "readyToStream": true,
                "image_url": "https://cdn.example.com/song-1.jpg",
                "video_url": "https://cdn.example.com/song-1.mp4"
            }
        ]
    }))
    .unwrap();

    assert!(songs_ready_for_output(&songs, UdioOutputKind::Image));
    assert!(songs_ready_for_output(&songs, UdioOutputKind::Video));

    let pending = extract_songs_from_feed(&json!({
        "songs": [
            {
                "id": "song-2",
                "status": "pending"
            }
        ]
    }))
    .unwrap();
    assert!(!songs_ready_for_output(&pending, UdioOutputKind::Image));
    assert!(!songs_ready_for_output(&pending, UdioOutputKind::Video));
}

#[test]
fn extract_track_ids_reads_track_ids_or_song_ids() {
    let direct = extract_track_ids(&json!({
        "track_ids": ["trk-1", "trk-2"]
    }))
    .unwrap();
    assert_eq!(direct, vec!["trk-1".to_string(), "trk-2".to_string()]);

    let from_songs = extract_track_ids(&json!({
        "songs": [
            { "id": "song-a" },
            { "id": "song-b" }
        ]
    }))
    .unwrap();
    assert_eq!(from_songs, vec!["song-a".to_string(), "song-b".to_string()]);
}

#[test]
fn extract_songs_from_feed_parses_finished_song() {
    let songs = extract_songs_from_feed(&json!({
        "songs": [
            {
                "id": "song-1",
                "title": "Night Drive",
                "song_path": "https://cdn.example.com/song-1.mp3",
                "image_url": "https://cdn.example.com/song-1.jpg",
                "finished": true,
                "created_at": "2026-04-11T00:00:00.000Z"
            }
        ]
    }))
    .unwrap();

    assert_eq!(songs.len(), 1);
    assert_eq!(songs[0].id, "song-1");
    assert_eq!(
        songs[0].audio_url.as_deref(),
        Some("https://cdn.example.com/song-1.mp3")
    );
    assert!(songs_ready(&songs));
}

#[test]
fn image_responses_support_b64_output_mode() {
    let mut req = make_request(json!({
        "prompt": "cover art",
        "response_format": "b64_json",
        "n": 1
    }));
    req.endpoint_kind = EndpointKind::ImagesGenerations;

    assert!(!prefers_url_response(&req).unwrap());
    let response = build_openai_images_response_from_bytes(
        &req,
        "cover art",
        &[("image/png".to_string(), vec![1, 2, 3, 4])],
    );
    assert_eq!(response["data"][0]["mime_type"], "image/png");
    assert!(response["data"][0]["b64_json"].as_str().is_some());
}

#[test]
fn build_video_generation_response_uses_video_asset_shape() {
    let songs = extract_songs_from_feed(&json!({
        "songs": [
            {
                "id": "song-1",
                "title": "Night Drive",
                "song_path": "https://cdn.example.com/song-1.mp3",
                "video_url": "https://cdn.example.com/song-1.mp4",
                "finished": true
            }
        ]
    }))
    .unwrap();

    let response = build_video_generation_response(
        UDIO_DEFAULT_MODEL,
        "night drive city lights",
        &songs,
        true,
        None,
    )
    .unwrap();
    assert_eq!(response["object"], "video.generation");
    assert_eq!(response["provider"], "udio");
    assert_eq!(response["data"][0]["kind"], "video");
    assert_eq!(
        response["data"][0]["url"],
        "https://cdn.example.com/song-1.mp4"
    );
}
