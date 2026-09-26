use super::*;

#[test]
fn resolve_producer_video_confirmation_prompt_skips_when_create_job_already_present_contract() {
    let creative_summary = json!({
        "tool_returns": [
            {
                "tool_name": "video__create_music_video",
                "content": { "job_id": "job-123" }
            }
        ]
    });

    let prompt = resolve_producer_video_confirmation_prompt(
        &json!({
            "confirm_prompt": "use this only if needed"
        }),
        &creative_summary,
    )
    .expect("confirmation prompt resolution");

    assert!(prompt.is_none());
}

#[test]
fn resolve_producer_video_confirmation_prompt_reads_protocol_contract_when_needed() {
    let creative_summary = json!({
        "tool_returns": [
            {
                "tool_name": "video__propose_music_video",
                "content": { "status": "ok" }
            }
        ]
    });

    let prompt = resolve_producer_video_confirmation_prompt(
        &json!({
            "confirm_prompt": "approve the generated storyboard"
        }),
        &creative_summary,
    )
    .expect("confirmation prompt resolution");

    assert_eq!(prompt.as_deref(), Some("approve the generated storyboard"));
}

#[test]
fn resolve_producer_video_create_job_id_from_summaries_reads_creative_contract() {
    let creative_summary = json!({
        "tool_returns": [
            {
                "tool_name": "video__propose_music_video",
                "content": { "status": "ok" }
            },
            {
                "tool_name": "video__create_music_video",
                "content": { "job_id": "job-creative" }
            }
        ]
    });

    let job_id = resolve_producer_video_create_job_id_from_summaries(
        &creative_summary,
        None,
        "producer_compatible",
    )
    .expect("creative summary should resolve job id");

    assert_eq!(job_id, "job-creative");
}

#[test]
fn resolve_producer_video_create_job_id_from_summaries_reads_confirmation_fallback_contract() {
    let creative_summary = json!({
        "tool_returns": [
            {
                "tool_name": "video__propose_music_video",
                "content": { "status": "ok" }
            }
        ]
    });
    let confirmation_summary = json!({
        "tool_returns": [
            {
                "tool_name": "video__create_music_video",
                "content": { "jobId": "job-confirm" }
            }
        ]
    });

    let job_id = resolve_producer_video_create_job_id_from_summaries(
        &creative_summary,
        Some(&confirmation_summary),
        "producer_compatible",
    )
    .expect("confirmation summary should resolve job id");

    assert_eq!(job_id, "job-confirm");
}

#[test]
fn select_producer_video_final_url_prefers_job_specific_contract() {
    let payload = json!({
        "items": [
            "https://cdn.example.com/music-video/other-job/render.mp4",
            "https://cdn.example.com/music-video/job-123/render.mp4"
        ]
    });

    assert_eq!(
        select_producer_video_final_url(&payload, "job-123").as_deref(),
        Some("https://cdn.example.com/music-video/job-123/render.mp4")
    );
}

#[test]
fn select_producer_video_final_url_falls_back_to_generic_music_video_contract() {
    let payload = json!({
        "items": [
            "https://cdn.example.com/movie.mov",
            "https://cdn.example.com/music-video/other-job/render.mp4"
        ]
    });

    assert_eq!(
        select_producer_video_final_url(&payload, "job-123").as_deref(),
        Some("https://cdn.example.com/music-video/other-job/render.mp4")
    );
}

#[test]
fn extract_producer_video_final_status_prefers_top_level_contract() {
    let payload = json!({
        "status": "  COMPLETED  ",
        "state": { "status": "failed" }
    });

    assert_eq!(
        extract_producer_video_final_status(&payload),
        "completed".to_string()
    );
}

#[test]
fn extract_producer_video_final_status_falls_back_to_nested_state_contract() {
    let payload = json!({
        "state": { "status": "  CANCELED  " }
    });

    assert_eq!(
        extract_producer_video_final_status(&payload),
        "canceled".to_string()
    );
}

#[test]
fn extract_producer_video_final_status_defaults_to_accepted_contract() {
    let payload = json!({
        "state": {}
    });

    assert_eq!(
        extract_producer_video_final_status(&payload),
        "accepted".to_string()
    );
}

#[test]
fn ensure_successful_producer_video_final_status_accepts_non_terminal_contract() {
    let payload = json!({
        "status": "processing"
    });

    assert!(ensure_successful_producer_video_final_status(
        "producer_compatible",
        "processing",
        &payload
    )
    .is_ok());
}

#[test]
fn ensure_successful_producer_video_final_status_rejects_terminal_contract() {
    let payload = json!({
        "status": "failed",
        "reason": "upstream"
    });

    let error =
        ensure_successful_producer_video_final_status("producer_compatible", "failed", &payload)
            .expect_err("terminal status should fail");

    assert_eq!(error.http_status, Some(500));
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    assert_eq!(error.code.as_deref(), Some("producer_http_video_failed"));
    assert_eq!(
        error.message,
        "Producer video job entered terminal status 'failed'. status payload: {\"reason\":\"upstream\",\"status\":\"failed\"}"
    );
}

#[test]
fn build_producer_video_http_response_preserves_completed_contract() {
    let body = build_producer_video_http_response(
        "producer_compatible",
        "https://www.producer.ai",
        &json!({
            "prompt": "launch trailer",
            "aspect_ratio": "16:9",
            "resolution": "1080p",
            "duration_s": 12
        }),
        "producer-video-model",
        "clip-1",
        "conv-video-1",
        "bootstrap-job-1",
        "creative-job-1",
        Some("confirm-job-1"),
        "video-job-1",
        &json!({
            "status": "completed",
            "preview": { "video": "https://cdn.example.com/preview.mp4" },
            "items": [
                "https://cdn.example.com/music-video/video-job-1/final.mp4"
            ]
        }),
        &json!({ "tool_calls": [] }),
        Some(&json!({ "tool_returns": [] })),
    )
    .expect("completed response should build");

    assert_eq!(body["object"], "video.generation");
    assert_eq!(body["completed"], true);
    assert_eq!(body["job_id"], "video-job-1");
    assert_eq!(
        body["data"][0]["url"],
        "https://cdn.example.com/music-video/video-job-1/final.mp4"
    );
    assert_eq!(body["state"], "completed");
}

#[test]
fn build_producer_video_http_response_rejects_terminal_status_contract() {
    let error = build_producer_video_http_response(
        "producer_compatible",
        "https://www.producer.ai",
        &json!({}),
        "producer-video-model",
        "clip-1",
        "conv-video-1",
        "bootstrap-job-1",
        "creative-job-1",
        None,
        "video-job-1",
        &json!({
            "status": "failed",
            "reason": "upstream"
        }),
        &json!({}),
        None,
    )
    .expect_err("terminal status should fail");

    assert_eq!(error.http_status, Some(500));
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    assert_eq!(error.code.as_deref(), Some("producer_http_video_failed"));
}
