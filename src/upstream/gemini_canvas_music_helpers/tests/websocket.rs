use super::super::*;
use serde_json::json;

#[test]
fn parse_gemini_canvas_music_ws_setup_text_reports_invalid_json_contract() {
    let error = parse_gemini_canvas_music_ws_setup_text("{not-json")
        .expect_err("invalid setup JSON should fail");
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_music_ws_invalid_json")
    );
    assert!(error
        .message
        .starts_with("Gemini Canvas music websocket returned invalid JSON during setup:"));
}

#[test]
fn parse_gemini_canvas_music_ws_setup_binary_reports_invalid_json_contract() {
    let error = parse_gemini_canvas_music_ws_setup_binary(b"{not-json")
        .expect_err("invalid setup binary JSON should fail");
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_music_ws_invalid_json")
    );
    assert!(error
        .message
        .starts_with("Gemini Canvas music websocket returned invalid binary JSON during setup:"));
}

#[test]
fn parse_gemini_canvas_music_ws_text_reports_invalid_json_contract() {
    let error = parse_gemini_canvas_music_ws_text("{not-json")
        .expect_err("invalid runtime JSON should fail");
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_music_ws_invalid_json")
    );
    assert!(error
        .message
        .starts_with("Gemini Canvas music websocket returned invalid JSON:"));
}

#[test]
fn parse_gemini_canvas_music_ws_binary_reports_invalid_json_contract() {
    let error = parse_gemini_canvas_music_ws_binary(b"{not-json")
        .expect_err("invalid runtime binary JSON should fail");
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_music_ws_invalid_json")
    );
    assert!(error
        .message
        .starts_with("Gemini Canvas music websocket returned invalid binary JSON:"));
}

#[test]
fn gemini_canvas_music_ws_pong_failed_error_matches_contract() {
    let error = gemini_canvas_music_ws_pong_failed_error("broken pipe");
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_music_ws_send_failed")
    );
    assert_eq!(
        error.message.as_str(),
        "failed to reply to Gemini Canvas music websocket ping: broken pipe"
    );
}

#[test]
fn gemini_canvas_music_ws_send_frame_error_matches_contract() {
    let error = gemini_canvas_music_ws_send_frame_error("music client_content", "broken pipe");
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_music_ws_send_failed")
    );
    assert_eq!(
        error.message.as_str(),
        "failed to send Gemini Canvas music client_content frame: broken pipe"
    );
}

#[test]
fn gemini_canvas_music_ws_request_failed_error_matches_contract() {
    let error = gemini_canvas_music_ws_request_failed_error(
        "build Gemini Canvas music websocket request",
        "bad header",
    );
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_music_ws_request_failed")
    );
    assert_eq!(
        error.message.as_str(),
        "failed to build Gemini Canvas music websocket request: bad header"
    );
}

#[test]
fn gemini_canvas_music_ws_connect_failed_error_matches_contract() {
    let error = gemini_canvas_music_ws_connect_failed_error("connection reset");
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_music_ws_connect_failed")
    );
    assert_eq!(
        error.message.as_str(),
        "failed to connect to Gemini Canvas music websocket: connection reset"
    );
}

#[test]
fn gemini_canvas_music_ws_setup_transport_error_matches_contract() {
    let error = gemini_canvas_music_ws_setup_transport_error("closed");
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_music_ws_transport_failed")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas music websocket setup failed: closed"
    );
}

#[test]
fn gemini_canvas_music_ws_stream_transport_error_matches_contract() {
    let error = gemini_canvas_music_ws_stream_transport_error("reset");
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_music_ws_transport_failed")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas music websocket failed while streaming audio: reset"
    );
}

#[test]
fn gemini_canvas_music_ws_setup_timeout_error_matches_contract() {
    let error = gemini_canvas_music_ws_setup_timeout_error(Some("close code=1000".to_string()));
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_music_ws_setup_timeout")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas music websocket did not acknowledge setup.; last_setup_event=close code=1000"
    );
}

#[test]
fn gemini_canvas_music_ws_authenticated_setup_error_matches_contract() {
    let error = gemini_canvas_music_ws_authenticated_setup_error();
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_music_ws_setup_timeout")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas music websocket could not establish an authenticated setup."
    );
}

#[test]
fn gemini_canvas_music_ws_missing_audio_error_matches_contract() {
    let error = gemini_canvas_music_ws_missing_audio_error();
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_music_missing_audio")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas music websocket completed without audio chunks."
    );
}

#[test]
fn build_gemini_canvas_music_ws_setup_frame_preserves_model_contract() {
    let frame = build_gemini_canvas_music_ws_setup_frame("lyria-realtime-exp");
    let parsed: Value = serde_json::from_str(&frame).expect("setup frame JSON");
    assert_eq!(parsed["setup"]["model"], "models/lyria-realtime-exp");
}

#[test]
fn build_gemini_canvas_music_ws_client_content_frame_wraps_prompt_contract() {
    let frame = build_gemini_canvas_music_ws_client_content_frame("write a synth cue");
    let parsed: Value = serde_json::from_str(&frame).expect("client content frame JSON");
    assert_eq!(
        parsed["client_content"],
        gemini_canvas::build_music_client_content("write a synth cue")
    );
}

#[test]
fn build_gemini_canvas_music_ws_generation_config_frame_wraps_config_contract() {
    let config = json!({
        "durationSeconds": 30,
        "temperature": 0.8
    });
    let frame = build_gemini_canvas_music_ws_generation_config_frame(&config);
    let parsed: Value = serde_json::from_str(&frame).expect("generation config frame JSON");
    assert_eq!(parsed["music_generation_config"], config);
}

#[test]
fn build_gemini_canvas_music_ws_playback_control_frame_preserves_play_contract() {
    let frame = build_gemini_canvas_music_ws_playback_control_frame();
    let parsed: Value = serde_json::from_str(&frame).expect("playback control frame JSON");
    assert_eq!(parsed["playback_control"], "PLAY");
}
