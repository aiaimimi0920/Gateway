use super::followup_test_support::make_payload;
use super::*;
use serde_json::json;
use std::collections::HashMap;

fn make_gemini_canvas_program_payload_with_handle(
    operation: Option<&str>,
) -> ProviderAccountPayload {
    let mut payload = make_payload(
        "gemini_canvas_program_web_reverse_compatible",
        "https://gemini.google.com",
    );
    payload.runtime_state_object_key =
        Some("credential-runtime/gemini-canvas/program/storage-state.json".to_string());
    let mut extra = HashMap::from([
        ("shareId".to_string(), json!("canvas-share-789")),
        ("appPath".to_string(), json!("/app/4abc4e7577b6149f")),
        ("conversationId".to_string(), json!("c_4abc4e7577b6149f")),
        ("responseId".to_string(), json!("r_payload")),
    ]);
    if let Some(operation) = operation {
        extra.insert("canvasProgramOperation".to_string(), json!(operation));
    }
    payload.extra_body = Some(extra);
    payload
}

#[test]
fn gemini_canvas_followup_target_bootstrap_and_recovery_helpers_preserve_contract() {
    let mut target = GeminiCanvasFollowupTarget::from_bootstrap(
        false,
        Some("/app/bootstrap-handle"),
        Some(gemini_canvas::GeminiCanvasStreamGenerateLocator {
            response_id: "r_bootstrap".to_string(),
            conversation_id: "c_bootstrap".to_string(),
            app_path: "/app/locator-handle".to_string(),
        }),
        GeminiCanvasPageTargetMode::Resolved,
    );
    assert_eq!(target.source_path, "/app/bootstrap-handle");
    assert_eq!(target.mode, GeminiCanvasPageTargetMode::Resolved);
    assert_eq!(target.response_id(), "r_bootstrap");
    assert_eq!(target.conversation_id(), "c_bootstrap");

    target.adopt_video_recovered_locator(gemini_canvas::GeminiCanvasStreamGenerateLocator {
        response_id: "r_recovered".to_string(),
        conversation_id: "c_recovered".to_string(),
        app_path: "/app/recovered-handle".to_string(),
    });
    assert_eq!(target.source_path, "/app/recovered-handle");
    assert_eq!(
        target.mode,
        GeminiCanvasPageTargetMode::ConversationListRecovered
    );
    assert_eq!(target.response_id(), "r_recovered");
    assert_eq!(target.conversation_id(), "c_recovered");
}

#[test]
fn resolve_gemini_canvas_followup_locator_uses_payload_handle_for_image() {
    let payload = make_gemini_canvas_program_payload_with_handle(Some("image"));

    let locator = resolve_gemini_canvas_followup_locator(
        &payload,
        gemini_canvas::GeminiCanvasMediaOperation::Image,
        "body without locator",
        "gemini_canvas_compatible",
        "test",
    )
    .expect("locator resolution")
    .expect("payload locator");

    assert_eq!(locator.app_path, "/app/4abc4e7577b6149f");
    assert_eq!(locator.conversation_id, "c_4abc4e7577b6149f");
    assert_eq!(locator.response_id, "r_payload");
}

#[test]
fn resolve_gemini_canvas_followup_locator_keeps_music_on_stream_locator_contract() {
    let payload = make_gemini_canvas_program_payload_with_handle(Some("image"));

    let locator = resolve_gemini_canvas_followup_locator(
        &payload,
        gemini_canvas::GeminiCanvasMediaOperation::Music,
        "body without locator",
        "gemini_canvas_compatible",
        "test",
    )
    .expect("music should gracefully fall back when payload handle modality mismatches");

    assert!(locator.is_none());
}

