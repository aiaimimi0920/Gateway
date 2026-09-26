use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{
    build_video_generation_response, build_video_proposal_prompt, choose_video_confirm_prompt,
    PRODUCER_VIDEO_DEFAULT_MODEL,
};

fn video_response(
    request_body: &Value,
    final_url: Option<&str>,
    final_status: &str,
    status_payload: &Value,
) -> Value {
    build_video_generation_response(
        "https://www.producer.ai",
        request_body,
        PRODUCER_VIDEO_DEFAULT_MODEL,
        "clip-test",
        "conv-test",
        "bootstrap-test",
        "creative-test",
        Some("confirm-test"),
        "video-test",
        final_url,
        final_status,
        status_payload,
        &json!({ "creative": true }),
        Some(&json!({ "confirmation": true })),
    )
}

#[test]
fn build_video_proposal_prompt_matches_http_orchestration_shape() {
    let prompt = build_video_proposal_prompt(&json!({
        "clip_id": "clip-9",
        "prompt": "chrome silhouettes in neon rain",
        "aspect_ratio": "9:16",
        "duration_s": 60,
        "render_lyrics": false,
    }))
    .unwrap();
    assert!(prompt.contains("Please propose the music video."));
    assert!(prompt.contains("Vision: chrome silhouettes in neon rain"));
    assert!(prompt.contains("Use 9:16."));
    assert!(prompt.contains("Lyrics on screen: no."));
    assert!(prompt.contains("Duration: use about 60 seconds."));
}

#[test]
fn build_video_proposal_prompt_rejects_invalid_inputs() {
    let non_object =
        build_video_proposal_prompt(&json!([])).expect_err("non-object video requests must fail");
    assert_eq!(non_object.http_status, Some(400));
    assert_eq!(non_object.provider_name, None);
    assert_eq!(non_object.code, None);
    assert_eq!(
        non_object.message,
        "Producer.ai video request body must be a JSON object"
    );

    let missing_prompt = build_video_proposal_prompt(&json!({ "clip_id": "clip-9" }))
        .expect_err("video proposal requests require a prompt");
    assert_eq!(missing_prompt.http_status, Some(400));
    assert_eq!(missing_prompt.provider_name, None);
    assert_eq!(missing_prompt.code.as_deref(), Some("missing_video_prompt"));
    assert_eq!(
        missing_prompt.message,
        "Producer.ai music video requests require one of: prompt, input, user_message, or args.user_message."
    );
}

#[test]
fn build_video_proposal_prompt_preserves_alias_precedence() {
    let prompt = build_video_proposal_prompt(&json!({
        "prompt": " primary vision ",
        "input": "secondary vision",
        "aspect_ratio": "4:3",
        "aspectRatio": "9:16",
        "size": "1:1",
        "style_image_url": " https://cdn.example.com/style-primary.png ",
        "styleImageUrl": "https://cdn.example.com/style-secondary.png",
        "likeness_image_url": " https://cdn.example.com/subject-primary.png ",
        "subjectImageUrl": "https://cdn.example.com/subject-secondary.png",
        "duration_s": "12.5",
        "durationSeconds": 30,
        "render_lyrics": false,
        "displayLyrics": true,
    }))
    .expect("video proposal prompt");

    assert_eq!(
        prompt,
        "Please propose the music video. Vision: primary vision Use 4:3. Style reference image: https://cdn.example.com/style-primary.png. Subject image: https://cdn.example.com/subject-primary.png. Lyrics on screen: no. Duration: use about 12.5 seconds."
    );
}

#[test]
fn choose_video_confirm_prompt_prefers_proposed_inputs() {
    let prompt = choose_video_confirm_prompt(
        &json!({
            "clip_id": "clip-9",
            "aspect_ratio": "9:16",
            "duration_s": 60,
            "resolution": "720p",
        }),
        &json!({
            "tool_calls": [{
                "tool_name": "video__propose_music_video",
                "args": {
                    "inputs": {
                        "start_s": 0,
                        "duration_s": 60,
                        "aspect_ratio": "9:16",
                        "resolution": "720p"
                    }
                }
            }]
        }),
    )
    .unwrap();
    assert!(prompt.contains("Create this exact proposed music video now."));
    assert!(prompt.contains("Keep the current start time at 0s."));
    assert!(prompt.contains("Keep the duration at 60s."));
    assert!(prompt.contains("Keep the aspect ratio at 9:16."));
    assert!(prompt.contains("Keep the resolution at 720p."));
}

