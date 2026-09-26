use serde_json::{json, Value};

use super::{
    normalize_image_generations, normalize_music_generations, normalize_video_generations,
    should_use_image_generations, should_use_video_generations, PRODUCER_IMAGE_DEFAULT_MODEL,
    PRODUCER_VIDEO_DEFAULT_MODEL,
};
use crate::error::GatewayError;
use crate::protocol::canonical::EndpointKind;

type Normalizer =
    fn(Value) -> Result<crate::protocol::canonical::CanonicalRelayRequest, GatewayError>;

#[test]
fn media_normalizers_reject_non_object_bodies() {
    let cases: [(Normalizer, &str); 3] = [
        (
            normalize_image_generations,
            "Image generation request body must be a JSON object",
        ),
        (
            normalize_music_generations,
            "Music generation request body must be a JSON object",
        ),
        (
            normalize_video_generations,
            "Video generation request body must be a JSON object",
        ),
    ];

    for (normalize, message) in cases {
        let error = normalize(json!([])).expect_err("array bodies must be rejected");
        assert_eq!(error.http_status, Some(400));
        assert_eq!(error.message, message);
    }
}

#[test]
fn media_normalizers_reject_streaming_with_endpoint_codes() {
    let cases: [(Normalizer, Value, &str); 3] = [
        (
            normalize_image_generations,
            json!({ "prompt": "cover", "stream": true }),
            "image_streaming_not_supported",
        ),
        (
            normalize_music_generations,
            json!({ "prompt": "song", "stream": true }),
            "music_streaming_not_supported",
        ),
        (
            normalize_video_generations,
            json!({ "clip_id": "clip-1", "stream": true }),
            "video_streaming_not_supported",
        ),
    ];

    for (normalize, body, code) in cases {
        let error = normalize(body).expect_err("streaming must be rejected");
        assert_eq!(error.http_status, Some(400));
        assert_eq!(error.code.as_deref(), Some(code));
    }
}

#[test]
fn image_normalizer_enforces_prompt_format_and_count() {
    let missing =
        normalize_image_generations(json!({})).expect_err("image prompt must be required");
    assert_eq!(missing.code.as_deref(), Some("missing_image_prompt"));

    let format = normalize_image_generations(json!({
        "prompt": "cover",
        "responseFormat": "B64_JSON"
    }))
    .expect_err("only URL responses are supported");
    assert_eq!(
        format.code.as_deref(),
        Some("unsupported_producer_image_response_format")
    );

    let count = normalize_image_generations(json!({ "prompt": "cover", "n": "2" }))
        .expect_err("only one image is supported");
    assert_eq!(
        count.code.as_deref(),
        Some("unsupported_producer_image_count")
    );
}

#[test]
fn music_normalizer_preserves_raw_body_and_session_alias() {
    let body = json!({
        "parts": [" first ", { "content": ["second", " "] }],
        "thread_id": " thread-9 ",
        "custom": { "preserved": true }
    });
    let request = normalize_music_generations(body.clone()).expect("valid music request");

    assert_eq!(request.raw_body, body);
    assert_eq!(request.explicit_session_key.as_deref(), Some("thread-9"));
    assert_eq!(request.messages[0].text_content(), "first\nsecond");
    assert!(request.requested_model.is_none());
}

#[test]
fn image_normalizer_trims_model_and_defaults_type() {
    let request = normalize_image_generations(json!({
        "model": " producer:image-custom ",
        "input": " cover "
    }))
    .expect("valid image request");

    assert_eq!(
        request.requested_model.as_deref(),
        Some("producer:image-custom")
    );
    assert_eq!(request.messages[0].text_content(), "type: clip\ncover");

    let defaulted =
        normalize_image_generations(json!({ "prompt": "cover" })).expect("default image model");
    assert_eq!(
        defaulted.requested_model.as_deref(),
        Some(PRODUCER_IMAGE_DEFAULT_MODEL)
    );
}

