use super::followup_test_support::make_gemini_bootstrap;
use super::*;
use serde_json::json;

#[test]
fn finalize_gemini_canvas_media_followup_attempt_state_prefers_error_over_body() {
    let target = GeminiCanvasFollowupTarget::from_bootstrap(
        false,
        Some("/app/from-bootstrap"),
        Some(gemini_canvas::GeminiCanvasStreamGenerateLocator {
            response_id: "r_followup".to_string(),
            conversation_id: "c_followup".to_string(),
            app_path: "/app/from-bootstrap".to_string(),
        }),
        GeminiCanvasPageTargetMode::Resolved,
    );
    let state = GeminiCanvasMediaFollowupAttemptState {
        last_body: Some("{\"status\":\"pending\"}".to_string()),
        last_error: Some(
            GatewayError::service_unavailable("usable asset missing")
                .with_provider("gemini_canvas_compatible")
                .with_code("gemini_canvas_media_followup_missing_asset"),
        ),
    };

    let error = finalize_gemini_canvas_media_followup_attempt_state(
        "gemini_canvas_compatible",
        &target,
        "https://gemini.google.com/app",
        state,
    )
    .expect_err("follow-up finalize should preserve richer error");

    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_media_followup_missing_asset")
    );
    assert!(error.message.contains("usable asset missing"));
}

#[test]
fn build_gemini_canvas_media_followup_missing_asset_error_keeps_body_preview() {
    let target = GeminiCanvasFollowupTarget::from_bootstrap(
        false,
        Some("/app/from-bootstrap"),
        Some(gemini_canvas::GeminiCanvasStreamGenerateLocator {
            response_id: "r_followup".to_string(),
            conversation_id: "c_followup".to_string(),
            app_path: "/app/from-bootstrap".to_string(),
        }),
        GeminiCanvasPageTargetMode::Resolved,
    );

    let error = build_gemini_canvas_media_followup_missing_asset_error(
        "gemini_canvas_compatible",
        gemini_canvas::GeminiCanvasMediaOperation::Video,
        &target,
        "https://gemini.google.com/app",
        2,
        "{\"status\":\"pending\",\"detail\":\"still rendering\"}",
    );

    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_media_followup_missing_asset")
    );
    assert!(error.message.contains("attempt=2"));
    assert!(error.message.contains("still rendering"));
}

#[test]
fn plan_gemini_canvas_video_completion_attempt_sleep_matches_pending_contract() {
    assert_eq!(
        plan_gemini_canvas_video_completion_attempt_sleep(true, false, 1),
        Some(Duration::from_secs(8))
    );
    assert_eq!(
        plan_gemini_canvas_video_completion_attempt_sleep(false, true, 1),
        Some(Duration::from_secs(8))
    );
    assert_eq!(
        plan_gemini_canvas_video_completion_attempt_sleep(false, false, 1),
        Some(Duration::from_secs(4))
    );
    assert_eq!(
        plan_gemini_canvas_video_completion_attempt_sleep(false, false, 3),
        None
    );
}

#[test]
fn build_gemini_canvas_video_completion_missing_asset_error_keeps_previews() {
    let error = build_gemini_canvas_video_completion_missing_asset_error(
        "/app/video",
        "c_video",
        "r_video",
        4,
        Some("job_123"),
        Some("{\"status\":\"pending\"}"),
        Some("{\"meta\":\"still rendering\"}"),
    );

    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_video_completion_followup_missing_asset")
    );
    assert!(error.message.contains("job_id=job_123"));
    assert!(error.message.contains("pending"));
    assert!(error.message.contains("still rendering"));
}

#[test]
fn build_gemini_canvas_direct_http_video_missing_asset_error_matches_contract() {
    let error =
        build_gemini_canvas_direct_http_video_missing_asset_error("gemini_canvas_compatible");
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(error.code.as_deref(), Some("gemini_canvas_no_video_asset"));
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas direct HTTP video generation completed without returning a video asset."
    );
}

#[test]
fn extract_gemini_canvas_video_operation_download_uri_reports_standard_contracts() {
    let missing_payload = json!({
        "response": {}
    });
    let missing_payload_error = extract_gemini_canvas_video_operation_download_uri(
        "gemini_canvas_compatible",
        &missing_payload,
    )
    .expect_err("missing payload should fail");
    assert_eq!(
        missing_payload_error.code.as_deref(),
        Some("gemini_canvas_no_video_asset")
    );
    assert_eq!(
        missing_payload_error.message.as_str(),
        "Gemini Canvas video operation completed without a downloadable video payload."
    );

    let missing_uri = json!({
        "response": {
            "generatedVideos": [
                { "video": {} }
            ]
        }
    });
    let missing_uri_error = extract_gemini_canvas_video_operation_download_uri(
        "gemini_canvas_compatible",
        &missing_uri,
    )
    .expect_err("missing uri should fail");
    assert_eq!(
        missing_uri_error.code.as_deref(),
        Some("gemini_canvas_no_video_asset")
    );
    assert_eq!(
        missing_uri_error.message.as_str(),
        "Gemini Canvas video operation completed without video.uri."
    );

    let uri_payload = json!({
        "response": {
            "generatedVideos": [
                { "video": { "uri": " https://example.com/video.mp4 " } }
            ]
        }
    });
    let uri = extract_gemini_canvas_video_operation_download_uri(
        "gemini_canvas_compatible",
        &uri_payload,
    )
    .expect("uri should exist");
    assert_eq!(uri, "https://example.com/video.mp4");
}

