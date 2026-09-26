use super::*;

#[test]
fn collect_image_media_assets_filters_only_image_entries() {
    let result = make_media_result(vec![
        make_media_asset("image", "https://example.com/image-1.png", "image/png"),
        make_media_asset("video", "https://example.com/video.mp4", "video/mp4"),
        make_media_asset("image", "https://example.com/image-2.png", "image/png"),
    ]);

    let assets = collect_image_media_assets(&result);
    assert_eq!(assets.len(), 2);
    assert!(assets.iter().all(|asset| asset.kind == "image"));
}

#[test]
fn collect_converted_image_media_assets_keeps_inline_payload() {
    let mut inline = make_media_asset("image", "https://example.com/image.png", "image/png");
    inline.body_base64 = Some(base64::engine::general_purpose::STANDARD.encode(b"png"));
    let result = make_media_result(vec![inline]);
    let assets = collect_converted_image_media_assets(&result);
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].mime_type, "image/png");
    assert_eq!(assets[0].body_base64.as_deref(), Some("cG5n"));
}

#[test]
fn collect_converted_image_media_assets_preserves_metadata_fields() {
    let mut asset = make_media_asset("image", "https://example.com/image.png", "image/png");
    asset.alt = Some("preview".to_string());
    asset.width = Some(640);
    asset.height = Some(480);

    let result = make_media_result(vec![asset]);
    let assets = collect_converted_image_media_assets(&result);
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].url, "https://example.com/image.png");
    assert_eq!(assets[0].alt.as_deref(), Some("preview"));
    assert_eq!(assets[0].width, Some(640));
    assert_eq!(assets[0].height, Some(480));
}

#[test]
fn decode_inline_image_asset_decodes_bytes_and_preserves_mime() {
    let mut asset = make_converted_image_asset("https://example.com/image.png", "image/png");
    asset.body_base64 = Some(base64::engine::general_purpose::STANDARD.encode(b"png"));

    let image = decode_inline_image_asset(
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        &asset,
        "invalid_inline",
        "inline image",
    )
    .expect("decode should succeed")
    .expect("inline image should exist");

    assert_eq!(image.mime_type, "image/png");
    assert_eq!(image.bytes, b"png");
}

#[test]
fn decode_inline_image_asset_returns_none_without_inline_payload() {
    let asset = make_converted_image_asset("https://example.com/image.png", "image/png");

    let image = decode_inline_image_asset(
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        &asset,
        "invalid_inline",
        "inline image",
    )
    .expect("decode should succeed");

    assert!(image.is_none());
}

#[test]
fn decode_inline_image_asset_reports_configured_error_code() {
    let mut asset = make_converted_image_asset("https://example.com/image.png", "image/png");
    asset.body_base64 = Some("not-base64".to_string());

    let error = decode_inline_image_asset(
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        &asset,
        "invalid_inline",
        "inline image",
    )
    .expect_err("invalid base64 should fail");

    assert_eq!(error.code.as_deref(), Some("invalid_inline"));
    assert!(error.message.contains("invalid inline image bytes"));
}

#[test]
fn decode_browser_pool_inline_image_asset_reports_standard_contract() {
    let mut asset = make_converted_image_asset("https://example.com/image.png", "image/png");
    asset.body_base64 = Some("not-base64".to_string());

    let error =
        decode_browser_pool_inline_image_asset(GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER, &asset)
            .expect_err("invalid base64 should fail");

    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_invalid_inline_image_bytes")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas browser pool returned invalid inline image bytes: Invalid symbol 45, offset 3."
    );
}

#[test]
fn decode_direct_http_inline_image_asset_reports_standard_contract() {
    let mut asset = make_converted_image_asset("https://example.com/image.png", "image/png");
    asset.body_base64 = Some("not-base64".to_string());

    let error = decode_direct_http_inline_image_asset("gemini_canvas_compatible", &asset)
        .expect_err("invalid base64 should fail");

    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_invalid_inline_image_bytes")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas direct HTTP image asset contained invalid base64 bytes returned invalid inline image bytes: Invalid symbol 45, offset 3."
    );
}

