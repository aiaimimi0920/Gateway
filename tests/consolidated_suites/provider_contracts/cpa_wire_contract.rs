//! Measured CPA HTTP envelope; credential selection remains independent of profile.
use neuro_gateway::{
    protocol::anthropic::normalize_messages,
    routing::candidate::ProviderAccountPayload,
    upstream::{client::UpstreamClient, headers::build_inference_headers},
};
use serde_json::json;
use std::collections::HashMap;

fn payload(profile: &str) -> ProviderAccountPayload {
    serde_json::from_value(
        json!({"adapter":"anthropic_compatible","base_url":"https://relay.example",
        "api_key":"selected-fixture","request_client_profile":profile}),
    )
    .unwrap()
}

#[test]
fn cpa_adds_measured_beta_query_without_overriding_explicit_query() {
    let req =
        normalize_messages(json!({"model":"claude-fixture","max_tokens":8,"messages":[]})).unwrap();
    let mut p = payload("cpa");
    let plan = UpstreamClient::build_request_plan(&p, &req, "claude-fixture", true).unwrap();
    assert_eq!(plan.query, vec![("beta".to_string(), "true".to_string())]);
    p.messages_path = Some("/v1/messages?beta=false".into());
    let plan = UpstreamClient::build_request_plan(&p, &req, "claude-fixture", true).unwrap();
    assert!(plan.query.is_empty());
    let plan =
        UpstreamClient::build_request_plan(&payload("claude_cli"), &req, "claude-fixture", true)
            .unwrap();
    assert!(plan.query.is_empty());
}

#[test]
fn cpa_does_not_expose_gateway_request_id_but_preserves_operator_and_auth() {
    let req =
        normalize_messages(json!({"model":"claude-fixture","max_tokens":8,"messages":[]})).unwrap();
    let mut p = payload("cpa");
    let extra = HashMap::from([("x-request-id".into(), "gateway-local-id".into())]);
    assert!(
        !build_inference_headers(&p, &req, "claude-fixture", Some(&extra))
            .contains_key("x-request-id")
    );
    p.headers
        .insert("x-request-id".into(), "operator-trace".into());
    assert_eq!(
        build_inference_headers(&p, &req, "claude-fixture", Some(&extra))["x-request-id"],
        "operator-trace"
    );
}

#[test]
fn anthropic_bearer_is_explicit_auth_configuration_not_a_style_side_effect() {
    let req =
        normalize_messages(json!({"model":"claude-fixture","max_tokens":8,"messages":[]})).unwrap();
    for profile in ["cpa", "claude_cli", "workbuddy"] {
        let mut p = payload(profile);
        let h = build_inference_headers(&p, &req, "claude-fixture", None);
        assert_eq!(h["x-api-key"], "selected-fixture");
        assert!(!h.contains_key("authorization"));
        p.auth_mode = Some("bearer".into());
        let h = build_inference_headers(&p, &req, "claude-fixture", None);
        assert_eq!(h["authorization"], "Bearer selected-fixture");
        assert!(!h.contains_key("x-api-key"));
    }
}