#[test]
fn build_gemini_canvas_video_completion_requests_recovers_job_and_rpcs() {
    let bootstrap = make_gemini_bootstrap();
    let primary_body = r#"
            )]}'
            [["wrb.fr",null,"[null,[\"c_98256aabc450ff93\",\"r_931fae4dfff02e00\"],null,null,[[\"rc_pending\",[\"正在生成视频，这可能需要几分钟时间，请稍后回来查看完成状态。\nhttp://googleusercontent.com/video_gen_chip/0\n\"],null,null,null,null,null,null,[1],\"zh\",null,null,[{\"65\":[[\"http://googleusercontent.com/video_gen_chip/0\"],\"e3136f2f-29f4-4f33-9223-271d4697c60a\"]}]]]]"]]
        "#;

    let requests = build_gemini_canvas_video_completion_requests(
        &bootstrap,
        "/app/video",
        "c_98256aabc450ff93",
        "r_931fae4dfff02e00",
        primary_body,
    )
    .expect("video completion requests");

    assert_eq!(
        requests.job_id.as_deref(),
        Some("e3136f2f-29f4-4f33-9223-271d4697c60a")
    );
    assert!(requests
        .job_poll_request
        .as_ref()
        .expect("job poll request")
        .as_ref()
        .expect("job poll request result")
        .query
        .iter()
        .any(|(key, value)| key == "rpcids" && value == "kwDCne"));
    assert!(requests
        .completion_request
        .query
        .iter()
        .any(|(key, value)| key == "rpcids" && value == "hNvQHb"));
    assert!(requests
        .metadata_request
        .query
        .iter()
        .any(|(key, value)| key == "rpcids" && value == "MUAZcd"));
}

#[test]
fn gemini_canvas_video_completion_poll_state_tracks_attempt_progress() {
    let mut poll_state = GeminiCanvasVideoCompletionPollState::default();
    assert_eq!(poll_state.next_attempt(), 1);

    let next_sleep_for =
        poll_state.record_attempt_progress(&GeminiCanvasVideoCompletionAttemptState {
            completion: GeminiCanvasVideoStageProgress {
                body: "{\"status\":\"pending\"}".to_string(),
                pending: true,
            },
            metadata: Some(GeminiCanvasVideoStageProgress {
                body: "{\"meta\":\"settling\"}".to_string(),
                pending: false,
            }),
        });

    assert_eq!(next_sleep_for, Some(Duration::from_secs(8)));
    let error =
        poll_state.build_missing_asset_error("/app/video", "c_video", "r_video", Some("job_456"));
    assert!(error.message.contains("job_id=job_456"));
    assert!(error.message.contains("pending"));
    assert!(error.message.contains("settling"));
}

#[test]
fn classify_gemini_canvas_video_stage_body_marks_pending_progress() {
    let body = r#"
            )]}'
            [["wrb.fr",null,"[null,[\"c_98256aabc450ff93\",\"r_931fae4dfff02e00\"],null,null,[[\"rc_pending\",[\"正在生成视频，这可能需要几分钟时间，请稍后回来查看完成状态。\nhttp://googleusercontent.com/video_gen_chip/0\n\"],null,null,null,null,null,null,[1],\"zh\",null,null,[{\"65\":[[\"http://googleusercontent.com/video_gen_chip/0\"],\"e3136f2f-29f4-4f33-9223-271d4697c60a\"]}]]]]"]]
        "#
    .to_string();

    let progress = classify_gemini_canvas_video_stage_body(body)
        .expect_err("pending body should stay in progress contract");
    assert!(progress.pending);
    assert!(progress.body.contains("video_gen_chip"));
}

#[test]
fn classify_gemini_canvas_video_stage_body_accepts_page_blob_asset() {
    let body = r#"
            <video
                src="https:\/\/contribution.usercontent.google.com\/download?c\u003dxyz789\u0026filename\u003dvideo.mp4\u0026opi\u003d103135050">
            </video>
        "#
    .to_string();

    let result = classify_gemini_canvas_video_stage_body(body.clone())
        .expect("video asset body should complete");
    assert_eq!(result, body);
}

#[test]
fn build_gemini_canvas_video_accepted_response_from_body_prefers_stream_locator() {
    let body = concat!(
        ")]}'\n\n",
        "126\n",
        "[[\"wrb.fr\",null,\"[null,[\\\"c_stream\\\",\\\"r_stream\\\"],null,null,[[\\\"rc_done\\\",[\\\"ready\\\"],null,null,null,null,null,null,[1],\\\"zh\\\",null,null,[{\\\"65\\\":[[\\\"/app/video-stream\\\"],\\\"job-1\\\"]}]]]]\"]]\n"
    );

    let response = build_gemini_canvas_video_accepted_response_from_body(
        "gemini-2.5-pro",
        "make a clip",
        body,
        Some("c_fallback"),
        Some("r_fallback"),
        Some("/app/fallback"),
    );

    assert_eq!(response["provider"], json!("gemini_canvas"));
    assert_eq!(response["conversation_id"], json!("c_stream"));
    assert_eq!(response["response_id"], json!("r_stream"));
    assert_eq!(response["app_path"], json!("/app/video-stream"));
    assert_eq!(response["job_id"], json!("job-1"));
}
