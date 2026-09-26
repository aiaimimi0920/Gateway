use super::{
    build_business_generated_drafts, build_canvas_generated_drafts, build_web_generated_drafts,
    extract_last_json_object, now_rfc3339, parse_remote_gemini_business_capture_output,
    parse_remote_gemini_canvas_capture_output, parse_remote_gemini_web_capture_output,
    prepare_gemini_auth_session_control_state, resolve_gemini_auth_helper_execution_mode,
    GeminiAuthFamily, GeminiAuthHelperExecutionMode, GeminiAuthSessionControlState,
    GeminiAuthSessionManager, GeminiAuthSessionStatus, GeminiAuthSessionView,
    GeminiWebCaptureOutput,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use time::OffsetDateTime;

#[test]
fn extracts_last_json_object_after_helper_logs() {
    let output = r#"
[gemini-canvas-export] Opening browser at https://gemini.google.com/app
[gemini-canvas-export] Complete Gemini login in the opened browser window.
{
  "ok": true,
  "runtimeStateObjectKey": "credential-runtime/gemini-canvas/demo/storage-state.json",
  "suggestedShareId": "fe24c455a570"
}
"#;
    let extracted = extract_last_json_object(output).expect("json");
    assert!(extracted.contains("\"runtimeStateObjectKey\""));
}

#[test]
fn build_canvas_generated_drafts_shares_runtime_material_across_both_families() {
    let drafts = build_canvas_generated_drafts(
        "credential-runtime/gemini-canvas/demo/storage-state.json",
        "fe24c455a570",
        None,
        None,
    );
    assert_eq!(drafts.len(), 2);
    assert_eq!(drafts[0].provider_id, "gemini-canvas");
    assert_eq!(drafts[1].provider_id, "gemini-canvas-chat");
    assert_eq!(
        drafts[0].credential["runtime_state_object_key"],
        Value::String("credential-runtime/gemini-canvas/demo/storage-state.json".to_string())
    );
    assert_eq!(
        drafts[1].credential["extra_body"]["apiBaseUrl"],
        Value::String("https://generativelanguage.googleapis.com/v1beta".to_string())
    );
    assert!(drafts[0].credential.get("api_key").is_none());
    assert!(drafts[1].credential.get("api_key").is_none());
}

#[test]
fn build_canvas_generated_drafts_keeps_official_api_keys_out_of_program_owned_draft() {
    let drafts = build_canvas_generated_drafts(
        "credential-runtime/gemini-canvas/demo/storage-state.json",
        "fe24c455a570",
        Some("credential-runtime/gemini-canvas/demo/browser-profile"),
        Some(&[
            "AIzaCapturedOne123456789012".to_string(),
            "AIzaCapturedTwo123456789012".to_string(),
            "AIzaCapturedOne123456789012".to_string(),
        ]),
    );

    assert!(drafts[0].credential["extra_body"].get("apiKeys").is_none());
    assert_eq!(
        drafts[1].credential["extra_body"]["apiKeys"],
        json!(["AIzaCapturedOne123456789012", "AIzaCapturedTwo123456789012"])
    );
    assert_eq!(
        drafts[0].credential["extra_body"]["browserRuntimeStateObjectKey"],
        Value::String("credential-runtime/gemini-canvas/demo/browser-profile".to_string())
    );
    assert_eq!(
        drafts[1].credential["extra_body"]["browserRuntimeStateObjectKey"],
        Value::String("credential-runtime/gemini-canvas/demo/browser-profile".to_string())
    );
}

#[test]
fn build_canvas_generated_drafts_keeps_browser_profile_override_for_profile_dirs() {
    let drafts = build_canvas_generated_drafts(
        "credential-runtime/gemini-canvas/demo/browser-profile",
        "fe24c455a570",
        Some("credential-runtime/gemini-canvas/demo/browser-profile"),
        None,
    );

    assert_eq!(
        drafts[0].credential["extra_body"]["browserRuntimeStateObjectKey"],
        Value::String("credential-runtime/gemini-canvas/demo/browser-profile".to_string())
    );
    assert_eq!(
        drafts[1].credential["extra_body"]["browserRuntimeStateObjectKey"],
        Value::String("credential-runtime/gemini-canvas/demo/browser-profile".to_string())
    );
}

#[test]
fn build_business_generated_draft_keeps_jwt_in_secret_edits() {
    let drafts =
        build_business_generated_drafts("ey.demo.jwt", "cfg-123", "projects/demo/sessions/abc");
    assert_eq!(drafts.len(), 1);
    assert_eq!(drafts[0].provider_id, "gemini-business");
    assert_eq!(
        drafts[0].credential["extra_body"]["configId"],
        Value::String("cfg-123".to_string())
    );
    assert_eq!(
        drafts[0].credential["extra_body"]["session"],
        Value::String("projects/demo/sessions/abc".to_string())
    );
    assert_eq!(drafts[0].secret_edits.len(), 1);
    assert_eq!(drafts[0].secret_edits[0].operation, "replace");
    assert_eq!(drafts[0].secret_edits[0].value, "ey.demo.jwt");
}

#[test]
fn build_web_generated_draft_keeps_both_cookies_in_secret_edits() {
    let output = GeminiWebCaptureOutput {
        ok: true,
        error: None,
        api_key: Some("primary-cookie".to_string()),
        auth_token: Some("secondary-cookie".to_string()),
        account_index: Some("1".to_string()),
        access_token: Some("access-1".to_string()),
        build_label: Some("build-1".to_string()),
        session_id: Some("session-1".to_string()),
        language: Some("zh-CN".to_string()),
        app_page_path: Some("/u/1/app".to_string()),
        endpoint_path: Some(
            "/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate".to_string(),
        ),
        referer: Some("https://gemini.google.com/u/1/app".to_string()),
        model_headers: Some(HashMap::from([(
            "x-goog-ext-525001261-jspb".to_string(),
            "model-header".to_string(),
        )])),
        request_context_header: Some("context-header".to_string()),
        response_status: Some(200),
        response_contains_paris: Some(true),
    };
    let drafts = build_web_generated_drafts(
        "gemini-web-secondary",
        "primary-cookie",
        Some("secondary-cookie"),
        &output,
    );

    assert_eq!(drafts.len(), 1);
    assert_eq!(drafts[0].provider_id, "gemini-web-secondary");
    assert_eq!(drafts[0].credential["extra_body"]["authUser"], "1");
    assert_eq!(
        drafts[0].credential["extra_body"]["appPagePath"],
        "/u/1/app"
    );
    assert_eq!(drafts[0].credential["headers"]["X-Goog-AuthUser"], "1");
    assert_eq!(drafts[0].secret_edits.len(), 2);
    assert_eq!(drafts[0].secret_edits[0].field, "api_key");
    assert_eq!(drafts[0].secret_edits[1].field, "auth_token");
    assert!(drafts[0].credential["api_key"].as_str().unwrap().is_empty());
    assert!(drafts[0].credential["auth_token"]
        .as_str()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn create_business_session_starts_waiting_for_manual_capture() {
    let manager = Arc::new(GeminiAuthSessionManager::default());
    let session = manager.create_session(super::CreateGeminiAuthSessionInput {
        target_family: GeminiAuthFamily::GeminiBusiness,
        provider_id: "gemini-business".to_string(),
        account_label: None,
    });
    assert_eq!(session.target_family, GeminiAuthFamily::GeminiBusiness);
    assert_eq!(session.status, GeminiAuthSessionStatus::WaitingUser);
    assert!(session.message.contains("Gemini Business"));
    let loaded = manager.get_session(&session.id).expect("stored session");
    assert_eq!(loaded.status, GeminiAuthSessionStatus::WaitingUser);
}

#[test]
fn remote_gemini_canvas_payload_parses_runtime_state_contract() {
    let parsed = parse_remote_gemini_canvas_capture_output(json!({
        "ok": true,
        "targetFamily": "gemini-canvas",
        "runtimeStateObjectKey": "credential-runtime/gemini-canvas/host-export/storage-state.json",
        "browserRuntimeStateObjectKey": "credential-runtime/gemini-canvas/host-export",
        "suggestedShareId": "fe24c455a570",
        "apiKeys": ["AIzaCapturedOne123456789012"],
    }))
    .expect("parse remote canvas payload");

    assert_eq!(
        parsed.runtime_state_object_key.as_deref(),
        Some("credential-runtime/gemini-canvas/host-export/storage-state.json")
    );
    assert_eq!(
        parsed.browser_runtime_state_object_key.as_deref(),
        Some("credential-runtime/gemini-canvas/host-export")
    );
    assert_eq!(parsed.suggested_share_id.as_deref(), Some("fe24c455a570"));
    assert_eq!(
        parsed.api_keys.as_deref(),
        Some(&["AIzaCapturedOne123456789012".to_string()][..])
    );
}

#[test]
fn remote_gemini_business_payload_parses_secret_fields_contract() {
    let parsed = parse_remote_gemini_business_capture_output(json!({
        "ok": true,
        "targetFamily": "gemini-business",
        "jwt": "ey.demo.jwt",
        "configId": "cfg-123",
        "session": "projects/demo/sessions/abc",
    }))
    .expect("parse remote business payload");

    assert_eq!(parsed.jwt.as_deref(), Some("ey.demo.jwt"));
    assert_eq!(parsed.config_id.as_deref(), Some("cfg-123"));
    assert_eq!(
        parsed.session.as_deref(),
        Some("projects/demo/sessions/abc")
    );
}

#[test]
fn remote_gemini_web_payload_requires_successful_validation_response() {
    let parsed = parse_remote_gemini_web_capture_output(json!({
        "ok": true,
        "targetFamily": "gemini-web",
        "apiKey": "primary-cookie",
        "authToken": "secondary-cookie",
        "accountIndex": "1",
        "appPagePath": "/u/1/app",
        "responseStatus": 200,
        "responseContainsParis": true,
    }))
    .expect("parse remote web payload");

    assert_eq!(parsed.api_key.as_deref(), Some("primary-cookie"));
    assert_eq!(parsed.auth_token.as_deref(), Some("secondary-cookie"));
    assert_eq!(parsed.account_index.as_deref(), Some("1"));

    let error = parse_remote_gemini_web_capture_output(json!({
        "ok": true,
        "apiKey": "primary-cookie",
        "responseStatus": 401,
    }))
    .expect_err("401 capture must fail");
    assert!(error.contains("HTTP 401"));

    let error = parse_remote_gemini_web_capture_output(json!({
        "ok": true,
        "apiKey": "primary-cookie",
        "responseStatus": 200,
        "responseContainsParis": false,
    }))
    .expect_err("an incorrect validation answer must fail");
    assert!(error.contains("expected answer"));
}

#[test]
fn displayless_linux_requires_host_executor_when_remote_helper_missing() {
    assert_eq!(
        resolve_gemini_auth_helper_execution_mode("linux", None, None, None),
        GeminiAuthHelperExecutionMode::HostExecutorRequired
    );
}

#[test]
fn displayless_linux_prefers_remote_helper_when_configured() {
    assert_eq!(
        resolve_gemini_auth_helper_execution_mode(
            "linux",
            None,
            None,
            Some("http://host.docker.internal:42341"),
        ),
        GeminiAuthHelperExecutionMode::RemotePreferred
    );
}

#[test]
fn request_manual_completion_marks_waiting_canvas_session_and_creates_signal_file() {
    let manager = GeminiAuthSessionManager::default();
    let session_id = format!(
        "manual-complete-{}",
        OffsetDateTime::now_utc().unix_timestamp_nanos()
    );
    let control_state =
        prepare_gemini_auth_session_control_state(session_id.as_str()).expect("control state");
    let complete_signal_relative_path = control_state.complete_signal_relative_path.clone();
    let complete_signal_absolute_path =
        super::absolute_control_path(complete_signal_relative_path.as_str());

    manager.sessions.insert(
        session_id.clone(),
        GeminiAuthSessionView {
            id: session_id.clone(),
            target_family: GeminiAuthFamily::GeminiCanvas,
            provider_id: "gemini-canvas".to_string(),
            status: GeminiAuthSessionStatus::WaitingUser,
            message: "waiting".to_string(),
            created_at: now_rfc3339(),
            updated_at: now_rfc3339(),
            generated_drafts: Vec::new(),
        },
    );
    manager.controls.insert(
        session_id.clone(),
        GeminiAuthSessionControlState {
            complete_signal_relative_path,
        },
    );

    let updated = manager
        .request_manual_completion(session_id.as_str())
        .expect("manual completion");

    assert_eq!(updated.status, GeminiAuthSessionStatus::WaitingUser);
    assert!(updated.message.contains("Finishing capture"));
    assert!(complete_signal_absolute_path.exists());

    let _ = std::fs::remove_file(complete_signal_absolute_path);
}
