use super::followup_test_support::{make_gemini_bootstrap, make_payload, make_request};
use super::*;
use crate::protocol::canonical::ProtocolFamily;
use serde_json::json;

fn make_image_asset(url: &str) -> gemini_canvas::GeminiCanvasMediaAsset {
    gemini_canvas::GeminiCanvasMediaAsset {
        kind: "image".to_string(),
        url: url.to_string(),
        mime_type: "image/png".to_string(),
        download_token: None,
        body_base64: None,
        alt: None,
        width: Some(1024),
        height: Some(1024),
        duration_seconds: None,
    }
}

#[test]
fn gemini_canvas_media_followup_preflight_strategy_labels_match_contract() {
    assert_eq!(
        GeminiCanvasMediaFollowupPreflightStrategy::ParityThenLegacy.as_str(),
        "parity_then_legacy"
    );
    assert_eq!(
        GeminiCanvasMediaFollowupPreflightStrategy::LegacyOnly.as_str(),
        "legacy_only"
    );
}

#[tokio::test]
async fn prepare_gemini_canvas_direct_http_image_context_builds_edit_followup_state() {
    let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
    let mut req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesEdits);
    req.raw_body = json!({
        "image": "data:image/png;base64,aGVsbG8="
    });

    let context = prepare_gemini_canvas_direct_http_image_context(&payload, &req, "prompt text")
        .await
        .expect("image edit context");

    assert_eq!(
        context.mode_index,
        gemini_canvas::stream_generate_mode_index(gemini_canvas::GeminiCanvasMediaOperation::Image)
    );
    assert!(context.image_edit_uploads.is_some());
    assert!(context.image_edit_followup_context.is_some());
    assert_eq!(
        context
            .image_edit_followup_context
            .as_ref()
            .map(|value| value.prompt.as_str()),
        Some("prompt text")
    );
    assert!(context.initial_stream_allows_replay_template);
}

#[test]
fn build_gemini_canvas_media_followup_preflight_plan_preserves_mode_and_source_path() {
    let bootstrap = make_gemini_bootstrap();
    let plan = build_gemini_canvas_media_followup_preflight_plan(
        &bootstrap,
        "/app/4abc4e7577b6149f",
        gemini_canvas::GeminiCanvasMediaOperation::Music,
    )
    .expect("preflight plan");

    assert_eq!(
        plan.mode_index,
        gemini_canvas::stream_generate_mode_index(gemini_canvas::GeminiCanvasMediaOperation::Music)
    );
    assert!(plan
        .followup_model_header
        .contains(gemini_canvas::GEMINI_CANVAS_TEXT_SELECTED_MODEL_HEADER_ID));
    assert!(plan
        .activity_request
        .query
        .iter()
        .any(|(key, value)| key == "source-path" && value == "/app/4abc4e7577b6149f"));
    assert!(plan
        .followup_request
        .query
        .iter()
        .any(|(key, value)| key == "rpcids"
            && value == gemini_canvas::GEMINI_CANVAS_TEXT_BOOTSTRAP_RPCID));
}

#[test]
fn plan_gemini_canvas_image_response_returns_url_body_when_assets_are_caller_usable() {
    let mut req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesGenerations);
    req.raw_body = json!({
        "response_format": "url",
        "n": 2
    });
    let assets = vec![
        make_image_asset("https://example.com/a.png"),
        make_image_asset("https://example.com/b.png"),
    ];

    let result = plan_gemini_canvas_image_response(
        &req,
        "neon city",
        &assets,
        "gemini_canvas_compatible",
        "missing image asset",
        "gemini_canvas_no_image_asset",
    )
    .expect("planned image response");

    match result {
        GeminiCanvasImageResponsePlan::FinalResponse(body) => {
            let data = body
                .get("data")
                .and_then(Value::as_array)
                .expect("url response data");
            assert_eq!(data.len(), 2);
            assert_eq!(
                data[0].get("url").and_then(Value::as_str),
                Some("https://example.com/a.png")
            );
        }
        GeminiCanvasImageResponsePlan::Materialize(_) => {
            panic!("expected direct url response plan");
        }
    }
}

#[test]
fn plan_gemini_canvas_image_response_materializes_requested_asset_count() {
    let mut req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesGenerations);
    req.raw_body = json!({
        "response_format": "b64_json",
        "n": 1
    });
    let assets = vec![
        make_image_asset("blob:https://gemini.google.com/example-1"),
        make_image_asset("https://example.com/b.png"),
    ];

    let result = plan_gemini_canvas_image_response(
        &req,
        "neon city",
        &assets,
        "gemini_canvas_compatible",
        "missing image asset",
        "gemini_canvas_no_image_asset",
    )
    .expect("planned image response");

    match result {
        GeminiCanvasImageResponsePlan::Materialize(assets) => {
            assert_eq!(assets.len(), 1);
            assert_eq!(assets[0].url, "blob:https://gemini.google.com/example-1");
        }
        GeminiCanvasImageResponsePlan::FinalResponse(_) => {
            panic!("expected materialize response plan");
        }
    }
}

#[tokio::test]
async fn prepare_gemini_canvas_direct_http_image_context_skips_edit_state_for_generations() {
    let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
    let req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesGenerations);

    let context = prepare_gemini_canvas_direct_http_image_context(&payload, &req, "prompt text")
        .await
        .expect("image generation context");

    assert_eq!(
        context.mode_index,
        gemini_canvas::stream_generate_mode_index(gemini_canvas::GeminiCanvasMediaOperation::Image)
    );
    assert!(context.image_edit_uploads.is_none());
    assert!(context.image_edit_followup_context.is_none());
    assert!(context.initial_stream_allows_replay_template);
}
