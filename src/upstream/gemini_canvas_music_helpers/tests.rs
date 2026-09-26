use super::*;
use serde_json::json;

mod websocket;

fn make_music_asset(
    kind: &str,
    url: &str,
    mime_type: &str,
) -> gemini_canvas::GeminiCanvasMediaAsset {
    gemini_canvas::GeminiCanvasMediaAsset {
        kind: kind.to_string(),
        url: url.to_string(),
        mime_type: mime_type.to_string(),
        download_token: None,
        body_base64: None,
        alt: None,
        width: None,
        height: None,
        duration_seconds: Some(30.0),
    }
}

#[test]
fn gemini_canvas_music_filtered_prompt_error_matches_contract() {
    let error = gemini_canvas_music_filtered_prompt_error(&Value::String("policy".to_string()));
    assert_eq!(error.http_status, Some(400));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_music_filtered_prompt")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas music prompt was filtered: \"policy\""
    );
}

#[test]
fn gemini_canvas_music_missing_asset_error_matches_contract() {
    let error = gemini_canvas_music_missing_asset_error("gemini_canvas_compatible");
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(error.code.as_deref(), Some("gemini_canvas_no_music_asset"));
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas music generation completed without a downloadable media asset."
    );
}

#[test]
fn gemini_canvas_music_missing_api_key_error_matches_contract() {
    let error = gemini_canvas_music_missing_api_key_error();
    assert_eq!(error.http_status, Some(401));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(error.code.as_deref(), Some("gemini_canvas_missing_api_key"));
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas official music API requires api_key."
    );
}

#[test]
fn gemini_canvas_program_music_no_key_contract_missing_error_matches_contract() {
    let error =
        gemini_canvas_program_music_no_key_contract_missing_error("gemini_canvas_compatible");
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_program_music_no_key_contract_missing")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas no-key music StreamGenerate did not expose a usable action contract or audio asset."
    );
}

#[test]
fn gemini_canvas_program_music_no_key_browserless_stage_incomplete_error_matches_contract() {
    let error = gemini_canvas_program_music_no_key_browserless_stage_incomplete_error(
        "gemini_canvas_compatible",
    );
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_program_music_no_key_stage_incomplete")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas no-key music StreamGenerate reached an app-owned contract, but the current browserless replay still lacks the required page-state to advance beyond BardErrorInfo [1060]."
    );
}

#[test]
fn gemini_canvas_program_music_no_key_prelude_stage_incomplete_error_matches_contract() {
    let error = gemini_canvas_program_music_no_key_prelude_stage_incomplete_error(
        "gemini_canvas_compatible",
    );
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_program_music_no_key_stage_incomplete")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas no-key music StreamGenerate reached an app-owned contract, but even the current-page minimal prelude still could not advance beyond BardErrorInfo [1060]."
    );
}

#[test]
fn gemini_canvas_program_music_no_key_post_1060_contract_missing_error_matches_contract() {
    let error = gemini_canvas_program_music_no_key_post_1060_contract_missing_error(
        "gemini_canvas_compatible",
    );
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_program_music_no_key_contract_missing")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas no-key music StreamGenerate advanced past the old 1060 gate, but still did not expose a usable action contract or audio asset."
    );
}

#[test]
fn gemini_canvas_program_music_page_poll_failed_error_matches_contract() {
    let error = gemini_canvas_program_music_page_poll_failed_error(
        "gemini_canvas_compatible",
        "https://example.com/app",
        "timeout",
    );
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_program_music_page_poll_failed")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas music follow-up still did not expose a usable music asset after PCck7e/page poll. bootstrap_page=https://example.com/app; upstream=timeout"
    );
}

#[test]
fn gemini_canvas_program_music_recent_page_poll_failed_error_matches_contract() {
    let error = gemini_canvas_program_music_recent_page_poll_failed_error(
        "gemini_canvas_compatible",
        "https://example.com/bootstrap",
        "https://example.com/recovered",
        "timeout",
    );
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_program_music_recent_page_poll_failed")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas music aPya6c follow-up advanced to accepted state, but recent-conversation page polling still did not expose a usable music asset. bootstrap_page=https://example.com/bootstrap; recovered_page=https://example.com/recovered; upstream=timeout"
    );
}

#[test]
fn gemini_canvas_program_music_invoke_target_missing_error_matches_contract() {
    let error = gemini_canvas_program_music_invoke_target_missing_error();
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_program_music_invoke_target_missing")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas program-owned music lane is missing an explicit app invoke target."
    );
}

#[test]
fn gemini_canvas_program_music_browser_fallback_forbidden_error_matches_contract() {
    let error =
        gemini_canvas_program_music_browser_fallback_forbidden_error("gemini_canvas_compatible");
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_program_music_browser_fallback_forbidden")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas program-owned music did not expose a direct no-key contract. Browser execution fallback is disabled on the default path."
    );
}

