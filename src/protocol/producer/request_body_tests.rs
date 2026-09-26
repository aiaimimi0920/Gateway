use serde_json::{json, Value};

use super::{
    build_conversation_request_body, build_image_generation_request, build_send_message_request,
    build_video_tool_call_request, normalize_image_generations, normalize_music_generations,
    normalize_video_generations, PRODUCER_DEFAULT_MODEL, PRODUCER_IMAGE_DEFAULT_MODEL,
    PRODUCER_VIDEO_DEFAULT_MODEL,
};
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind, ProtocolFamily};

fn request(raw_body: Value, endpoint_kind: EndpointKind) -> CanonicalRelayRequest {
    CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind,
        requested_model: None,
        stream: false,
        messages: Vec::new(),
        tools: Vec::new(),
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body,
        previous_response_id: None,
        explicit_session_key: None,
        extra: std::collections::HashMap::new(),
    }
}

#[test]
fn send_message_request_preserves_exact_default_wire_shape() {
    let req = request(
        json!({
            "model": "caller-model",
            "stream": false,
            "prompt": "  synthwave sunrise  ",
            "input": "ignored input",
            "lyrics": "ignored lyrics",
            "user": "caller-user",
            "project_id": "project-7",
            "custom": { "trace": true }
        }),
        EndpointKind::MusicGenerations,
    );

    let body = build_send_message_request(&req, PRODUCER_DEFAULT_MODEL).unwrap();

    assert_eq!(
        body["parts"],
        json!([{
            "part_kind": "user-prompt",
            "content": "synthwave sunrise"
        }])
    );
    assert_eq!(
        body["client_context"],
        json!({
            "current_song_id": null,
            "song_queue": [],
            "project_id": "project-7",
            "selected_model": PRODUCER_DEFAULT_MODEL,
            "lyrics_id_map": {},
            "ghostwriter_version": "standard"
        })
    );
    assert_eq!(body["model_name"], PRODUCER_DEFAULT_MODEL);
    assert_eq!(body["mode"], "standard");
    assert_eq!(body["project_id"], "project-7");
    assert_eq!(body["custom"], json!({ "trace": true }));
    for removed in ["model", "stream", "prompt", "input", "lyrics", "user"] {
        assert!(body.get(removed).is_none(), "unexpected field: {removed}");
    }
}

#[test]
fn send_message_request_preserves_explicit_wire_fields() {
    let req = request(
        json!({
            "parts": [{ "part_kind": "user-prompt", "content": "kept" }],
            "client_context": { "selected_model": "explicit-context" },
            "model_name": "explicit-model",
            "mode": "explicit-mode"
        }),
        EndpointKind::MusicGenerations,
    );

    let body = build_send_message_request(&req, PRODUCER_DEFAULT_MODEL).unwrap();

    assert_eq!(body["parts"][0]["content"], "kept");
    assert_eq!(body["client_context"]["selected_model"], "explicit-context");
    assert_eq!(body["model_name"], "explicit-model");
    assert_eq!(body["mode"], "explicit-mode");
}

#[test]
fn image_request_preserves_exact_native_wire_shape() {
    let req = request(
        json!({
            "model": "caller-model",
            "stream": false,
            "response_format": "url",
            "n": 1,
            "user": "caller-user",
            "input": "  chrome skyline  ",
            "imageType": "playlist",
            "custom": 42
        }),
        EndpointKind::ImagesGenerations,
    );

    let body = build_image_generation_request(&req, PRODUCER_IMAGE_DEFAULT_MODEL).unwrap();

    assert_eq!(body["prompt"], "chrome skyline");
    assert_eq!(body["type"], "playlist");
    assert_eq!(body["model_name"], PRODUCER_IMAGE_DEFAULT_MODEL);
    assert_eq!(body["custom"], 42);
    for removed in [
        "model",
        "stream",
        "response_format",
        "n",
        "user",
        "image_type",
        "imageType",
        "input",
    ] {
        assert!(body.get(removed).is_none(), "unexpected field: {removed}");
    }
}

#[test]
fn video_tool_call_request_preserves_exact_default_wire_shape() {
    let req = request(
        json!({
            "songId": "clip-42",
            "input": "  violet stage lights  ",
            "project_id": "project-9"
        }),
        EndpointKind::VideosGenerations,
    );

    let body = build_video_tool_call_request(&req, PRODUCER_VIDEO_DEFAULT_MODEL).unwrap();

    assert_eq!(
        body,
        json!({
            "part": {
                "tool_name": "video__create_music_video",
                "args": {
                    "clip_id": "clip-42",
                    "user_message": "violet stage lights",
                    "aspect_ratio": "9:16",
                    "resolution": "720p",
                    "render_lyrics": true
                }
            },
            "client_context": {
                "current_song_id": "clip-42",
                "song_queue": ["clip-42"],
                "project_id": "project-9",
                "selected_model": PRODUCER_VIDEO_DEFAULT_MODEL,
                "lyrics_id_map": {},
                "ghostwriter_version": "standard"
            }
        })
    );
}

