//! Default Web media must use the guarded Web owner without migrating stored accounts.
use neuro_gateway::{
    protocol::{
        call_flow::{prepare, ProviderCallFlow, SpecialAdapter},
        canonical::{CanonicalRelayRequest, EndpointKind, ProtocolFamily},
        registry::*,
    },
    routing::{
        candidate::{ProviderAccountPayload, ProviderExecutionMode, RouteCandidate},
        protocol_resolution::finalize_candidate_protocol_family,
    },
    upstream::{client::UpstreamClient, gemini::web_reverse as web},
};
use serde_json::json;

#[path = "gemini_web_media_ingress_contract/http.rs"]
mod http;
#[path = "console_contract_support/mod.rs"]
mod support;

fn payload() -> ProviderAccountPayload {
    serde_json::from_value(json!({
        "adapter":"gemini_web_compatible", "base_url":"https://gemini.google.com",
        "api_key":"synthetic-cookie", "credential_id":"fixture-credential",
        "runtime_state_object_key":"synthetic-not-read",
        "call_flow":{"special_adapter":"gemini_web","fingerprint_adapter":"session"}
    }))
    .unwrap()
}

fn request(endpoint: EndpointKind) -> CanonicalRelayRequest {
    serde_json::from_value(json!({
        "protocol_family":ProtocolFamily::OpenAi, "endpoint_kind":endpoint,
        "messages":[], "tools":[], "stream":false,
        "raw_body":{"prompt":"fixture", "n":1}
    }))
    .unwrap()
}

