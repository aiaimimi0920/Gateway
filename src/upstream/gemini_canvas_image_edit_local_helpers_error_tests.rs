use super::*;
#[test]
fn image_edit_missing_push_id_error_matches_contract() {
    let error = gemini_canvas_image_edit_missing_push_id_error();
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_image_edit_missing_push_id")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas image edit upload requires bootstrap push_id from the /app page."
    );
}

#[test]
fn image_edit_missing_client_pctx_error_matches_contract() {
    let error = gemini_canvas_image_edit_missing_client_pctx_error();
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_image_edit_missing_client_pctx")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas image edit upload requires bootstrap client_pctx from the /app page."
    );
}

#[test]
fn image_edit_missing_upload_url_error_matches_contract() {
    let error = gemini_canvas_image_edit_missing_upload_url_error();
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_image_edit_missing_upload_url")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas image edit upload start response did not expose an upload URL."
    );
}

#[test]
fn image_edit_missing_resource_path_error_matches_contract() {
    let error = gemini_canvas_image_edit_missing_resource_path_error();
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_image_edit_missing_resource_path")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas image edit upload finalize response did not return a contrib_service resource path."
    );
}
#[test]
fn image_edit_post_ack_missing_app_url_error_matches_contract() {
    let error = gemini_canvas_image_edit_post_ack_missing_app_url_error();
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_image_edit_post_ack_missing_app_url")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas image edit post-ack follow-up requires a concrete signaler app url."
    );
}

#[test]
fn image_edit_post_ack_bootstrap_missing_error_matches_contract() {
    let error = gemini_canvas_image_edit_post_ack_bootstrap_missing_error();
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_image_edit_post_ack_bootstrap_missing")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas image edit post-ack follow-up could not bootstrap /app and payload cache did not provide a fallback bootstrap."
    );
}

#[test]
fn image_edit_conversation_bootstrap_missing_error_matches_contract() {
    let error = gemini_canvas_image_edit_conversation_bootstrap_missing_error();
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_image_edit_conversation_bootstrap_missing")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas image edit conversation follow-up could not bootstrap /app and payload cache did not provide a fallback bootstrap."
    );
}

#[test]
fn image_edit_page_refresh_bootstrap_missing_error_matches_contract() {
    let error = gemini_canvas_image_page_refresh_bootstrap_missing_error();
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_image_page_refresh_bootstrap_missing")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas image page refresh could not bootstrap /app and payload cache did not provide a fallback bootstrap."
    );
}

#[test]
fn image_edit_signaler_missing_account_id_error_matches_contract() {
    let error = gemini_canvas_image_edit_signaler_missing_account_id_error();
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_signaler_missing_account_id")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas image edit signaler bootstrap did not expose S06Grb account id."
    );
}

#[test]
fn image_edit_signaler_missing_api_key_error_matches_contract() {
    let error = gemini_canvas_image_edit_signaler_missing_api_key_error();
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_signaler_missing_api_key")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas image edit signaler bootstrap did not expose a Google API key."
    );
}

#[test]
fn image_edit_signaler_all_keys_failed_error_matches_contract() {
    let error = gemini_canvas_image_edit_signaler_all_keys_failed_error("key1=403 | key2=timeout");
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_signaler_all_keys_failed")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas image edit signaler bootstrap exhausted all Google API key candidates. failures=key1=403 | key2=timeout"
    );
}

#[test]
fn image_edit_signaler_handoff_ready_error_matches_contract() {
    let error =
        gemini_canvas_image_edit_signaler_handoff_ready_error(3, 42, "{\"state\":\"ready\"}");
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_image_edit_signaler_handoff_ready")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas image edit signaler reached concrete app paths but has not surfaced a usable image asset yet. distinct_app_paths=3; next_aid=42; last_body_preview={\"state\":\"ready\"}"
    );
}

#[test]
fn image_edit_signaler_missing_asset_error_matches_contract() {
    let error = gemini_canvas_image_edit_signaler_missing_asset_error(
        9,
        "poll1=empty | poll2=403",
        "<preview>",
    );
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_image_edit_signaler_missing_asset")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas image edit signaler poll did not expose a usable media asset. next_aid=9; failures=poll1=empty | poll2=403; last_body_preview=<preview>"
    );
}

#[test]
fn image_edit_conversation_followup_failed_error_matches_contract() {
    let error = gemini_canvas_image_edit_conversation_followup_failed_error(
        "/app/canvas",
        4,
        "prompt-preview",
        "entries-preview",
        "probe-preview",
        "full-preview",
        "completion-preview",
        "parity-probe",
        "parity-full",
        "parity-o30",
        "parity-k4",
        "attempt1=timeout | attempt2=empty",
    );
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_image_edit_conversation_followup_failed")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas image edit conversation follow-up did not expose a usable image asset. bootstrap_page=/app/canvas; attempts=4; prompt_preview=prompt-preview; entries_preview=entries-preview; last_probe_preview=probe-preview; last_full_preview=full-preview; last_completion_preview=completion-preview; parity_probe_preview=parity-probe; parity_full_preview=parity-full; parity_o30_preview=parity-o30; parity_k4_preview=parity-k4; failures=attempt1=timeout | attempt2=empty"
    );
}