#[test]
fn request_builders_preserve_failure_contracts() {
    let non_object = request(Value::Null, EndpointKind::MusicGenerations);
    let error = build_send_message_request(&non_object, PRODUCER_DEFAULT_MODEL).unwrap_err();
    assert_eq!(
        error.message,
        "Producer.ai request body must be a JSON object"
    );

    let missing_music = request(json!({}), EndpointKind::MusicGenerations);
    let error = build_send_message_request(&missing_music, PRODUCER_DEFAULT_MODEL).unwrap_err();
    assert_eq!(error.code.as_deref(), Some("missing_music_prompt"));

    let missing_image = request(json!({}), EndpointKind::ImagesGenerations);
    let error =
        build_image_generation_request(&missing_image, PRODUCER_IMAGE_DEFAULT_MODEL).unwrap_err();
    assert_eq!(error.code.as_deref(), Some("missing_image_prompt"));

    let missing_video = request(json!({}), EndpointKind::VideosGenerations);
    let error =
        build_video_tool_call_request(&missing_video, PRODUCER_VIDEO_DEFAULT_MODEL).unwrap_err();
    assert_eq!(error.code.as_deref(), Some("missing_video_clip_id"));
}

#[test]
fn build_image_generation_request_uses_site_native_type_field() {
    let req = normalize_image_generations(json!({
        "model": "producer:image",
        "prompt": "holographic album cover",
        "imageType": "clip",
        "response_format": "url"
    }))
    .unwrap();

    let body = build_image_generation_request(&req, PRODUCER_IMAGE_DEFAULT_MODEL).unwrap();
    assert_eq!(body["prompt"], "holographic album cover");
    assert_eq!(body["type"], "clip");
    assert_eq!(body["model_name"], PRODUCER_IMAGE_DEFAULT_MODEL);
    assert!(body.get("response_format").is_none());
}

#[test]
fn build_send_message_request_uses_prompt_when_parts_missing() {
    let req = normalize_music_generations(json!({
        "prompt": "lo-fi piano with rain ambience"
    }))
    .unwrap();

    let body = build_send_message_request(&req, PRODUCER_DEFAULT_MODEL).unwrap();
    assert_eq!(body["model_name"], PRODUCER_DEFAULT_MODEL);
    assert_eq!(body["mode"], "standard");
    assert_eq!(body["parts"][0]["part_kind"], "user-prompt");
    assert_eq!(
        body["parts"][0]["content"],
        "lo-fi piano with rain ambience"
    );
}

#[test]
fn build_send_message_request_preserves_explicit_parts() {
    let req = normalize_music_generations(json!({
        "model": "producer:standard",
        "parts": [
            {
                "part_kind": "user-prompt",
                "content": "cinematic trailer drums"
            }
        ]
    }))
    .unwrap();

    let body = build_send_message_request(&req, PRODUCER_DEFAULT_MODEL).unwrap();
    assert_eq!(body["parts"][0]["content"], "cinematic trailer drums");
}

#[test]
fn build_conversation_request_body_uses_prompt_and_standard_mode() {
    let body = build_conversation_request_body(
        "cinematic trailer drums",
        None,
        &json!({ "project_id": "proj-1" }),
        PRODUCER_DEFAULT_MODEL,
    );
    assert_eq!(body["parts"][0]["part_kind"], "user-prompt");
    assert_eq!(body["parts"][0]["content"], "cinematic trailer drums");
    assert_eq!(body["client_context"]["project_id"], "proj-1");
    assert_eq!(body["model_name"], PRODUCER_DEFAULT_MODEL);
    assert_eq!(body["mode"], "standard");
    assert!(body.get("conversation_id").is_none());
}

#[test]
fn build_conversation_request_body_preserves_conversation_id_when_present() {
    let body = build_conversation_request_body(
        "follow-up idea",
        Some("conv-123"),
        &json!({ "project_id": "proj-2" }),
        PRODUCER_DEFAULT_MODEL,
    );
    assert_eq!(body["conversation_id"], "conv-123");
}

#[test]
fn build_video_tool_call_request_uses_real_web_field_names() {
    let req = normalize_video_generations(json!({
        "clip_id": "clip-9",
        "prompt": "dreamy stage lights",
        "subjectImageUrl": "https://example.com/subject.png",
        "style_image_url": "https://example.com/style.png",
        "displayLyrics": false,
        "durationSeconds": 42,
        "conversation_id": "conv-video-1"
    }))
    .unwrap();

    let body = build_video_tool_call_request(&req, PRODUCER_VIDEO_DEFAULT_MODEL).unwrap();
    assert_eq!(body["conversation_id"], "conv-video-1");
    assert_eq!(body["part"]["tool_name"], "video__create_music_video");
    assert_eq!(body["part"]["args"]["clip_id"], "clip-9");
    assert_eq!(body["part"]["args"]["user_message"], "dreamy stage lights");
    assert_eq!(
        body["part"]["args"]["likeness_image_url"],
        "https://example.com/subject.png"
    );
    assert_eq!(
        body["part"]["args"]["style_image_url"],
        "https://example.com/style.png"
    );
    assert_eq!(body["part"]["args"]["render_lyrics"], false);
    assert_eq!(body["part"]["args"]["duration_s"], 42);
    assert_eq!(body["part"]["args"]["aspect_ratio"], "9:16");
    assert_eq!(body["part"]["args"]["resolution"], "720p");
    assert_eq!(body["client_context"]["current_song_id"], "clip-9");
}
