use super::*;

#[test]
fn producer_summary_helpers_detect_tool_name_and_job_id() {
    let summary = json!({
        "tool_calls": [
            { "tool_name": " video__propose_music_video " }
        ],
        "tool_returns": [
            {
                "tool_name": "video__create_music_video",
                "content": { "jobId": "job-camel" }
            },
            {
                "tool_name": "video__create_music_video_alt",
                "content": { "job_id": "job-snake" }
            }
        ]
    });
    assert!(producer_summary_has_tool_name(
        &summary,
        "tool_calls",
        "video__propose_music_video"
    ));
    assert_eq!(
        producer_summary_find_tool_return_job_id(&summary, "video__create_music_video").as_deref(),
        Some("job-camel")
    );
    assert_eq!(
        producer_summary_find_tool_return_job_id(&summary, "video__create_music_video_alt")
            .as_deref(),
        Some("job-snake")
    );
}

#[test]
fn producer_collect_media_urls_filters_video_assets() {
    let payload = json!({
        "audioUrl": "https://cdn.example.com/audio.mp3",
        "items": [
            "https://cdn.example.com/render.mp4",
            { "preview": "https://cdn.example.com/movie.mov" },
            { "page": "https://cdn.example.com/music-video/123" }
        ]
    });
    assert_eq!(
        producer_collect_media_urls(&payload),
        vec![
            "https://cdn.example.com/render.mp4".to_string(),
            "https://cdn.example.com/movie.mov".to_string(),
            "https://cdn.example.com/music-video/123".to_string(),
        ]
    );
}

#[test]
fn extract_producer_conversation_id_trims_valid_contract() {
    let summary = json!({
        "conversation_id": "  convo-123  "
    });

    assert_eq!(
        extract_producer_conversation_id(&summary).as_deref(),
        Some("convo-123")
    );
}

#[test]
fn extract_producer_conversation_id_rejects_blank_contract() {
    let summary = json!({
        "conversation_id": "   "
    });

    assert_eq!(extract_producer_conversation_id(&summary), None);
}

#[test]
fn require_producer_conversation_id_reads_valid_contract() {
    let summary = json!({
        "conversation_id": " conv-123 "
    });

    let conversation_id =
        require_producer_conversation_id(&summary, "producer_compatible").expect("id");

    assert_eq!(conversation_id, "conv-123");
}

#[test]
fn require_producer_conversation_id_preserves_missing_contract() {
    let summary = json!({});

    let error = require_producer_conversation_id(&summary, "producer_compatible")
        .expect_err("missing conversation id should fail");

    assert_eq!(
        error.code.as_deref(),
        Some("producer_http_video_missing_conversation_id")
    );
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
}

#[test]
fn producer_summary_has_video_proposal_detects_tool_call_or_return_contract() {
    let summary = json!({
        "tool_calls": [
            { "tool_name": "video__propose_music_video" }
        ],
        "tool_returns": []
    });

    assert!(producer_summary_has_video_proposal(&summary));

    let summary = json!({
        "tool_calls": [],
        "tool_returns": [
            { "tool_name": "video__propose_music_video" }
        ]
    });

    assert!(producer_summary_has_video_proposal(&summary));
}

#[test]
fn ensure_producer_video_proposal_seen_accepts_detected_contract() {
    let summary = json!({
        "tool_calls": [
            { "tool_name": "video__propose_music_video" }
        ]
    });

    ensure_producer_video_proposal_seen(&summary, "producer_compatible")
        .expect("proposal should be accepted");
}

#[test]
fn ensure_producer_video_proposal_seen_preserves_missing_contract() {
    let summary = json!({
        "tool_calls": [],
        "tool_returns": []
    });

    let error = ensure_producer_video_proposal_seen(&summary, "producer_compatible")
        .expect_err("missing proposal should fail");

    assert_eq!(
        error.code.as_deref(),
        Some("producer_http_video_missing_video_proposal")
    );
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
}

#[test]
fn extract_producer_video_create_job_id_reads_canonical_tool_return_contract() {
    let summary = json!({
        "tool_returns": [
            {
                "tool_name": "video__create_music_video",
                "content": { "job_id": "job-123" }
            }
        ]
    });

    assert_eq!(
        extract_producer_video_create_job_id(&summary).as_deref(),
        Some("job-123")
    );
}

#[test]
fn require_producer_video_create_job_id_reads_valid_contract() {
    let job_id =
        require_producer_video_create_job_id(Some("job-123".to_string()), "producer_compatible")
            .expect("job id should be accepted");

    assert_eq!(job_id, "job-123");
}

#[test]
fn require_producer_video_create_job_id_preserves_missing_contract() {
    let error = require_producer_video_create_job_id(None, "producer_compatible")
        .expect_err("missing job id should fail");

    assert_eq!(
        error.code.as_deref(),
        Some("producer_http_video_missing_video_job_id")
    );
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
}

#[test]
fn build_producer_video_bootstrap_plan_builds_default_context_contract() {
    let plan = build_producer_video_bootstrap_plan(
        "https://www.flowmusic.app",
        &json!({
            "clip_id": "clip-123",
            "project_id": "proj-9"
        }),
    )
    .expect("bootstrap plan");

    assert_eq!(plan.clip_id, "clip-123");
    assert_eq!(
        plan.bootstrap_prompt,
        "Let's make a music video with the song https://www.flowmusic.app/song/clip-123"
    );
    assert_eq!(plan.bootstrap_referer, "https://www.flowmusic.app/");
    assert_eq!(plan.client_context["current_song_id"], "clip-123");
    assert_eq!(plan.client_context["song_queue"][0]["id"], "clip-123");
    assert_eq!(plan.client_context["project_id"], "proj-9");
    assert_eq!(
        plan.client_context["selected_model"],
        crate::protocol::producer::PRODUCER_DEFAULT_MODEL
    );
}

#[test]
fn build_producer_video_bootstrap_plan_preserves_existing_context_contract() {
    let plan = build_producer_video_bootstrap_plan(
        "https://www.flowmusic.app/",
        &json!({
            "songId": "clip-xyz",
            "client_context": {
                "current_song_id": "custom-song",
                "selected_model": "custom-model"
            }
        }),
    )
    .expect("bootstrap plan");

    assert_eq!(plan.clip_id, "clip-xyz");
    assert_eq!(
        plan.bootstrap_prompt,
        "Let's make a music video with the song https://www.flowmusic.app/song/clip-xyz"
    );
    assert_eq!(plan.bootstrap_referer, "https://www.flowmusic.app/");
    assert_eq!(plan.client_context["current_song_id"], "custom-song");
    assert_eq!(plan.client_context["selected_model"], "custom-model");
}

#[test]
fn resolve_producer_video_session_plan_reads_valid_contract() {
    let plan = resolve_producer_video_session_plan(
        "https://www.flowmusic.app",
        &json!({
            "conversation_id": " conv-123 "
        }),
        "producer_compatible",
    )
    .expect("session plan");

    assert_eq!(plan.conversation_id, "conv-123");
    assert_eq!(
        plan.session_referer,
        "https://www.flowmusic.app/session/conv-123"
    );
}

#[test]
fn resolve_producer_video_session_plan_preserves_missing_contract() {
    let error = resolve_producer_video_session_plan(
        "https://www.flowmusic.app",
        &json!({}),
        "producer_compatible",
    )
    .expect_err("missing conversation id should fail");

    assert_eq!(
        error.code.as_deref(),
        Some("producer_http_video_missing_conversation_id")
    );
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
}
