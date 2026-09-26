use super::*;

fn sample_followup_target() -> GeminiCanvasFollowupTarget {
    GeminiCanvasFollowupTarget {
        locator: Some(gemini_canvas::GeminiCanvasStreamGenerateLocator {
            response_id: "resp_123".to_string(),
            conversation_id: "c_456".to_string(),
            app_path: "/app/followup".to_string(),
        }),
        source_path: "/app/followup".to_string(),
        mode: GeminiCanvasPageTargetMode::Resolved,
    }
}

#[test]
fn gemini_canvas_media_followup_failed_error_matches_contract() {
    let error = gemini_canvas_media_followup_failed_error(
        "gemini_canvas_compatible",
        &sample_followup_target(),
        "https://example.test/bootstrap",
        "timeout",
    );
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_media_followup_failed")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas pure HTTP media aPya6c follow-up failed. locator_mode=resolved; app_path=/app/followup; response_id=resp_123; conversation_id=c_456; bootstrap_page=https://example.test/bootstrap; upstream=timeout"
    );
}

#[test]
fn gemini_canvas_media_followup_missing_result_error_matches_contract() {
    let error = gemini_canvas_media_followup_missing_result_error(
        "gemini_canvas_compatible",
        &sample_followup_target(),
        "https://example.test/bootstrap",
    );
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_media_followup_missing_result")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas pure HTTP media aPya6c follow-up produced neither body nor explicit error. locator_mode=resolved; app_path=/app/followup; response_id=resp_123; conversation_id=c_456; bootstrap_page=https://example.test/bootstrap"
    );
}

#[test]
fn gemini_canvas_media_followup_bootstrap_failed_error_matches_contract() {
    let error = gemini_canvas_media_followup_bootstrap_failed_error(
        GeminiCanvasPageTargetMode::RootAppFallback,
        "/app/bootstrap",
        "https://a.test/app,https://b.test/share",
        "https://a.test/app: timeout | https://b.test/share: 403 challenge",
    );
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_media_followup_bootstrap_failed")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas media follow-up could not bootstrap the conversation page. locator_mode=root_app_fallback; app_path=/app/bootstrap; attempted_urls=https://a.test/app,https://b.test/share; failures=https://a.test/app: timeout | https://b.test/share: 403 challenge"
    );
}

#[test]
fn gemini_canvas_program_video_invoke_target_missing_error_matches_contract() {
    let error = gemini_canvas_program_video_invoke_target_missing_error();
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_program_video_invoke_target_missing")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas program-owned video lane is missing an explicit app invoke target."
    );
}

#[test]
fn gemini_canvas_program_video_no_key_request_contract_missing_error_matches_contract() {
    let error = gemini_canvas_program_video_no_key_request_contract_missing_error(
        "gemini_canvas_compatible",
    );
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_program_video_no_key_contract_missing")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas program-owned video StreamGenerate contract is missing requestUrl/requestBody."
    );
}

#[test]
fn gemini_canvas_program_video_browser_fallback_forbidden_error_matches_contract() {
    let error =
        gemini_canvas_program_video_browser_fallback_forbidden_error("gemini_canvas_compatible");
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_program_video_browser_fallback_forbidden")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas program-owned video did not expose a direct no-key contract. Browser execution fallback is disabled on the default path."
    );
}

#[test]
fn gemini_canvas_program_video_no_key_empty_body_error_matches_contract() {
    let error = gemini_canvas_program_video_no_key_empty_body_error("gemini_canvas_compatible");
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_program_video_no_key_empty_body")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas preview-frame no-key video fetch returned an empty body."
    );
}

#[test]
fn gemini_canvas_program_video_no_key_invalid_json_error_matches_contract() {
    let error = gemini_canvas_program_video_no_key_invalid_json_error(
        "gemini_canvas_compatible",
        "expected value",
    );
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_program_video_no_key_invalid_json")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas preview-frame no-key video fetch returned non-JSON body: expected value"
    );
}

#[test]
fn gemini_canvas_program_video_no_key_request_exhausted_error_matches_contract() {
    let error =
        gemini_canvas_program_video_no_key_request_exhausted_error("gemini_canvas_compatible");
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_program_video_no_key_request_exhausted")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas preview-frame no-key video invoke exhausted all candidate request URLs."
    );
}

#[test]
fn gemini_canvas_program_video_no_key_poll_empty_body_error_matches_contract() {
    let error =
        gemini_canvas_program_video_no_key_poll_empty_body_error("gemini_canvas_compatible");
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_program_video_no_key_poll_empty_body")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas preview-frame no-key video poll returned an empty body."
    );
}

#[test]
fn gemini_canvas_program_video_no_key_poll_invalid_json_error_matches_contract() {
    let error = gemini_canvas_program_video_no_key_poll_invalid_json_error(
        "gemini_canvas_compatible",
        "expected value",
    );
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_program_video_no_key_invalid_json")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas preview-frame no-key video poll returned non-JSON body: expected value"
    );
}

#[test]
fn gemini_canvas_video_operation_timeout_error_matches_contract() {
    let error = gemini_canvas_video_operation_timeout_error("gemini_canvas_compatible");
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_video_operation_timeout")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas video generation timed out before the operation completed."
    );
}

#[test]
fn gemini_canvas_video_missing_operation_error_matches_contract() {
    let error = gemini_canvas_video_missing_operation_error("gemini_canvas_compatible");
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_video_missing_operation")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas video generation did not return an operation name."
    );
}

#[test]
fn gemini_canvas_video_music_modality_mismatch_error_matches_contract() {
    let error = gemini_canvas_video_music_modality_mismatch_error("gemini_canvas_compatible");
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_video_music_modality_mismatch")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas video generation resolved a music-branded media body instead of a real video result."
    );
}

#[test]
fn gemini_canvas_video_unsupported_count_error_matches_contract() {
    let error = gemini_canvas_video_unsupported_count_error("gemini_canvas_compatible");
    assert_eq!(error.http_status, Some(400));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("unsupported_gemini_canvas_video_count")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas video generation currently supports only n=1 requests."
    );
}

#[test]
fn gemini_canvas_modular_video_unsupported_count_error_matches_contract() {
    let error = gemini_canvas_modular_video_unsupported_count_error("gemini_canvas_compatible");
    assert_eq!(error.http_status, Some(400));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("unsupported_gemini_canvas_modular_video_count")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas modular browser relay video generation currently supports only n=1 requests."
    );
}

#[test]
fn gemini_canvas_video_missing_asset_error_matches_contract() {
    let error = gemini_canvas_video_missing_asset_error("gemini_canvas_compatible");
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(error.code.as_deref(), Some("gemini_canvas_no_video_asset"));
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas video generation completed without a downloadable video asset."
    );
}

#[test]
fn gemini_canvas_video_followup_missing_locator_error_matches_contract() {
    let error = gemini_canvas_video_followup_missing_locator_error(
        "gemini_canvas_compatible",
        "/app/path",
        "https://example.test/bootstrap",
    );
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_stream_generate_missing_conversation_id")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas video follow-up could not recover a usable conversation locator from the StreamGenerate response or recent conversation list. app_path=/app/path; bootstrap_page=https://example.test/bootstrap"
    );
}