#[test]
fn gemini_canvas_program_music_no_key_request_contract_missing_error_matches_contract() {
    let error = gemini_canvas_program_music_no_key_request_contract_missing_error(
        "gemini_canvas_compatible",
    );
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_program_music_no_key_contract_missing")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas program-owned music StreamGenerate contract is missing requestUrl/requestBody."
    );
}

#[test]
fn decode_gemini_canvas_program_music_no_key_audio_reports_missing_audio_contract() {
    let error = decode_gemini_canvas_program_music_no_key_audio(None, None)
        .expect_err("missing audio should fail");
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_program_music_no_key_missing_audio")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas preview-frame no-key music websocket completed without audio bytes."
    );
}

#[test]
fn decode_gemini_canvas_program_music_no_key_audio_reports_invalid_audio_contract() {
    let error =
        decode_gemini_canvas_program_music_no_key_audio(Some("not-base64"), Some("audio/ogg"))
            .expect_err("invalid audio should fail");
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_program_music_no_key_invalid_audio")
    );
    assert!(error.message.starts_with(
        "Gemini Canvas preview-frame no-key music websocket returned invalid base64 audio bytes:"
    ));
}

#[test]
fn select_preferred_gemini_canvas_music_asset_prefers_audio_over_video_placeholder() {
    let assets = vec![
        make_music_asset("video", "https://example.invalid/fallback.mp4", "video/mp4"),
        make_music_asset("audio", "https://example.invalid/final.mp3", "audio/mpeg"),
    ];

    let selected = select_preferred_gemini_canvas_music_asset(&assets)
        .expect("music asset should be selected");
    assert_eq!(selected.kind, "audio");
    assert_eq!(selected.mime_type, "audio/mpeg");
    assert_eq!(selected.url, "https://example.invalid/final.mp3");
}

#[test]
fn gemini_canvas_music_body_indicates_accepted_progress_for_generation_tokens() {
    let body = concat!(
        ")]}'\n\n",
        "135\n",
        "[[\"wrb.fr\",null,\"[null,[\\\"c_music\\\",\\\"r_music\\\"],{\\\"11\\\":[\\\"Electronic Music Cue Generation\\\"],\\\"44\\\":true}]\"]]\n",
        "133\n",
        "[[\"wrb.fr\",null,\"[null,[\\\"c_music\\\",\\\"r_music\\\"],{\\\"26\\\":\\\"AwAAAAAAAAAQwBHO-LzoF6Ltg6rx4Bk\\\",\\\"44\\\":true}]\"]]\n"
    );
    assert!(gemini_canvas_music_body_indicates_accepted_progress(body));
}

#[test]
fn build_gemini_canvas_music_accepted_response_from_body_prefers_stream_locator() {
    let body = concat!(
        ")]}'\n\n",
        "177\n",
        "[[\"wrb.fr\",null,\"[null,[\\\"c_611d46744df5f83e\\\",\\\"r_5d8c944e0c56973b\\\"],{\\\"18\\\":\\\"r_5d8c944e0c56973b\\\",\\\"21\\\":[\\\"token\\\"],\\\"44\\\":true}]\"]]\n",
        "135\n",
        "[[\"wrb.fr\",null,\"[null,[\\\"c_611d46744df5f83e\\\",\\\"r_5d8c944e0c56973b\\\"],{\\\"11\\\":[\\\"Electronic Music Cue Generation\\\"],\\\"44\\\":true}]\"]]\n"
    );

    let response = build_gemini_canvas_music_accepted_response_from_body(
        "gemini-3-flash-preview",
        "test prompt",
        Some(30.0),
        body,
        Some("c_fallback"),
        Some("r_fallback"),
        Some("/app/fallback"),
    );
    assert_eq!(response["accepted"], json!(true));
    assert_eq!(response["completed"], json!(false));
    assert_eq!(response["conversation_id"], json!("c_611d46744df5f83e"));
    assert_eq!(response["response_id"], json!("r_5d8c944e0c56973b"));
    assert_eq!(response["app_path"], json!("/app/611d46744df5f83e"));
}

#[test]
fn gemini_canvas_music_response_requires_browser_followup_for_pending_acceptance() {
    let response = json!({
        "accepted": true,
        "completed": false,
        "data": [{
            "kind": "audio",
            "status": "pending",
            "url": null,
        }]
    });

    assert!(gemini_canvas_music_response_requires_browser_followup(
        &response
    ));
}

#[test]
fn gemini_canvas_music_response_does_not_require_browser_followup_for_completed_asset() {
    let response = json!({
        "completed": true,
        "data": [{
            "kind": "audio",
            "status": "completed",
            "url": "https://example.invalid/final.mp3",
        }]
    });

    assert!(!gemini_canvas_music_response_requires_browser_followup(
        &response
    ));
}
