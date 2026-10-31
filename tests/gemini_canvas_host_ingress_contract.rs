//! The public route selector must admit implemented host operations, not legacy program edits.
use neuro_gateway::{
    protocol::{canonical::EndpointKind, gemini_business, registry::*},
    routing::{
        candidate::{ProviderAccountPayload, ProviderExecutionMode, RouteCandidate},
        protocol_resolution::finalize_candidate_protocol_family,
    },
};
use serde_json::json;

#[path = "gemini_canvas_host_ingress_contract/documents.rs"]
mod documents;
#[path = "console_contract_support/mod.rs"]
mod support;
#[path = "gemini_canvas_host_ingress_contract/transcription_http.rs"]
mod transcription_http;
#[path = "gemini_canvas_host_ingress_contract/transcription_transport.rs"]
mod transcription_transport;

fn candidate(adapter: &str) -> RouteCandidate {
    let payload: ProviderAccountPayload = serde_json::from_value(json!({
        "adapter":adapter,"base_url":"https://gemini.google.com","api_key":"unused-fixture",
        "runtime_state_object_key":"fixture/state.json"
    }))
    .unwrap();
    RouteCandidate {
        provider_account_id: "fixture-provider".into(),
        provider_credential_id: Some("fixture-credential".into()),
        label: "fixture".into(),
        payload,
        protocol_family: GEMINI_CANVAS_MUSIC_FAMILY.into(),
        protocol_profile: "gemini_canvas_web_reverse_modular".into(),
        supported_protocol_families: vec![
            GEMINI_GENERATE_CONTENT_FAMILY.into(),
            GEMINI_CANVAS_IMAGES_FAMILY.into(),
            GEMINI_CANVAS_MUSIC_FAMILY.into(),
            GEMINI_CANVAS_VIDEOS_FAMILY.into(),
        ],
        adapter: adapter.into(),
        model_alias: None,
        upstream_model: Some("gemini-2.5-flash-image".into()),
        resolved_execution_mode: ProviderExecutionMode::BrowserBacked,
        priority: 0,
        weight: 1,
        failure_count: 0,
        cooldown_until: None,
        routing_score: None,
        routing_health_weight: None,
        routing_capacity_weight: None,
        routing_degraded: None,
        routing_breaker_open: None,
        routing_degradation_reasons: vec![],
    }
}

#[test]
fn browser_host_image_edits_survive_public_candidate_selection() {
    let req = gemini_business::normalize_image_edits(json!({
        "model":"gemini-2.5-flash-image","prompt":"make it red",
        "images":[{"mime_type":"image/png","base64":"aGVsbG8="}]
    }))
    .unwrap();
    let mut selected = candidate("gemini_canvas_web_reverse_compatible");
    assert_eq!(
        finalize_candidate_protocol_family(&mut selected, &req),
        Some(false)
    );
    assert_eq!(selected.protocol_family, GEMINI_CANVAS_IMAGES_FAMILY);
    assert_eq!(
        selected.provider_credential_id.as_deref(),
        Some("fixture-credential")
    );
}

#[test]
fn legacy_program_edits_remain_unimplemented() {
    let mut req = gemini_business::normalize_image_generations(json!({"prompt":"test"})).unwrap();
    req.endpoint_kind = EndpointKind::ImagesEdits;
    let mut selected = candidate("gemini_canvas_program_web_reverse_compatible");
    assert_eq!(
        finalize_candidate_protocol_family(&mut selected, &req),
        None
    );
}

#[test]
fn browser_host_existing_text_speech_and_image_surfaces_remain_routable() {
    for (endpoint, family) in [
        (
            EndpointKind::ChatCompletions,
            GEMINI_GENERATE_CONTENT_FAMILY,
        ),
        (EndpointKind::AudioSpeech, GEMINI_GENERATE_CONTENT_FAMILY),
        (EndpointKind::ImagesGenerations, GEMINI_CANVAS_IMAGES_FAMILY),
    ] {
        let mut req =
            gemini_business::normalize_image_generations(json!({"prompt":"test"})).unwrap();
        req.endpoint_kind = endpoint;
        let mut selected = candidate("gemini_canvas_web_reverse_compatible");
        assert!(finalize_candidate_protocol_family(&mut selected, &req).is_some());
        assert_eq!(selected.protocol_family, family);
    }
}

#[test]
fn browser_host_transcription_routes_to_generate_content_only() {
    let req = neuro_gateway::protocol::openai::normalize_audio_transcriptions(json!({
        "model":"gemini-3-flash-preview", "file":{"mime_type":"audio/mpeg","base64":"SUQz"}
    }))
    .unwrap();
    for adapter in [
        "gemini_canvas_web_reverse_compatible",
        "gemini_canvas_program_web_reverse_compatible",
        "gemini_api_compatible",
        "gemini_canvas_compatible",
    ] {
        let mut selected = candidate(adapter);
        let result = finalize_candidate_protocol_family(&mut selected, &req);
        if adapter == "gemini_canvas_web_reverse_compatible" {
            assert!(
                result.is_some(),
                "implemented Canvas transcription must be routable"
            );
            assert_eq!(selected.protocol_family, GEMINI_GENERATE_CONTENT_FAMILY);
        } else {
            assert_eq!(
                result, None,
                "unimplemented adapter {adapter} must remain rejected"
            );
        }
    }
}

#[test]
fn ordinary_web_transcription_has_its_own_public_route() {
    let req = neuro_gateway::protocol::openai::normalize_audio_transcriptions(json!({
        "model":"gemini-3-flash-preview", "file":{"mime_type":"audio/mpeg","base64":"SUQz"}
    }))
    .unwrap();
    for adapter in [
        "gemini_web_compatible",
        "gemini_web_reverse_modular_compatible",
    ] {
        let mut selected = candidate(adapter);
        selected.protocol_family = GEMINI_WEB_CHAT_FAMILY.into();
        selected
            .supported_protocol_families
            .push(GEMINI_WEB_CHAT_FAMILY.into());
        assert!(finalize_candidate_protocol_family(&mut selected, &req).is_some());
        assert_eq!(selected.protocol_family, GEMINI_WEB_CHAT_FAMILY);
    }
}