#[test]
fn choose_video_confirm_prompt_preserves_explicit_alias_precedence() {
    let explicit = choose_video_confirm_prompt(
        &json!({
            "confirm_prompt": "  primary confirmation  ",
            "confirmPrompt": "secondary confirmation",
        }),
        &json!({ "suggestions": ["Create the video"] }),
    )
    .expect("explicit confirmation prompt");
    assert_eq!(explicit, "primary confirmation");

    let non_object = choose_video_confirm_prompt(&json!([]), &json!({}))
        .expect_err("non-object video requests must fail");
    assert_eq!(non_object.http_status, Some(400));
    assert_eq!(non_object.code, None);
    assert_eq!(
        non_object.message,
        "Producer.ai video request body must be a JSON object"
    );
}

#[test]
fn choose_video_confirm_prompt_preserves_proposal_and_fallback_priority() {
    let proposed = choose_video_confirm_prompt(
        &json!({
            "start_s": 99,
            "duration_s": 88,
            "aspect_ratio": "1:1",
            "resolution": "360p",
        }),
        &json!({
            "tool_calls": [
                null,
                { "tool_name": "other", "args": { "inputs": { "start_s": 70 } } },
                {
                    "tool_name": "video__propose_music_video",
                    "args": { "inputs": {
                        "startSeconds": "1.5",
                        "durationSeconds": 12,
                        "size": "21:9",
                        "resolution": "4k"
                    } }
                },
                {
                    "tool_name": "video__propose_music_video",
                    "args": { "inputs": { "start_s": 55 } }
                }
            ]
        }),
    )
    .expect("proposed confirmation prompt");
    assert_eq!(
        proposed,
        "Create this exact proposed music video now. Keep the current start time at 1.5s. Keep the duration at 12s. Keep the aspect ratio at 21:9. Keep the resolution at 4k. Do not ask follow-up questions or change the selected song section."
    );

    let preferred = choose_video_confirm_prompt(
        &json!({}),
        &json!({
            "suggestions": ["Render a preview", "Create the video now"],
            "message_texts": ["Start when ready"]
        }),
    )
    .expect("preferred suggestion");
    assert_eq!(preferred, "Create the video now");
    assert_eq!(
        choose_video_confirm_prompt(&json!({}), &json!({})).unwrap(),
        "Create the video"
    );
}

#[test]
fn build_video_generation_response_preserves_completed_contract() {
    let body = build_video_generation_response(
        "https://www.producer.ai",
        &json!({
            "prompt": "launch trailer",
            "aspect_ratio": "16:9",
            "resolution": "1080p",
            "duration_s": 12
        }),
        PRODUCER_VIDEO_DEFAULT_MODEL,
        "clip-1",
        "conv-video-1",
        "bootstrap-job-1",
        "creative-job-1",
        Some("confirm-job-1"),
        "video-job-1",
        Some("https://cdn.example.com/music-video/video-job-1/final.mp4"),
        "completed",
        &json!({
            "status": "completed",
            "preview": { "video": "https://cdn.example.com/preview.mp4" }
        }),
        &json!({ "tool_calls": [] }),
        Some(&json!({ "tool_returns": [] })),
    );

    assert_eq!(body["object"], "video.generation");
    assert_eq!(body["accepted"], false);
    assert_eq!(body["completed"], true);
    assert_eq!(body["model"], PRODUCER_VIDEO_DEFAULT_MODEL);
    assert_eq!(
        body["prompt"],
        "Please propose the music video. Vision: launch trailer Use 16:9. Style reference image: none. Subject image: none, generate one. Lyrics on screen: no. Duration: use about 12 seconds."
    );
    assert_eq!(body["clip_id"], "clip-1");
    assert_eq!(body["conversation_id"], "conv-video-1");
    assert_eq!(body["bootstrap_job_id"], "bootstrap-job-1");
    assert_eq!(body["creative_job_id"], "creative-job-1");
    assert_eq!(body["confirmation_job_id"], "confirm-job-1");
    assert_eq!(body["job_id"], "video-job-1");
    assert_eq!(
        body["progress_url"],
        "https://www.producer.ai/session/conv-video-1"
    );
    assert_eq!(body["status_path"], "/__api/music-video/video-job-1/status");
    assert_eq!(body["detail_path"], "/__api/music-video/get/video-job-1");
    assert_eq!(body["library_path"], "/library/videos");
    assert_eq!(
        body["data"][0]["url"],
        "https://cdn.example.com/music-video/video-job-1/final.mp4"
    );
    assert_eq!(
        body["data"][0]["preview_url"],
        "https://cdn.example.com/preview.mp4"
    );
    assert_eq!(body["data"][0]["aspect_ratio"], "16:9");
    assert_eq!(body["data"][0]["resolution"], "1080p");
    assert_eq!(body["data"][0]["duration_seconds"], 12);
    assert_eq!(body["state"], "completed");
}