fn candidate() -> RouteCandidate {
    RouteCandidate {
        provider_account_id: "fixture-provider".into(),
        provider_credential_id: Some("fixture-credential".into()),
        label: "fixture".into(),
        payload: payload(),
        protocol_family: GEMINI_WEB_CHAT_FAMILY.into(),
        protocol_profile: "gemini_web".into(),
        supported_protocol_families: vec![],
        adapter: "gemini_web_compatible".into(),
        model_alias: None,
        upstream_model: Some("fixture".into()),
        resolved_execution_mode: ProviderExecutionMode::DirectHttp,
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
fn default_web_media_selector_admits_only_implemented_operations() {
    for (endpoint, family) in [
        (EndpointKind::ImagesGenerations, GEMINI_CANVAS_IMAGES_FAMILY),
        (EndpointKind::ImagesEdits, GEMINI_CANVAS_IMAGES_FAMILY),
        (EndpointKind::MusicGenerations, GEMINI_CANVAS_MUSIC_FAMILY),
        (EndpointKind::VideosGenerations, GEMINI_CANVAS_VIDEOS_FAMILY),
        (EndpointKind::ChatCompletions, GEMINI_WEB_CHAT_FAMILY),
        (EndpointKind::AudioTranscriptions, GEMINI_WEB_CHAT_FAMILY),
        (EndpointKind::AudioSpeech, OPENAI_AUDIO_SPEECH_FAMILY),
    ] {
        let mut selected = candidate();
        assert!(
            finalize_candidate_protocol_family(&mut selected, &request(endpoint)).is_some(),
            "{endpoint:?}"
        );
        assert_eq!(selected.protocol_family, family);
        assert_eq!(selected.payload.adapter, "gemini_web_compatible");
        assert_eq!(
            selected.provider_credential_id.as_deref(),
            Some("fixture-credential")
        );
    }
    for endpoint in [EndpointKind::Embeddings] {
        assert_eq!(
            finalize_candidate_protocol_family(&mut candidate(), &request(endpoint)),
            None
        );
    }
    let mut restricted = candidate();
    restricted.supported_protocol_families = vec![GEMINI_WEB_CHAT_FAMILY.into()];
    assert_eq!(
        finalize_candidate_protocol_family(
            &mut restricted,
            &request(EndpointKind::ImagesGenerations)
        ),
        None
    );
    assert_eq!(
        finalize_candidate_protocol_family(&mut restricted, &request(EndpointKind::AudioSpeech)),
        None,
        "adding speech support must not override explicit protocol restrictions"
    );
}

#[test]
fn default_web_media_bridge_is_request_local_and_preserves_identity_and_policy() {
    for key in ["pureHttpMode", "pure_http_mode"] {
        for mode in ["required", "disabled"] {
            let mut original = payload();
            original.extra_body = Some([(key.into(), json!(mode))].into());
            let before = serde_json::to_value(&original).unwrap();
            for endpoint in [
                EndpointKind::ImagesGenerations,
                EndpointKind::ImagesEdits,
                EndpointKind::MusicGenerations,
                EndpointKind::VideosGenerations,
            ] {
                let route = web::legacy_mixed_lane_execution_route(&original, endpoint, false)
                    .expect("default media owner");
                assert_eq!(route.kind, web::LegacyMixedLaneExecutionKind::Media);
                assert_eq!(
                    route.payload.adapter,
                    "gemini_web_reverse_modular_compatible"
                );
                assert_eq!(route.payload.extra_body.as_ref().unwrap()[key], mode);
                assert!(
                    prepare(&route.payload).is_ok(),
                    "the normalized owner must have a valid call flow"
                );
                let mut projected = serde_json::to_value(route.payload).unwrap();
                // These are the only compatibility projections; credentials and paths stay intact.
                for field in ["adapter", "call_flow", "execution_mode"] {
                    projected[field] = before[field].clone();
                }
                assert_eq!(projected, before);
            }
            assert_eq!(serde_json::to_value(&original).unwrap(), before);
            assert!(web::legacy_mixed_lane_execution_route(
                &original,
                EndpointKind::ChatCompletions,
                false
            )
            .is_none());
            assert!(web::legacy_mixed_lane_execution_route(
                &original,
                EndpointKind::AudioSpeech,
                false
            )
            .is_none());
        }
    }
}

#[tokio::test]
async fn default_web_media_dispatch_reaches_policy_owner_before_network() {
    let client = UpstreamClient::new(1);
    for (endpoint, mode) in [
        (EndpointKind::MusicGenerations, "required"),
        (EndpointKind::VideosGenerations, "required"),
        (EndpointKind::ImagesEdits, "disabled"),
    ] {
        let mut original = payload();
        original.base_url = "http://127.0.0.1:1".into();
        original.extra_body = Some([("pureHttpMode".into(), json!(mode))].into());
        let error = client
            .execute_json_passthrough(
                "fixture-provider",
                ProviderExecutionMode::DirectHttp,
                &original,
                &request(endpoint),
                "fixture",
                None,
            )
            .await
            .unwrap_err();
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_web_media_execution_policy_unsupported")
        );
        assert!(!error.retryable);
        assert!(matches!(
            error.fallback_hint,
            neuro_gateway::error::FallbackHint::Abort { .. }
        ));
    }
}

#[tokio::test]
async fn default_web_media_does_not_bypass_invalid_call_flow_or_other_surfaces() {
    let mut invalid = payload();
    invalid.call_flow = Some(ProviderCallFlow {
        special_adapter: SpecialAdapter::GeminiCanvas,
        ..Default::default()
    });
    let error = UpstreamClient::new(1)
        .execute_json_passthrough(
            "fixture",
            ProviderExecutionMode::DirectHttp,
            &invalid,
            &request(EndpointKind::ImagesGenerations),
            "fixture",
            None,
        )
        .await
        .unwrap_err();
    assert!(error.message.starts_with("Invalid call_flow:"));
    assert!(web::legacy_mixed_lane_execution_route(
        &invalid,
        EndpointKind::ImagesGenerations,
        false
    )
    .is_none());
    let mut canvas = payload();
    canvas.adapter = "gemini_canvas_web_reverse_compatible".into();
    assert!(web::legacy_mixed_lane_execution_route(
        &canvas,
        EndpointKind::ImagesGenerations,
        false
    )
    .is_none());
}
