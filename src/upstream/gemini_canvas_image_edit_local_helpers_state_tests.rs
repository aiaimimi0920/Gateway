use super::*;
use crate::upstream::response_preview_helpers::compact_response_preview;
use serde_json::json;

#[test]
fn record_gemini_canvas_image_edit_signaler_locator_from_body_preserves_locator_contract() {
    let frame1 = serde_json::to_string(&vec![json!([
        "wrb.fr",
        null,
        "[null,[null,\"r_e0e4aa76ab2755e3\"],{\"18\":\"r_e0e4aa76ab2755e3\",\"44\":false}]"
    ])])
    .expect("frame1");
    let frame2 = serde_json::to_string(&vec![json!([
        "wrb.fr",
        null,
        "[null,[\"c_1004db0ef60d9dda\",\"r_e0e4aa76ab2755e3\"],null,null,[]]"
    ])])
    .expect("frame2");
    let body = format!(
        ")]}}'\n\n{}\n{}\n{}\n{}\n",
        frame1.encode_utf16().count(),
        frame1,
        frame2.encode_utf16().count(),
        frame2
    );
    let mut context = GeminiCanvasImageEditFollowupContext {
        prompt: "edit prompt".to_string(),
        request_started_at: std::time::SystemTime::UNIX_EPOCH,
        locale_hint: None,
        signaler_session: None,
        signaler_channel: None,
        signaler_app_urls: Vec::new(),
        signaler_app_url: None,
        signaler_conversation_id: None,
        signaler_response_id: None,
    };

    record_gemini_canvas_image_edit_signaler_locator_from_body(&mut context, &body);

    assert_eq!(
        context.signaler_response_id.as_deref(),
        Some("r_e0e4aa76ab2755e3")
    );
    assert_eq!(
        context.signaler_conversation_id.as_deref(),
        Some("c_1004db0ef60d9dda")
    );
}
#[test]
fn record_gemini_canvas_image_edit_signaler_poll_body_preview_preserves_locator_contract() {
    let frame1 = serde_json::to_string(&vec![json!([
        "wrb.fr",
        null,
        "[null,[null,\"r_e0e4aa76ab2755e3\"],{\"18\":\"r_e0e4aa76ab2755e3\",\"44\":false}]"
    ])])
    .expect("frame1");
    let frame2 = serde_json::to_string(&vec![json!([
        "wrb.fr",
        null,
        "[null,[\"c_1004db0ef60d9dda\",\"r_e0e4aa76ab2755e3\"],null,null,[]]"
    ])])
    .expect("frame2");
    let body = format!(
        ")]}}'\n\n{}\n{}\n{}\n{}\n",
        frame1.encode_utf16().count(),
        frame1,
        frame2.encode_utf16().count(),
        frame2
    );
    let mut context = GeminiCanvasImageEditFollowupContext {
        prompt: "edit prompt".to_string(),
        request_started_at: std::time::SystemTime::UNIX_EPOCH,
        locale_hint: None,
        signaler_session: None,
        signaler_channel: None,
        signaler_app_urls: Vec::new(),
        signaler_app_url: None,
        signaler_conversation_id: None,
        signaler_response_id: None,
    };

    let preview =
        record_gemini_canvas_image_edit_signaler_poll_body_preview(Some(&mut context), &body);

    assert_eq!(preview, compact_response_preview(&body, 240));
    assert_eq!(
        context.signaler_response_id.as_deref(),
        Some("r_e0e4aa76ab2755e3")
    );
    assert_eq!(
        context.signaler_conversation_id.as_deref(),
        Some("c_1004db0ef60d9dda")
    );
}