#[tokio::test]
async fn build_image_generation_response_from_invocation_uses_url_mode_and_requested_count() {
    let result = make_media_result(vec![
        make_media_asset("image", "https://example.com/image-1.png", "image/png"),
        make_media_asset("image", "https://example.com/image-2.png", "image/png"),
    ]);
    let mut req = make_media_request(EndpointKind::ImagesGenerations);
    req.raw_body = json!({
        "response_format": "url",
        "n": 1
    });

    let response = build_image_generation_response_from_invocation(
        &rquest::Client::new(),
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        &req,
        "bright cube",
        &result,
        std::time::Duration::from_secs(5),
    )
    .await
    .expect("url image response");

    let data = response["data"].as_array().expect("data array");
    assert_eq!(data.len(), 1);
    assert_eq!(data[0]["url"], "https://example.com/image-1.png");
    assert_eq!(data[0]["mime_type"], "image/png");
    assert_eq!(data[0]["revised_prompt"], "bright cube");
}

#[tokio::test]
async fn build_image_generation_response_from_invocation_uses_inline_b64_mode() {
    let mut inline = make_media_asset("image", "https://example.com/image-1.png", "image/png");
    inline.body_base64 = Some(base64::engine::general_purpose::STANDARD.encode(b"png-1"));
    let mut second = make_media_asset("image", "https://example.com/image-2.png", "image/png");
    second.body_base64 = Some(base64::engine::general_purpose::STANDARD.encode(b"png-2"));
    let result = make_media_result(vec![inline, second]);
    let mut req = make_media_request(EndpointKind::ImagesGenerations);
    req.raw_body = json!({
        "response_format": "b64_json",
        "n": 1
    });

    let response = build_image_generation_response_from_invocation(
        &rquest::Client::new(),
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        &req,
        "bright cube",
        &result,
        std::time::Duration::from_secs(5),
    )
    .await
    .expect("inline image response");

    let data = response["data"].as_array().expect("data array");
    assert_eq!(data.len(), 1);
    assert_eq!(
        data[0]["b64_json"],
        base64::engine::general_purpose::STANDARD.encode(b"png-1")
    );
    assert_eq!(data[0]["mime_type"], "image/png");
}

#[tokio::test]
async fn build_image_generation_response_from_invocation_errors_without_image_asset() {
    let result = make_media_result(vec![make_media_asset(
        "video",
        "https://example.com/video.mp4",
        "video/mp4",
    )]);
    let req = make_media_request(EndpointKind::ImagesGenerations);

    let error = build_image_generation_response_from_invocation(
        &rquest::Client::new(),
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        &req,
        "bright cube",
        &result,
        std::time::Duration::from_secs(5),
    )
    .await
    .expect_err("missing image asset should fail");

    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_modular_no_image_asset")
    );
}

#[tokio::test]
async fn build_image_generation_response_from_invocation_reports_invalid_inline_bytes() {
    let mut inline = make_media_asset("image", "https://example.com/image-1.png", "image/png");
    inline.body_base64 = Some("not-base64".to_string());
    let result = make_media_result(vec![inline]);
    let mut req = make_media_request(EndpointKind::ImagesGenerations);
    req.raw_body = json!({
        "response_format": "b64_json"
    });

    let error = build_image_generation_response_from_invocation(
        &rquest::Client::new(),
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        &req,
        "bright cube",
        &result,
        std::time::Duration::from_secs(5),
    )
    .await
    .expect_err("invalid inline bytes should fail");

    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_modular_invalid_inline_image_bytes")
    );
}

