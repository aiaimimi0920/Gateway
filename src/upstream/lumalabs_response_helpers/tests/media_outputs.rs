use super::*;

#[test]
fn classify_lumalabs_media_fetch_error_preserves_generic_upstream_contract() {
    let error = classify_lumalabs_media_fetch_error(503, "service unavailable");

    assert_eq!(error.provider_name.as_deref(), Some("lumalabs_compatible"));
    assert_eq!(error.http_status, Some(503));
    assert!(error.message.contains("service unavailable"));
}

#[test]
fn ensure_successful_lumalabs_media_fetch_status_accepts_2xx_contract() {
    ensure_successful_lumalabs_media_fetch_status(204, "").expect("2xx should pass");
}

#[test]
fn ensure_successful_lumalabs_media_fetch_status_preserves_failure_contract() {
    let error = ensure_successful_lumalabs_media_fetch_status(503, "service unavailable")
        .expect_err("non-2xx should fail");

    assert_eq!(error.provider_name.as_deref(), Some("lumalabs_compatible"));
    assert_eq!(error.http_status, Some(503));
    assert!(error.message.contains("service unavailable"));
}

#[test]
fn resolve_lumalabs_downloaded_image_mime_type_prefers_image_header_contract() {
    let mut headers = rquest::header::HeaderMap::new();
    headers.insert(
        rquest::header::CONTENT_TYPE,
        rquest::header::HeaderValue::from_static("image/webp"),
    );

    let mime_type =
        resolve_lumalabs_downloaded_image_mime_type(&headers, "https://cdn.example.com/render.png");

    assert_eq!(mime_type, "image/webp");
}

#[test]
fn resolve_lumalabs_downloaded_image_mime_type_falls_back_to_signed_url_contract() {
    let headers = rquest::header::HeaderMap::new();

    let mime_type =
        resolve_lumalabs_downloaded_image_mime_type(&headers, "https://cdn.example.com/render.jpg");

    assert_eq!(mime_type, "image/jpeg");
}

#[test]
fn resolve_lumalabs_image_generation_plan_preserves_url_response_contract() {
    let req = image_generation_request(serde_json::json!({}));

    let response = resolve_lumalabs_image_generation_plan(
        &req,
        "crystal portrait",
        "https://cdn.example.com/luma.png",
    )
    .expect("image plan")
    .expect("url response");

    assert_eq!(
        response["data"][0]["url"],
        "https://cdn.example.com/luma.png"
    );
    assert_eq!(response["data"][0]["revised_prompt"], "crystal portrait");
}

#[test]
fn resolve_lumalabs_image_generation_plan_preserves_b64_contract() {
    let req = image_generation_request(serde_json::json!({ "response_format": "b64_json" }));

    let response = resolve_lumalabs_image_generation_plan(
        &req,
        "crystal portrait",
        "https://cdn.example.com/luma.png",
    )
    .expect("image plan");

    assert!(response.is_none());
}

#[test]
fn build_lumalabs_non_image_generation_response_preserves_video_contract() {
    let body = build_lumalabs_non_image_generation_response(
        crate::protocol::lumalabs::LumalabsMediaOperation::Video,
        "ray3.14",
        "ocean flythrough",
        "https://cdn.example.com/video.mp4",
    );

    assert_eq!(body["object"], "video.generation");
    assert_eq!(body["model"], "ray3.14");
    assert_eq!(body["prompt"], "ocean flythrough");
    assert_eq!(body["data"][0]["url"], "https://cdn.example.com/video.mp4");
}

#[test]
fn build_lumalabs_non_image_generation_response_preserves_audio_contract() {
    let body = build_lumalabs_non_image_generation_response(
        crate::protocol::lumalabs::LumalabsMediaOperation::Audio,
        "elevenlabs-music-v1",
        "ambient pulse",
        "https://cdn.example.com/audio.mp3",
    );

    assert_eq!(body["object"], "audio.generation");
    assert_eq!(body["model"], "elevenlabs-music-v1");
    assert_eq!(body["prompt"], "ambient pulse");
    assert_eq!(body["data"][0]["url"], "https://cdn.example.com/audio.mp3");
}

#[test]
fn build_lumalabs_downloaded_image_response_preserves_header_mime_contract() {
    let mut headers = rquest::header::HeaderMap::new();
    headers.insert(
        rquest::header::CONTENT_TYPE,
        rquest::header::HeaderValue::from_static("image/webp"),
    );

    let body = build_lumalabs_downloaded_image_response(
        "luma portrait",
        "https://cdn.example.com/render.png",
        &headers,
        b"png-bytes",
    );

    assert_eq!(body["data"][0]["revised_prompt"], "luma portrait");
    assert_eq!(body["data"][0]["mime_type"], "image/webp");
    assert!(body["data"][0]["b64_json"].as_str().is_some());
}

#[test]
fn build_lumalabs_downloaded_image_response_falls_back_to_signed_url_contract() {
    let headers = rquest::header::HeaderMap::new();

    let body = build_lumalabs_downloaded_image_response(
        "luma portrait",
        "https://cdn.example.com/render.jpg",
        &headers,
        b"jpg-bytes",
    );

    assert_eq!(body["data"][0]["mime_type"], "image/jpeg");
}