#[test]
fn record_gemini_canvas_image_edit_signaler_followup_state_from_body_preserves_state_contract() {
    let frame1 = serde_json::to_string(&vec![json!([
        "wrb.fr",
        null,
        "[null,[null,\"r_e0e4aa76ab2755e3\"],{\"18\":\"r_e0e4aa76ab2755e3\",\"44\":false}]"
    ])])
    .expect("frame1");
    let frame2 = serde_json::to_string(&vec![json!([
        "wrb.fr",
        null,
        "[null,[\"c_1004db0ef60d9dda\",\"r_e0e4aa76ab2755e3\"],null,null,[]]"
    ])])
    .expect("frame2");
    let body = format!(
        ")]}}'\n\n{}\n{}\n{}\n{}\n",
        frame1.encode_utf16().count(),
        frame1,
        frame2.encode_utf16().count(),
        frame2
    );
    let session = gemini_canvas::GeminiCanvasPureHttpSession {
        cookie_header: "SID=abc; SAPISID=def".to_string(),
        sapisid: "def".to_string(),
        auth_user: "1".to_string(),
    };
    let channel = crate::upstream::gemini_canvas_client_types::GeminiCanvasSignalerChannel {
        api_key: "api-key".to_string(),
        gsession_id: "gsession-id".to_string(),
        sid: "sid".to_string(),
        next_aid: 7,
    };
    let mut context = GeminiCanvasImageEditFollowupContext {
        prompt: "edit prompt".to_string(),
        request_started_at: std::time::SystemTime::UNIX_EPOCH,
        locale_hint: None,
        signaler_session: None,
        signaler_channel: None,
        signaler_app_urls: Vec::new(),
        signaler_app_url: None,
        signaler_conversation_id: None,
        signaler_response_id: None,
    };

    record_gemini_canvas_image_edit_signaler_followup_state_from_body(
        &mut context,
        &session,
        &channel,
        Some("en-US"),
        &body,
    );

    assert_eq!(
        context
            .signaler_session
            .as_ref()
            .map(|value| value.auth_user.as_str()),
        Some("1")
    );
    assert_eq!(
        context
            .signaler_channel
            .as_ref()
            .map(|value| value.next_aid),
        Some(7)
    );
    assert_eq!(context.locale_hint.as_deref(), Some("en-US"));
    assert_eq!(
        context.signaler_response_id.as_deref(),
        Some("r_e0e4aa76ab2755e3")
    );
    assert_eq!(
        context.signaler_conversation_id.as_deref(),
        Some("c_1004db0ef60d9dda")
    );
}

#[test]
fn record_gemini_canvas_image_edit_signaler_followup_state_preserves_existing_locale_contract() {
    let session = gemini_canvas::GeminiCanvasPureHttpSession {
        cookie_header: "SID=abc; SAPISID=def".to_string(),
        sapisid: "def".to_string(),
        auth_user: "1".to_string(),
    };
    let channel = crate::upstream::gemini_canvas_client_types::GeminiCanvasSignalerChannel {
        api_key: "api-key".to_string(),
        gsession_id: "gsession-id".to_string(),
        sid: "sid".to_string(),
        next_aid: 7,
    };
    let mut context = GeminiCanvasImageEditFollowupContext {
        prompt: "edit prompt".to_string(),
        request_started_at: std::time::SystemTime::UNIX_EPOCH,
        locale_hint: Some("zh-CN".to_string()),
        signaler_session: None,
        signaler_channel: None,
        signaler_app_urls: Vec::new(),
        signaler_app_url: None,
        signaler_conversation_id: None,
        signaler_response_id: None,
    };

    record_gemini_canvas_image_edit_signaler_followup_state(
        &mut context,
        &session,
        &channel,
        Some("en-US"),
    );

    assert_eq!(context.locale_hint.as_deref(), Some("zh-CN"));
    assert_eq!(
        context
            .signaler_session
            .as_ref()
            .map(|value| value.auth_user.as_str()),
        Some("1")
    );
    assert_eq!(
        context
            .signaler_channel
            .as_ref()
            .map(|value| value.next_aid),
        Some(7)
    );
}

#[test]
fn finish_gemini_canvas_image_edit_signaler_missing_asset_records_state_and_error_contract() {
    let session = gemini_canvas::GeminiCanvasPureHttpSession {
        cookie_header: "SID=abc; SAPISID=def".to_string(),
        sapisid: "def".to_string(),
        auth_user: "1".to_string(),
    };
    let channel = crate::upstream::gemini_canvas_client_types::GeminiCanvasSignalerChannel {
        api_key: "api-key".to_string(),
        gsession_id: "gsession-id".to_string(),
        sid: "sid".to_string(),
        next_aid: 9,
    };
    let mut context = GeminiCanvasImageEditFollowupContext {
        prompt: "edit prompt".to_string(),
        request_started_at: std::time::SystemTime::UNIX_EPOCH,
        locale_hint: None,
        signaler_session: None,
        signaler_channel: None,
        signaler_app_urls: Vec::new(),
        signaler_app_url: None,
        signaler_conversation_id: None,
        signaler_response_id: None,
    };
    let failures = vec!["poll1=empty".to_string(), "poll2=403".to_string()];

    let error = finish_gemini_canvas_image_edit_signaler_missing_asset(
        Some(&mut context),
        &session,
        &channel,
        Some("en-US"),
        &failures,
        Some("<preview>".to_string()),
    );

    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_image_edit_signaler_missing_asset")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas image edit signaler poll did not expose a usable media asset. next_aid=9; failures=poll1=empty | poll2=403; last_body_preview=<preview>"
    );
    assert_eq!(context.locale_hint.as_deref(), Some("en-US"));
    assert_eq!(
        context
            .signaler_session
            .as_ref()
            .map(|value| value.auth_user.as_str()),
        Some("1")
    );
    assert_eq!(
        context
            .signaler_channel
            .as_ref()
            .map(|value| value.next_aid),
        Some(9)
    );
}