#[tokio::test]
async fn build_image_generation_response_from_invocation_validates_response_format() {
    let mut inline = make_media_asset("image", "https://example.com/image-1.png", "image/png");
    inline.body_base64 = Some(base64::engine::general_purpose::STANDARD.encode(b"png-1"));
    let result = make_media_result(vec![inline]);

    let mut invalid_req = make_media_request(EndpointKind::ImagesGenerations);
    invalid_req.raw_body = json!({
        "response_format": 123
    });
    let invalid_error = build_image_generation_response_from_invocation(
        &rquest::Client::new(),
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        &invalid_req,
        "bright cube",
        &result,
        std::time::Duration::from_secs(5),
    )
    .await
    .expect_err("non-string response_format should fail");
    assert_eq!(
        invalid_error.code.as_deref(),
        Some("invalid_image_response_format")
    );

    let mut unsupported_req = make_media_request(EndpointKind::ImagesGenerations);
    unsupported_req.raw_body = json!({
        "response_format": "stream"
    });
    let unsupported_error = build_image_generation_response_from_invocation(
        &rquest::Client::new(),
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        &unsupported_req,
        "bright cube",
        &result,
        std::time::Duration::from_secs(5),
    )
    .await
    .expect_err("unsupported response_format should fail");
    assert_eq!(
        unsupported_error.code.as_deref(),
        Some("unsupported_image_response_format")
    );
}

#[test]
fn select_downloaded_image_mime_type_prefers_image_content_type() {
    assert_eq!(
        select_downloaded_image_mime_type("image/png", Some(" image/webp ")),
        "image/webp"
    );
}

#[test]
fn select_downloaded_image_mime_type_falls_back_for_missing_or_non_image_header() {
    assert_eq!(
        select_downloaded_image_mime_type("image/png", Some("application/octet-stream")),
        "image/png"
    );
    assert_eq!(
        select_downloaded_image_mime_type("image/png", None),
        "image/png"
    );
}

#[test]
fn build_downloaded_image_from_bytes_uses_selected_mime_and_body() {
    let asset = make_converted_image_asset("https://example.com/image.png", "image/png");
    let image = build_downloaded_image_from_bytes(&asset, Some("image/jpeg"), b"jpeg-bytes");
    assert_eq!(image.mime_type, "image/jpeg");
    assert_eq!(image.bytes, b"jpeg-bytes");
}

#[test]
fn require_music_media_asset_accepts_video_or_audio_assets() {
    let result = make_media_result(vec![
        make_media_asset("image", "https://example.com/image.png", "image/png"),
        make_media_asset("audio", "https://example.com/audio.wav", "audio/wav"),
    ]);

    let asset = require_music_media_asset(&result, GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER)
        .expect("music asset");
    assert_eq!(asset.kind, "audio");
}

#[test]
fn require_music_media_asset_reports_modular_missing_asset_code() {
    let result = make_media_result(vec![make_media_asset(
        "image",
        "https://example.com/image.png",
        "image/png",
    )]);

    let error = require_music_media_asset(&result, GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER)
        .expect_err("missing music asset should error");
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_modular_no_music_asset")
    );
}

#[test]
fn require_video_media_asset_only_accepts_video_entries() {
    let result = make_media_result(vec![
        make_media_asset("audio", "https://example.com/audio.wav", "audio/wav"),
        make_media_asset("video", "https://example.com/video.mp4", "video/mp4"),
    ]);

    let asset = require_video_media_asset(&result, GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER)
        .expect("video asset");
    assert_eq!(asset.kind, "video");
}

#[test]
fn require_video_media_asset_reports_modular_missing_asset_code() {
    let result = make_media_result(vec![make_media_asset(
        "audio",
        "https://example.com/audio.wav",
        "audio/wav",
    )]);

    let error = require_video_media_asset(&result, GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER)
        .expect_err("missing video asset should error");
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_modular_no_video_asset")
    );
}

#[test]
fn build_music_generation_response_from_invocation_uses_selected_asset() {
    let result = make_media_result(vec![make_media_asset(
        "audio",
        "https://example.com/audio.wav",
        "audio/wav",
    )]);
    let response = build_music_generation_response_from_invocation(
        "gemini-2.5-music",
        "bright synth pulse",
        &result,
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
    )
    .expect("music response");
    assert_eq!(response["object"], "music.generation");
    assert_eq!(response["data"][0]["mime_type"], "audio/wav");
}