#[test]
fn video_normalizer_accepts_nested_aliases_and_defaults_model() {
    let request = normalize_video_generations(json!({
        "args": {
            "songId": " song-7 ",
            "userMessage": " neon stage "
        },
        "session_id": "session-7"
    }))
    .expect("nested Producer video aliases should normalize");

    assert_eq!(
        request.requested_model.as_deref(),
        Some(PRODUCER_VIDEO_DEFAULT_MODEL)
    );
    assert_eq!(request.explicit_session_key.as_deref(), Some("session-7"));
    assert_eq!(
        request.messages[0].text_content(),
        "clip_id: song-7\nneon stage"
    );
}

#[test]
fn video_normalizer_requires_clip_identifier() {
    let error = normalize_video_generations(json!({ "prompt": "neon stage" }))
        .expect_err("video clip identifier must be required");
    assert_eq!(error.code.as_deref(), Some("missing_video_clip_id"));
}

#[test]
fn producer_router_honors_explicit_model_before_fallback_fields() {
    assert!(should_use_image_generations(&json!({
        "model": "producer-image-v2"
    })));
    assert!(should_use_image_generations(&json!({ "type": "CLIP" })));
    assert!(!should_use_image_generations(&json!({
        "model": "gemini-image",
        "type": "clip"
    })));

    assert!(should_use_video_generations(&json!({
        "args": { "durationSeconds": 12 }
    })));
    assert!(!should_use_video_generations(&json!({
        "model": "gemini-video",
        "clip_id": "clip-1"
    })));
    assert!(!should_use_video_generations(&json!([])));
}

#[test]
fn normalize_image_generations_defaults_clip_type_and_model() {
    let req = normalize_image_generations(json!({
        "prompt": "retro chrome mascot"
    }))
    .unwrap();

    assert_eq!(req.endpoint_kind, EndpointKind::ImagesGenerations);
    assert_eq!(
        req.requested_model.as_deref(),
        Some(PRODUCER_IMAGE_DEFAULT_MODEL)
    );
    assert_eq!(
        req.messages[0].text_content(),
        "type: clip\nretro chrome mascot"
    );
}

#[test]
fn normalize_image_generations_rejects_b64_response_format() {
    let err = normalize_image_generations(json!({
        "prompt": "retro chrome mascot",
        "response_format": "b64_json"
    }))
    .expect_err("unsupported Producer image formats should fail");

    assert_eq!(err.http_status, Some(400));
    assert_eq!(
        err.code.as_deref(),
        Some("unsupported_producer_image_response_format")
    );
}

#[test]
fn gemini_business_image_model_is_not_claimed_by_producer_router() {
    assert!(!should_use_image_generations(&json!({
        "model": "gemini-2.5-flash-image-preview",
        "prompt": "launch poster"
    })));
}

#[test]
fn normalize_music_generations_leaves_model_unspecified_when_absent() {
    let req = normalize_music_generations(json!({
        "prompt": "dreamy synthpop with airy vocals"
    }))
    .unwrap();

    assert_eq!(req.endpoint_kind, EndpointKind::MusicGenerations);
    assert!(req.requested_model.is_none());
    assert_eq!(
        req.messages[0].text_content(),
        "dreamy synthpop with airy vocals"
    );
}

#[test]
fn normalize_video_generations_defaults_model_and_song_alias() {
    let req = normalize_video_generations(json!({
        "songId": "song-123",
        "user_message": "anime cyberpunk performance video"
    }))
    .unwrap();

    assert_eq!(req.endpoint_kind, EndpointKind::VideosGenerations);
    assert_eq!(
        req.requested_model.as_deref(),
        Some(PRODUCER_VIDEO_DEFAULT_MODEL)
    );
    assert_eq!(
        req.messages[0].text_content(),
        "clip_id: song-123\nanime cyberpunk performance video"
    );
}

#[test]
fn gemini_canvas_video_model_is_not_claimed_by_producer_router() {
    assert!(!should_use_video_generations(&json!({
        "model": "gemini-canvas-video-preview",
        "prompt": "launch video"
    })));
}