#[test]
fn record_gemini_canvas_image_edit_signaler_app_path_preserves_app_url_contract() {
    let mut seen_app_paths = std::collections::HashSet::new();
    let mut first_app_path_seen_at = None;
    let mut context = GeminiCanvasImageEditFollowupContext {
        prompt: "edit prompt".to_string(),
        request_started_at: std::time::SystemTime::UNIX_EPOCH,
        locale_hint: None,
        signaler_session: None,
        signaler_channel: None,
        signaler_app_urls: Vec::new(),
        signaler_app_url: None,
        signaler_conversation_id: None,
        signaler_response_id: None,
    };

    let page_url = record_gemini_canvas_image_edit_signaler_app_path(
        "https://gemini.google.com",
        "/app/1004db0ef60d9dda",
        &mut seen_app_paths,
        &mut first_app_path_seen_at,
        Some(&mut context),
    );
    let first_seen = first_app_path_seen_at;
    let duplicate_page_url = record_gemini_canvas_image_edit_signaler_app_path(
        "https://gemini.google.com",
        "/app/1004db0ef60d9dda",
        &mut seen_app_paths,
        &mut first_app_path_seen_at,
        Some(&mut context),
    );

    assert_eq!(page_url, "https://gemini.google.com/app/1004db0ef60d9dda");
    assert_eq!(duplicate_page_url, page_url);
    assert!(seen_app_paths.contains(&page_url));
    assert_eq!(seen_app_paths.len(), 1);
    assert!(first_app_path_seen_at.is_some());
    assert_eq!(first_app_path_seen_at, first_seen);
    assert_eq!(context.signaler_app_url.as_deref(), Some(page_url.as_str()));
    assert_eq!(context.signaler_app_urls, vec![page_url]);
}

#[test]
fn gemini_canvas_image_edit_signaler_handoff_ready_preserves_threshold_contract() {
    assert!(!gemini_canvas_image_edit_signaler_handoff_ready(
        0,
        Some(std::time::Duration::from_secs(121)),
    ));
    assert!(!gemini_canvas_image_edit_signaler_handoff_ready(
        2,
        Some(std::time::Duration::from_secs(119)),
    ));
    assert!(gemini_canvas_image_edit_signaler_handoff_ready(
        2,
        Some(std::time::Duration::from_secs(120)),
    ));
    assert!(gemini_canvas_image_edit_signaler_handoff_ready(
        3,
        Some(std::time::Duration::from_secs(1)),
    ));
    assert!(!gemini_canvas_image_edit_signaler_handoff_ready(3, None));
}

