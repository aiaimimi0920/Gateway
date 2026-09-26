use super::image_edit_local_helpers_test_support::make_payload;
use super::*;
use serde_json::json;

#[test]
fn extract_gemini_canvas_image_edit_signaler_assets_from_body_preserves_followup_state_contract() {
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
        ")]}}'\n\n{}\n{}\n{}\n{}\n<script>window.__IMAGE__=[\"https:\\/\\/lh3.googleusercontent.com\\/rd-ogw\\/asset-token=s32-c\"];</script>",
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

    let assets = extract_gemini_canvas_image_edit_signaler_assets_from_body(
        &body,
        &session,
        &channel,
        Some("en-US"),
        Some(&mut context),
    )
    .expect("image asset should be extracted");

    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].kind, "image");
    assert_eq!(assets[0].mime_type, "image/png");
    assert_eq!(
        assets[0].url,
        "https://lh3.googleusercontent.com/rd-ogw/asset-token=s32-c"
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
        Some(7)
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
fn try_extract_gemini_canvas_image_edit_signaler_assets_response_from_body_preserves_body_contract()
{
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
        ")]}}'\n\n{}\n{}\n{}\n{}\n<script>window.__IMAGE__=[\"https:\\/\\/lh3.googleusercontent.com\\/rd-ogw\\/asset-token=s32-c\"];</script>",
        frame1.encode_utf16().count(),
        frame1,
        frame2.encode_utf16().count(),
        frame2
    );
    let expected_body = body.clone();
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

    let (assets, returned_body) =
        try_extract_gemini_canvas_image_edit_signaler_assets_response_from_body(
            body,
            &session,
            &channel,
            Some("en-US"),
            Some(&mut context),
        )
        .expect("image asset response should be extracted");

    assert_eq!(returned_body, expected_body);
    assert_eq!(assets.len(), 1);
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
fn resolve_gemini_canvas_image_edit_signaler_bootstrap_material_prefers_page_account_and_merges_keys(
) {
    let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
    let storage_state = json!({
        "signalerAccountId": "runtime-account-id",
        "apiKey": "AIzaStorageKey123456789012345",
    });
    let page_body = r#"
        <script>
          window.WIZ_global_data={"S06Grb":"page-account-id"};
          window.firebaseConfig={"apiKey":"AIzaPageKey123456789012345"};
        </script>
    "#;

    let material = resolve_gemini_canvas_image_edit_signaler_bootstrap_material(
        &payload,
        &storage_state,
        page_body,
    )
    .expect("bootstrap material");

    assert_eq!(material.account_id, "page-account-id");
    assert!(material
        .api_key_candidates
        .iter()
        .any(|candidate| candidate == "sk-test"));
    assert!(material
        .api_key_candidates
        .iter()
        .any(|candidate| candidate == "AIzaPageKey123456789012345"));
}
