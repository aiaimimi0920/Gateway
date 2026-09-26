use super::*;

#[test]
fn extract_stream_generate_media_assets_recovers_video_and_music_urls() {
    let video_url = "https://example.invalid/video.mp4";
    let music_url = "https://example.invalid/music.mp3";
    let thumb_url = "https://example.invalid/thumb.png";
    let video_info = json!([[
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        [thumb_url, video_url]
    ]]);
    let music_data = json!([
        [
            null,
            [
                null,
                null,
                null,
                null,
                null,
                null,
                null,
                [thumb_url, music_url]
            ]
        ],
        [
            null,
            [
                null,
                null,
                null,
                null,
                null,
                null,
                null,
                [thumb_url, "https://example.invalid/music.mp4"]
            ]
        ]
    ]);

    let mut video_frame = vec![Value::Null; 60];
    video_frame[59] = json!([[[video_info]]]);
    let video_frame_json = serde_json::to_string(&vec![Value::Array(video_frame)]).unwrap();
    let video_body = format!(
        ")]}}'\n{}\n{}\n",
        video_frame_json.encode_utf16().count(),
        video_frame_json
    );
    let video_assets =
        extract_stream_generate_media_assets(&video_body, GeminiCanvasMediaOperation::Video)
            .unwrap();
    assert_eq!(video_assets.len(), 1);
    assert_eq!(video_assets[0].kind, "video");
    assert_eq!(video_assets[0].url, video_url);
    assert_eq!(video_assets[0].mime_type, "video/mp4");

    let mut music_frame = vec![Value::Null; 87];
    music_frame[86] = music_data;
    let music_frame_json = serde_json::to_string(&vec![Value::Array(music_frame)]).unwrap();
    let music_body = format!(
        ")]}}'\n{}\n{}\n",
        music_frame_json.encode_utf16().count(),
        music_frame_json
    );
    let music_assets =
        extract_stream_generate_media_assets(&music_body, GeminiCanvasMediaOperation::Music)
            .unwrap();
    assert_eq!(music_assets.len(), 1);
    assert_eq!(music_assets[0].kind, "audio");
    assert_eq!(music_assets[0].url, music_url);
    assert_eq!(music_assets[0].mime_type, "audio/mpeg");
}

#[test]
fn extract_stream_generate_media_assets_recovers_video_and_music_from_root_candidate_shape() {
    let video_url = "https://example.invalid/root-video.mp4";
    let music_url = "https://example.invalid/root-music.mp3";
    let thumb_url = "https://example.invalid/root-thumb.png";
    let video_info = json!([[
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        [thumb_url, video_url]
    ]]);
    let music_data = json!([
        [
            null,
            [
                null,
                null,
                null,
                null,
                null,
                null,
                null,
                [thumb_url, music_url]
            ]
        ],
        [
            null,
            [
                null,
                null,
                null,
                null,
                null,
                null,
                null,
                [thumb_url, "https://example.invalid/root-music.mp4"]
            ]
        ]
    ]);

    let mut root_video_candidate = vec![Value::Null; 13];
    root_video_candidate[12] = json!({
        "59": [[[video_info]]]
    });
    let root_video_json = serde_json::to_string(&vec![Value::Array(root_video_candidate)]).unwrap();
    let root_video_body = format!(
        ")]}}'\n{}\n{}\n",
        root_video_json.encode_utf16().count(),
        root_video_json
    );
    let video_assets =
        extract_stream_generate_media_assets(&root_video_body, GeminiCanvasMediaOperation::Video)
            .unwrap();
    assert_eq!(video_assets.len(), 1);
    assert_eq!(video_assets[0].kind, "video");
    assert_eq!(video_assets[0].url, video_url);
    assert_eq!(video_assets[0].mime_type, "video/mp4");

    let mut root_music_candidate = vec![Value::Null; 13];
    root_music_candidate[12] = json!({
        "86": music_data
    });
    let root_music_json = serde_json::to_string(&vec![Value::Array(root_music_candidate)]).unwrap();
    let root_music_body = format!(
        ")]}}'\n{}\n{}\n",
        root_music_json.encode_utf16().count(),
        root_music_json
    );
    let music_assets =
        extract_stream_generate_media_assets(&root_music_body, GeminiCanvasMediaOperation::Music)
            .unwrap();
    assert_eq!(music_assets.len(), 1);
    assert_eq!(music_assets[0].kind, "audio");
    assert_eq!(music_assets[0].url, music_url);
    assert_eq!(music_assets[0].mime_type, "audio/mpeg");
}

