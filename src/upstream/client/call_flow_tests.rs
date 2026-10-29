//! Call-flow regressions never contact a live provider.
use super::*;
use serde_json::json;

fn payload(flow: Value) -> ProviderAccountPayload {
    serde_json::from_value(json!({
        "adapter":"openai_compatible", "base_url":"https://fixture.test",
        "api_key":"fixture-secret", "call_flow":flow
    }))
    .unwrap()
}

#[test]
fn call_flow_selects_format_before_building_request() {
    let payload = payload(
        json!({"call_adapter":"anthropic", "fingerprint_adapter":"claude_cli", "special_adapter":"none"}),
    );
    let req = responses::normalize_responses(json!({"model":"fixture", "input":"hello"})).unwrap();
    let plan = UpstreamClient::build_request_plan(&payload, &req, "fixture", false).unwrap();
    assert_eq!(plan.response_kind, EndpointKind::Messages);
    assert!(plan.url.ends_with("/v1/messages"));
}