#[test]
fn build_video_generation_response_requires_completed_status_and_url() {
    let completed = video_response(
        &json!({ "prompt": "ready" }),
        Some("https://cdn.example.com/final.mp4"),
        "completed",
        &json!({}),
    );
    let missing_url = video_response(&json!({ "prompt": "ready" }), None, "completed", &json!({}));
    let processing_with_url = video_response(
        &json!({ "prompt": "ready" }),
        Some("https://cdn.example.com/not-final.mp4"),
        "processing",
        &json!({}),
    );

    assert_eq!(
        (
            completed["completed"].clone(),
            completed["accepted"].clone()
        ),
        (json!(true), json!(false))
    );
    assert_eq!(
        (
            missing_url["completed"].clone(),
            missing_url["accepted"].clone()
        ),
        (json!(false), json!(true))
    );
    assert_eq!(
        (
            processing_with_url["completed"].clone(),
            processing_with_url["accepted"].clone()
        ),
        (json!(false), json!(true))
    );
    assert_eq!(
        processing_with_url["data"][0]["url"],
        "https://cdn.example.com/not-final.mp4"
    );
}

#[test]
fn build_video_generation_response_preserves_pending_without_confirmation_contract() {
    let body = build_video_generation_response(
        "https://www.producer.ai",
        &json!({
            "resolution": "720p",
            "durationSeconds": 8
        }),
        PRODUCER_VIDEO_DEFAULT_MODEL,
        "clip-2",
        "conv-video-2",
        "bootstrap-job-2",
        "creative-job-2",
        None,
        "video-job-2",
        None,
        "processing",
        &json!({
            "state": { "status": "processing" }
        }),
        &json!({ "events": [] }),
        None,
    );

    assert_eq!(body["accepted"], true);
    assert_eq!(body["completed"], false);
    assert_eq!(body["confirmation_job_id"], serde_json::Value::Null);
    assert_eq!(body["data"][0]["url"], serde_json::Value::Null);
    assert_eq!(body["data"][0]["preview_url"], serde_json::Value::Null);
    assert_eq!(body["data"][0]["resolution"], "720p");
    assert_eq!(body["data"][0]["duration_seconds"], 8);
    assert_eq!(body["prompt"], PRODUCER_VIDEO_DEFAULT_MODEL);
    assert_eq!(body["state"], "processing");
    assert_eq!(body["confirmation_stream"], serde_json::Value::Null);
}

#[test]
fn build_video_generation_response_preserves_alias_and_stream_fields() {
    let status = json!({
        "preview_url": "https://cdn.example.com/ignored.mp4",
        "preview": { "video": "https://cdn.example.com/nested-preview.mp4" }
    });
    let response = video_response(
        &json!({
            "prompt": "alias contract",
            "aspect_ratio": "4:3",
            "aspectRatio": "9:16",
            "resolution": "1440p",
            "duration_s": 7,
            "durationSeconds": 8,
            "duration": 9,
        }),
        None,
        "accepted",
        &status,
    );

    assert_eq!(
        response["data"][0]["preview_url"],
        status["preview"]["video"]
    );
    assert_eq!(response["data"][0]["aspect_ratio"], "4:3");
    assert_eq!(response["data"][0]["resolution"], "1440p");
    assert_eq!(response["data"][0]["duration_seconds"], 7);
    assert_eq!(response["status"], status);
    assert_eq!(response["creative_stream"], json!({ "creative": true }));
    assert_eq!(
        response["confirmation_stream"],
        json!({ "confirmation": true })
    );
    assert!(response.get("upstream_response").is_none());
}

#[test]
fn build_video_generation_response_falls_back_for_malformed_request_and_sets_created() {
    let before = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_secs();
    let response = video_response(&json!([]), None, "processing", &json!([]));
    let after = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_secs();

    assert_eq!(response["prompt"], PRODUCER_VIDEO_DEFAULT_MODEL);
    assert_eq!(response["data"][0]["preview_url"], Value::Null);
    assert_eq!(response["data"][0]["aspect_ratio"], Value::Null);
    assert_eq!(response["data"][0]["resolution"], Value::Null);
    assert_eq!(response["data"][0]["duration_seconds"], Value::Null);
    let created = response["created"]
        .as_u64()
        .expect("created is a Unix timestamp");
    assert!((before..=after).contains(&created));
}
