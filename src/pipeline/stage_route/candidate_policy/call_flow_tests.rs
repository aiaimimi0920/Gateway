//! Projection/quota rows remain paired when policy rejects or reprioritizes a flow.
use super::*;
use crate::protocol::{anthropic::normalize_messages, canonical::EndpointKind};
use crate::routing::candidate::ProviderExecutionMode;
use serde_json::{json, Value};

fn candidate(id: &str, adapter: &str, flow: Option<Value>) -> RouteCandidate {
    RouteCandidate {
        provider_account_id: id.into(),
        provider_credential_id: None,
        label: id.into(),
        payload: serde_json::from_value(
            json!({"adapter":adapter,"base_url":"https://fixture.test",
            "api_key":"fixture","call_flow":flow}),
        )
        .unwrap(),
        adapter: adapter.into(),
        protocol_family: "openai_chat".into(),
        protocol_profile: "openai_compatible_generic".into(),
        supported_protocol_families: vec![if id == "fixed" {
            "anthropic_messages"
        } else {
            "openai_chat"
        }
        .into()],
        model_alias: Some("fixture".into()),
        upstream_model: Some("fixture".into()),
        resolved_execution_mode: ProviderExecutionMode::DirectHttp,
        priority: 1,
        weight: 1,
        failure_count: 0,
        cooldown_until: None,
        routing_score: None,
        routing_health_weight: None,
        routing_capacity_weight: None,
        routing_degraded: None,
        routing_breaker_open: None,
        routing_degradation_reasons: Vec::new(),
    }
}

#[test]
fn call_flow_candidate_pairs_preserve_projection_alignment_and_route_policy() {
    let req = normalize_messages(
        json!({"model":"fixture","max_tokens":8,"messages":[{"role":"user","content":"hi"}]}),
    )
    .unwrap();
    let candidates = vec![
        candidate("legacy", "openai_compatible", None),
        candidate(
            "invalid",
            "freebuff_compatible",
            Some(json!({"special_adapter":"none"})),
        ),
        candidate(
            "fixed",
            "openai_compatible",
            Some(json!({"call_adapter":"anthropic","special_adapter":"none"})),
        ),
    ];
    let (prepared, rows) =
        finalize_candidate_pairs_for_request(&req, candidates.clone(), Some(vec![1, 2, 3]), None);
    assert_eq!(rows.unwrap(), [3, 1]);
    assert_eq!(
        prepared
            .iter()
            .map(|c| c.provider_account_id.as_str())
            .collect::<Vec<_>>(),
        ["fixed", "legacy"]
    );
    let policy = crate::db::GatewayRoutePolicyConfig {
        allowed_protocol_families: Some(vec!["anthropic_messages".into()]),
        ..Default::default()
    };
    let allowed = filter_candidates_by_route_policy_family(prepared, Some(&policy));
    assert_eq!(allowed.len(), 1);
    assert_eq!(allowed[0].adapter, "anthropic_compatible");
    let (_, rows) = finalize_candidate_pairs_for_request(
        &req,
        candidates,
        Some(vec![1, 2, 3]),
        Some("openai_chat"),
    );
    assert_eq!(rows.unwrap(), [1]);
}

#[test]
fn call_flow_send_admission_rejects_unprepared_or_incompatible_candidates() {
    let mut fixed = candidate(
        "fixed",
        "openai_compatible",
        Some(json!({"call_adapter":"anthropic","special_adapter":"none"})),
    );
    assert!(crate::routing::call_flow::validate_prepared_candidate(&fixed).is_err());
    crate::routing::call_flow::prepare_candidate(&mut fixed, EndpointKind::Responses).unwrap();
    crate::routing::call_flow::validate_prepared_candidate(&fixed).unwrap();
    let invalid = candidate(
        "invalid",
        "freebuff_compatible",
        Some(json!({"special_adapter":"none"})),
    );
    assert!(
        crate::routing::call_flow::validate_prepared_candidate(&invalid)
            .unwrap_err()
            .message
            .starts_with("Invalid call_flow:")
    );
}
