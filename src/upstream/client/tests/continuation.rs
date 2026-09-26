use super::*;

#[test]
fn gemini_canvas_stream_collection_hands_video_off_at_locator() {
    let (music, music_handoff) = gemini_canvas_stream_collection_policy(
        gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_MUSIC_MODE_INDEX,
        false,
    );
    assert_eq!(music, gemini_canvas::GeminiCanvasMediaOperation::Music);
    assert!(!music_handoff);

    let (video, video_handoff) = gemini_canvas_stream_collection_policy(
        gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_VIDEO_MODE_INDEX,
        false,
    );
    assert_eq!(video, gemini_canvas::GeminiCanvasMediaOperation::Video);
    assert!(video_handoff);
}

#[test]
fn gemini_canvas_stream_collection_keeps_image_generation_open_until_asset() {
    let (image, image_handoff) = gemini_canvas_stream_collection_policy(
        gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX,
        false,
    );
    assert_eq!(image, gemini_canvas::GeminiCanvasMediaOperation::Image);
    assert!(!image_handoff);

    let (image_edit, image_edit_handoff) = gemini_canvas_stream_collection_policy(
        gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX,
        true,
    );
    assert_eq!(image_edit, gemini_canvas::GeminiCanvasMediaOperation::Image);
    assert!(image_edit_handoff);
}

#[test]
fn gemini_canvas_video_continuation_reads_completed_locator_fields() {
    let mut req = make_request(ProtocolFamily::OpenAi, EndpointKind::VideosGenerations);
    req.raw_body = json!({
        "prompt": "video",
        "conversation_id": "c_065d5bde601ff3af",
        "response_id": "r_f7c7e549d11c849b",
        "app_path": "/app/065d5bde601ff3af",
        "job_id": "5cb5f42d-d5a9-4643-99fe-0123456789ab",
    });

    let continuation = gemini_canvas_video_continuation_from_request(&req)
        .expect("valid continuation")
        .expect("continuation fields");

    assert_eq!(continuation.conversation_id, "c_065d5bde601ff3af");
    assert_eq!(continuation.response_id, "r_f7c7e549d11c849b");
    assert_eq!(continuation.app_path, "/app/065d5bde601ff3af");
    assert_eq!(
        continuation.job_id.as_deref(),
        Some("5cb5f42d-d5a9-4643-99fe-0123456789ab")
    );
    let seed_body = build_gemini_canvas_video_continuation_seed_body(&continuation);
    assert_eq!(
        gemini_canvas::extract_video_generation_job_id(&seed_body).as_deref(),
        Some("5cb5f42d-d5a9-4643-99fe-0123456789ab")
    );
    assert!(gemini_canvas::response_indicates_video_generation_pending(
        &seed_body
    ));

    let mut browser_input = json!({ "operation": "video" });
    apply_gemini_canvas_browser_video_continuation(&mut browser_input, &continuation);
    assert_eq!(browser_input["resumeExistingMedia"], true);
    assert_eq!(browser_input["conversationId"], "c_065d5bde601ff3af");
    assert_eq!(browser_input["responseId"], "r_f7c7e549d11c849b");
    assert_eq!(browser_input["appPath"], "/app/065d5bde601ff3af");
    assert_eq!(
        browser_input["jobId"],
        "5cb5f42d-d5a9-4643-99fe-0123456789ab"
    );
}

#[test]
fn gemini_canvas_video_continuation_rejects_partial_or_unsafe_locator_fields() {
    let mut partial = make_request(ProtocolFamily::OpenAi, EndpointKind::VideosGenerations);
    partial.raw_body = json!({
        "prompt": "video",
        "conversation_id": "c_065d5bde601ff3af",
    });
    let partial_error = gemini_canvas_video_continuation_from_request(&partial)
        .expect_err("partial continuation must fail");
    assert_eq!(partial_error.http_status, Some(400));
    assert_eq!(
        partial_error.code.as_deref(),
        Some("invalid_gemini_canvas_video_continuation")
    );

    let mut unsafe_path = make_request(ProtocolFamily::OpenAi, EndpointKind::VideosGenerations);
    unsafe_path.raw_body = json!({
        "prompt": "video",
        "conversation_id": "c_065d5bde601ff3af",
        "response_id": "r_f7c7e549d11c849b",
        "app_path": "/app/065d5bde601ff3af?redirect=unsafe",
    });
    let path_error = gemini_canvas_video_continuation_from_request(&unsafe_path)
        .expect_err("unsafe continuation path must fail");
    assert_eq!(path_error.http_status, Some(400));
    assert_eq!(
        path_error.code.as_deref(),
        Some("invalid_gemini_canvas_video_continuation")
    );
}