#[test]
fn extract_stream_generate_media_assets_recovers_music_urls_from_new_87_shape() {
    let music_url =
        "https://contribution.usercontent.google.com/download?filename=ascending_north.mp3";
    let music_preview = "https://lh3.googleusercontent.com/gg-dl/preview-music";
    let video_url =
        "https://contribution.usercontent.google.com/download?filename=ascending_north.mp4";
    let video_preview = "https://lh3.googleusercontent.com/gg-dl/preview-video";
    let music_data = json!([
        [
            null,
            [
                null,
                4,
                "ascending_north.mp3",
                null,
                null,
                "$music-download-token",
                null,
                [music_preview, music_url, music_preview],
                2,
                [1778420728, 123211014],
                null,
                "audio/mpeg",
                null,
                null,
                null,
                null,
                [[]]
            ]
        ],
        [
            null,
            [
                null,
                2,
                "ascending_north.mp4",
                null,
                null,
                "$video-download-token",
                null,
                [video_preview, video_url, video_preview],
                3,
                [1778420728, 223211014],
                null,
                "video/mp4",
                null,
                null,
                null,
                null,
                [[]]
            ]
        ]
    ]);

    let mut frame = vec![Value::Null; 88];
    frame[87] = music_data;
    let frame_json = serde_json::to_string(&vec![Value::Array(frame)]).unwrap();
    let body = format!(
        ")]}}'\n{}\n{}\n",
        frame_json.encode_utf16().count(),
        frame_json
    );

    let assets =
        extract_stream_generate_media_assets(&body, GeminiCanvasMediaOperation::Music).unwrap();
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].kind, "audio");
    assert_eq!(assets[0].url, music_url);
    assert_eq!(assets[0].mime_type, "audio/mpeg");
}

#[test]
fn extract_stream_generate_media_assets_recovers_music_urls_from_wrapped_87_shape() {
    let wrapped_frame = r#"[["wrb.fr",null,"[null,[\"c_f41e19b1a71ccd30\",\"r_b0fc2aaf1a366df8\"],null,null,[[\"rc_572c503ef5e46c34\",[\"\"],null,null,null,null,null,null,[1],null,null,null,[{\"8\":[],\"73\":[null,\"Generating your music...\",null,3],\"87\":[[null,[null,4,\"concrete_maps.mp3\",null,null,\"$AT+3-token\",null,[\"https://lh3.googleusercontent.com/gg-dl/preview-music\",\"https://contribution.usercontent.google.com/download?filename=concrete_maps.mp3\",\"https://lh3.googleusercontent.com/gg-dl/preview-music\"],2,[1778422963,941200288],null,\"audio/mpeg\",null,null,null,null,[[]]]],[null,[null,2,\"concrete_maps.mp4\",null,null,\"$AT+3-video-token\",null,[\"https://lh3.googleusercontent.com/gg-dl/preview-video\",\"https://contribution.usercontent.google.com/download?filename=concrete_maps.mp4\",\"https://lh3.googleusercontent.com/gg-dl/preview-video\"],3,[1778422963,941200288],null,\"video/mp4\",null,null,null,null,[[]]]]]}]]]]"]]"#;

    let body = format!(")]}}'\n{}\n{}\n", wrapped_frame.len(), wrapped_frame);
    let assets =
        extract_stream_generate_media_assets(&body, GeminiCanvasMediaOperation::Music).unwrap();
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].kind, "audio");
    assert_eq!(
        assets[0].url,
        "https://contribution.usercontent.google.com/download?filename=concrete_maps.mp3"
    );
    assert_eq!(assets[0].mime_type, "audio/mpeg");
}

#[test]
fn extract_video_asset_from_candidate_data_falls_back_to_completion_tuple_scan() {
    let asset = extract_video_asset_from_candidate_data(&json!({
            "60": [[[[[
                null,
                2,
                "video.mp4",
                null,
                null,
                "$download-token",
                null,
                [
                    "https://lh3.googleusercontent.com/gg/preview-token",
                    "https://contribution.usercontent.google.com/download?c=xyz789&filename=video.mp4&opi=103135050"
                ],
                2,
                [1775858568, 971367988],
                null,
                "video/mp4",
                null,
                null,
                null,
                null,
                null,
                [[8], 1280, 720]
            ]]]]]
        }))
        .expect("video asset should be recovered from completion tuple scan");

    assert_eq!(asset.kind, "video");
    assert_eq!(asset.mime_type, "video/mp4");
    assert_eq!(
            asset.url,
            "https://contribution.usercontent.google.com/download?c=xyz789&filename=video.mp4&opi=103135050"
        );
}

#[test]
fn extract_music_asset_from_candidate_data_falls_back_to_google_media_url_scan() {
    let asset = extract_music_asset_from_candidate_data(&json!({
        "preview": {
            "urls": [
                "https://lh3.googleusercontent.com/gg/preview-only",
                "https://work.fife.usercontent.google.com/rd-gg-dl/abc123/ivory_rain.mp4"
            ]
        }
    }))
    .expect("music asset should be recovered from google media url scan");

    assert_eq!(asset.kind, "video");
    assert_eq!(asset.mime_type, "video/mp4");
    assert_eq!(
        asset.url,
        "https://work.fife.usercontent.google.com/rd-gg-dl/abc123/ivory_rain.mp4"
    );
}