#[test]
fn try_finish_gemini_canvas_image_edit_signaler_handoff_ready_records_state_and_error_contract() {
    let frame1 = serde_json::to_string(&vec![json!([
        "wrb.fr",
        null,
        "[null,[null,\"r_e0e4aa76ab2755e3\"],{\"18\":\"r_e0e4aa76ab2755e3\",\"44\":false}]"
    ])])
    .expect("frame1");
    let frame2 = serde_json::to_string(&vec![json!([
        "wrb.fr",
        null,
        "[null,[\"c_1004db0ef60d9dda\",\"r_e0e4aa76ab2755e3\"],null,null,[]]"
    ])])
    .expect("frame2");
    let body = format!(
        ")]}}'\n\n{}\n{}\n{}\n{}\n",
        frame1.encode_utf16().count(),
        frame1,
        frame2.encode_utf16().count(),
        frame2
    );
    let session = gemini_canvas::GeminiCanvasPureHttpSession {
        cookie_header: "SID=abc; SAPISID=def".to_string(),
        sapisid: "def".to_string(),
        auth_user: "1".to_string(),
    };
    let channel = crate::upstream::gemini_canvas_client_types::GeminiCanvasSignalerChannel {
        api_key: "api-key".to_string(),
        gsession_id: "gsession-id".to_string(),
        sid: "sid".to_string(),
        next_aid: 42,
    };
    let mut context = GeminiCanvasImageEditFollowupContext {
        prompt: "edit prompt".to_string(),
        request_started_at: std::time::SystemTime::UNIX_EPOCH,
        locale_hint: None,
        signaler_session: None,
        signaler_channel: None,
        signaler_app_urls: Vec::new(),
        signaler_app_url: None,
        signaler_conversation_id: None,
        signaler_response_id: None,
    };

    let error = try_finish_gemini_canvas_image_edit_signaler_handoff_ready(
        3,
        Some(std::time::Duration::from_secs(1)),
        Some("{\"state\":\"ready\"}"),
        Some(&mut context),
        &session,
        &channel,
        Some("en-US"),
        &body,
    )
    .expect("handoff should be ready");

    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_image_edit_signaler_handoff_ready")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas image edit signaler reached concrete app paths but has not surfaced a usable image asset yet. distinct_app_paths=3; next_aid=42; last_body_preview={\"state\":\"ready\"}"
    );
    assert_eq!(context.locale_hint.as_deref(), Some("en-US"));
    assert_eq!(
        context
            .signaler_session
            .as_ref()
            .map(|value| value.auth_user.as_str()),
        Some("1")
    );
    assert_eq!(
        context
            .signaler_channel
            .as_ref()
            .map(|value| value.next_aid),
        Some(42)
    );
    assert_eq!(
        context.signaler_response_id.as_deref(),
        Some("r_e0e4aa76ab2755e3")
    );
    assert_eq!(
        context.signaler_conversation_id.as_deref(),
        Some("c_1004db0ef60d9dda")
    );
}

#[test]
fn build_gemini_canvas_image_edit_signaler_poll_url_preserves_contract() {
    let channel = crate::upstream::gemini_canvas_client_types::GeminiCanvasSignalerChannel {
        api_key: "api-key".to_string(),
        gsession_id: "gsession-id".to_string(),
        sid: "sid".to_string(),
        next_aid: 42,
    };

    let poll_url = build_gemini_canvas_image_edit_signaler_poll_url(&channel, "zx-token-123");

    assert_eq!(
        poll_url,
        "https://signaler-pa.clients6.google.com/punctual/multi-watch/channel?VER=8&gsessionid=gsession-id&key=api-key&RID=rpc&SID=sid&AID=42&CI=0&TYPE=xmlhttp&zx=zx-token-123&t=1"
    );
}

#[test]
fn update_gemini_canvas_image_edit_signaler_next_aid_from_body_preserves_max_aid_contract() {
    let mut channel = crate::upstream::gemini_canvas_client_types::GeminiCanvasSignalerChannel {
        api_key: "api-key".to_string(),
        gsession_id: "gsession-id".to_string(),
        sid: "sid".to_string(),
        next_aid: 7,
    };
    let body = concat!(
        "188\n",
        "[[1,[[null,null,[\"d5ty4AOG\"]]]],[2,[[[[\"1\",[[\"1777607372727478\"]]]]]]]]",
        "167\n",
        "[[18,[[[[\"4\",[null,null,[\"1777607613519203\"]]],",
        "[\"3\",[null,null,[\"1777607613519203\"]]]]]]]]"
    );

    let updated = update_gemini_canvas_image_edit_signaler_next_aid_from_body(&mut channel, body);

    assert_eq!(updated, Some(18));
    assert_eq!(channel.next_aid, 18);
}

#[test]
fn gemini_canvas_image_edit_signaler_page_failure_entry_preserves_contract() {
    let entry = gemini_canvas_image_edit_signaler_page_failure_entry(
        "https://gemini.google.com/app/abc",
        "fetch",
        "503 service unavailable",
    );

    assert_eq!(
        entry,
        "page_url=https://gemini.google.com/app/abc fetch=503 service unavailable"
    );
}

#[test]
fn gemini_canvas_image_edit_signaler_poll_error_entry_preserves_contract() {
    let entry = gemini_canvas_image_edit_signaler_poll_error_entry(42, "transport channel closed");

    assert_eq!(entry, "poll_aid=42 error=transport channel closed");
}

#[test]
fn gemini_canvas_image_edit_signaler_refresh_error_entry_preserves_contract() {
    let entry = gemini_canvas_image_edit_signaler_refresh_error_entry(43, "refresh token expired");

    assert_eq!(entry, "refresh_creds aid=43 error=refresh token expired");
}
