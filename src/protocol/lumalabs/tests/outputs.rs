use super::*;

#[test]
fn extracts_output_id_from_action_response() {
    let output_id = extract_action_output_id(&json!({
        "output_artifacts": {
            "image": ["0k57jd93"]
        }
    }))
    .unwrap();
    assert_eq!(output_id, "0k57jd93");
}

#[test]
fn extracts_signed_url_from_nested_event_json() {
    let payload = json!({
        "artifact": {
            "signedUrl": "https://cdn.prod01.labs.lumalabs.ai/realm/cdn-cgi/0k57jd93.png?Policy=test&Signature=sig&Key-Pair-Id=kp"
        }
    })
    .to_string();
    let url = extract_signed_url_from_event_payload("0k57jd93", &payload).unwrap();
    assert!(url.contains("0k57jd93.png"));
    assert!(url.contains("Policy=test"));
}

#[test]
fn extracts_signed_url_from_escaped_text_payload() {
    let payload = r#"data: {\"url\":\"https:\/\/cdn.prod01.labs.lumalabs.ai\/realm\/cdn-cgi\/0k57jd93.png?Policy=test\\u0026Signature=sig\\u0026Key-Pair-Id=kp\"}"#;
    let url = extract_signed_url_from_event_payload("0k57jd93", payload).unwrap();
    assert!(url.contains("Signature=sig"));
    assert!(url.contains("Key-Pair-Id=kp"));
}

#[test]
fn extracts_video_output_id_from_configured_artifact_field() {
    let output_id = extract_action_output_id_for_field(
        &json!({
            "output_artifacts": {
                "video": ["ray-video-123"]
            }
        }),
        "video",
    )
    .unwrap();
    assert_eq!(output_id, "ray-video-123");
}

#[test]
fn video_generation_response_uses_video_object_shape() {
    let response = build_video_generation_response(
        "ray-2",
        "cinematic skyline at dusk",
        "https://cdn.prod01.labs.lumalabs.ai/output/clip.mp4",
    );
    assert_eq!(response["object"], "video.generation");
    assert_eq!(response["data"][0]["kind"], "video");
    assert_eq!(response["data"][0]["mime_type"], "video/mp4");
}

#[test]
fn audio_generation_response_uses_audio_object_shape() {
    let response = build_audio_generation_response(
        "music-v1",
        "warm lo-fi rain ambience",
        "https://cdn.prod01.labs.lumalabs.ai/output/track.mp3",
    );
    assert_eq!(response["object"], "audio.generation");
    assert_eq!(response["data"][0]["kind"], "audio");
    assert_eq!(response["data"][0]["mime_type"], "audio/mpeg");
}