#[test]
fn prepare_gemini_canvas_page_seed_prefers_override_without_locator() {
    let payload = make_payload(
        "gemini_canvas_program_web_reverse_compatible",
        "https://gemini.google.com",
    );

    let runtime = gemini_canvas::GeminiCanvasRuntime {
        runtime_state_object_key: "credential-runtime/gemini-canvas/program/storage-state.json"
            .to_string(),
        share_id: "canvas-share-789".to_string(),
        api_base_url: "https://gemini.google.com".to_string(),
    };

    let seed = prepare_gemini_canvas_page_seed(
        &payload,
        &runtime,
        gemini_canvas::GeminiCanvasMediaOperation::Image,
        "body without locator",
        false,
        Some(" https://gemini.google.com/app/4abc4e7577b6149f "),
        true,
        "gemini_canvas_compatible",
        "test",
    )
    .expect("page seed");

    assert!(seed.locator.is_none());
    assert!(seed.prefer_root_app_path);
    assert_eq!(
        seed.conversation_page_url.as_deref(),
        Some("https://gemini.google.com/app/4abc4e7577b6149f")
    );
    assert_eq!(seed.app_bootstrap_url, "https://gemini.google.com/app");
    assert_eq!(
        seed.share_bootstrap_url,
        "https://gemini.google.com/share/canvas-share-789"
    );
    assert!(seed.has_page_url_override);
}

#[test]
fn build_gemini_canvas_followup_bootstrap_candidates_prefers_app_then_concrete_then_share() {
    let candidates = build_gemini_canvas_followup_bootstrap_candidates(
        false,
        Some("https://gemini.google.com/app/4abc4e7577b6149f"),
        "https://gemini.google.com/app",
        "https://gemini.google.com/share/canvas-share-789",
    );

    assert_eq!(
        candidates,
        vec![
            "https://gemini.google.com/app".to_string(),
            "https://gemini.google.com/app/4abc4e7577b6149f".to_string(),
            "https://gemini.google.com/share/canvas-share-789".to_string()
        ]
    );
}

#[test]
fn build_gemini_canvas_page_poll_urls_skips_concrete_page_for_forced_root_without_override() {
    let candidates = build_gemini_canvas_page_poll_urls(
        true,
        false,
        Some("https://gemini.google.com/app/4abc4e7577b6149f"),
        "https://gemini.google.com/app",
        "https://gemini.google.com/share/canvas-share-789",
    );

    assert_eq!(
        candidates,
        vec![
            "https://gemini.google.com/app".to_string(),
            "https://gemini.google.com/share/canvas-share-789".to_string()
        ]
    );
}

#[test]
fn gemini_canvas_followup_target_adopts_bootstrap_path_and_recovered_locator() {
    let mut target = GeminiCanvasFollowupTarget::from_bootstrap(
        false,
        Some("/app/from-bootstrap"),
        None,
        GeminiCanvasPageTargetMode::RootAppFallback,
    );
    assert_eq!(target.source_path, "/app/from-bootstrap");
    assert_eq!(target.mode, GeminiCanvasPageTargetMode::RootAppFallback);
    assert_eq!(target.response_id(), "<none>");

    target.adopt_video_recovered_locator(gemini_canvas::GeminiCanvasStreamGenerateLocator {
        response_id: "r_recovered".to_string(),
        conversation_id: "c_recovered".to_string(),
        app_path: "/app/recovered".to_string(),
    });
    assert_eq!(target.source_path, "/app/recovered");
    assert_eq!(
        target.mode,
        GeminiCanvasPageTargetMode::ConversationListRecovered
    );
    assert_eq!(target.response_id(), "r_recovered");
    assert_eq!(target.conversation_id(), "c_recovered");
}

#[test]
fn classify_gemini_canvas_page_target_mode_covers_lane_variants() {
    assert_eq!(
        classify_gemini_canvas_page_target_mode(true, false, false, false),
        GeminiCanvasPageTargetMode::ForcedRootApp
    );
    assert_eq!(
        classify_gemini_canvas_page_target_mode(false, true, true, false),
        GeminiCanvasPageTargetMode::SignalerPageBootstrap
    );
    assert_eq!(
        classify_gemini_canvas_page_target_mode(false, false, true, true),
        GeminiCanvasPageTargetMode::ConversationListRecovered
    );
    assert_eq!(
        classify_gemini_canvas_page_target_mode(false, false, true, false),
        GeminiCanvasPageTargetMode::Resolved
    );
    assert_eq!(
        classify_gemini_canvas_page_target_mode(false, false, false, false),
        GeminiCanvasPageTargetMode::RootAppFallback
    );
}