#[test]
fn build_music_generation_response_from_invocation_marks_pending_when_only_video_and_busy() {
    let mut result = make_media_result(vec![make_media_asset(
        "video",
        "https://example.com/music-preview.mp4",
        "video/mp4",
    )]);
    result.app_path = Some("/app/canvas-music".to_string());
    result.conversation_id = Some("c_canvas_music".to_string());
    result.response_id = Some("r_canvas_music".to_string());
    result.body_text = Some("I've hit a bit of a snag".to_string());

    let response = build_music_generation_response_from_invocation(
        "gemini-2.5-music",
        "bright synth pulse",
        &result,
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
    )
    .expect("accepted pending music response");

    assert_eq!(response["object"], "music.generation");
    assert_eq!(response["accepted"], true);
    assert_eq!(response["completed"], false);
    assert_eq!(response["conversation_id"], "c_canvas_music");
    assert_eq!(response["response_id"], "r_canvas_music");
}

#[test]
fn build_music_generation_response_from_invocation_marks_pending_for_generation_tokens() {
    let mut result = make_media_result(vec![make_media_asset(
        "video",
        "https://example.com/music-preview.mp4",
        "video/mp4",
    )]);
    result.app_path = Some("/app/canvas-music".to_string());
    result.conversation_id = Some("c_canvas_music".to_string());
    result.response_id = Some("r_canvas_music".to_string());
    result.body_text = Some(
        "[null,[\"c_canvas_music\",\"r_canvas_music\"],{\"11\":[\"Electronic Music Cue Generation\"],\"26\":\"AwAAAAAAAAAQwBHO-LzoF6Ltg6rx4Bk\",\"44\":true}]"
            .to_string(),
    );

    let response = build_music_generation_response_from_invocation(
        "gemini-2.5-music",
        "bright synth pulse",
        &result,
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
    )
    .expect("accepted pending music token response");

    assert_eq!(response["accepted"], true);
    assert_eq!(response["completed"], false);
    assert_eq!(response["conversation_id"], "c_canvas_music");
    assert_eq!(response["response_id"], "r_canvas_music");
}

#[test]
fn build_video_generation_response_from_invocation_uses_selected_asset() {
    let mut result = make_media_result(vec![make_media_asset(
        "video",
        "https://example.com/video.mp4",
        "video/mp4",
    )]);
    result.body_text = Some("video ready".to_string());
    let response = build_video_generation_response_from_invocation(
        "gemini-2.5-video",
        "rotating cube",
        &result,
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
    )
    .expect("video response");
    assert_eq!(response["object"], "video.generation");
    assert_eq!(response["data"][0]["mime_type"], "video/mp4");
}

#[test]
fn build_video_generation_response_from_invocation_marks_pending_when_only_locator_frames_exist() {
    let mut result = make_media_result(vec![]);
    result.body_text = Some(
        "[\"wrb.fr\",null,\"[null,[null,\\\"r_canvas_video\\\"],{\\\"18\\\":\\\"r_canvas_video\\\",\\\"21\\\":[\\\"123e4567-e89b-12d3-a456-426614174000\\\"],\\\"44\\\":true}]\"]".to_string(),
    );
    result.conversation_id = Some("c_canvas_video".to_string());
    result.response_id = Some("r_canvas_video".to_string());
    result.app_path = Some("/app/canvas-video".to_string());

    let response = build_video_generation_response_from_invocation(
        "gemini-2.5-video",
        "rotating cube",
        &result,
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
    )
    .expect("video pending response");
    assert_eq!(response["object"], "video.generation");
    assert_eq!(response["accepted"], true);
    assert_eq!(response["completed"], false);
    assert_eq!(response["conversation_id"], "c_canvas_video");
    assert_eq!(response["response_id"], "r_canvas_video");
}
