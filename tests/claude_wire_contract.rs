//! Captured Claude 2.1.278 protocol contracts, without live credentials.
use neuro_gateway::protocol::{anthropic, claude_wire};
use neuro_gateway::routing::candidate::ProviderAccountPayload;
use neuro_gateway::upstream::{client::UpstreamClient, headers::build_inference_headers};
use serde_json::json;
use std::collections::HashMap;

fn payload() -> ProviderAccountPayload {
    serde_json::from_value(
        json!({"adapter":"anthropic_compatible","base_url":"https://example.invalid",
        "api_key":"selected-fixture","request_client_profile":"claude_cli"}),
    )
    .unwrap()
}

#[test]
fn claude_native_system_messages_and_unknown_blocks_survive() {
    let body = json!({"model":"claude-fixture","max_tokens":128,"stream":true,"tools":[],
        "system":[{"type":"text","text":"business system","cache_control":{"type":"ephemeral"}}],
        "messages":[{"role":"user","content":"hi"},{"role":"system","content":[
            {"type":"text","text":"date context","cache_control":{"type":"ephemeral"},"future":17}]}],
        "metadata":{"user_id":"caller-owned"},"future_top":{"x":1}});
    let req = anthropic::normalize_messages(body.clone()).unwrap();
    let plan =
        UpstreamClient::build_request_plan(&payload(), &req, "claude-fixture", true).unwrap();
    assert_eq!(plan.body.unwrap(), body);
    let mut changed = req;
    changed.messages.clear();
    let body = UpstreamClient::build_request_plan(&payload(), &changed, "mapped-model", true)
        .unwrap()
        .body
        .unwrap();
    assert_eq!(body["model"], "mapped-model");
    assert_eq!(body["messages"], json!([]));
}

#[test]
fn claude_system_cannot_smuggle_tool_results_or_unknown_roles() {
    for message in [
        json!({"role":"system","content":[{"type":"tool_use","id":"id","name":"run","input":{}}]}),
        json!({"role":"developer","content":"x"}),
        json!({"role":"system","content":[{"type":"tool_result","tool_use_id":"id","content":"x"}]}),
    ] {
        assert!(anthropic::normalize_messages(
            json!({"model":"claude-fixture","messages":[message]})
        )
        .is_err());
    }
}

#[test]
fn claude_inline_system_is_not_duplicated_at_top_level() {
    let body = json!({"model":"claude-fixture","max_tokens":128,"stream":true,
        "messages":[{"role":"system","content":[{"type":"text","text":"inline",
            "cache_control":{"type":"ephemeral"}}]},{"role":"user","content":"hi"}]});
    let req = anthropic::normalize_messages(body.clone()).unwrap();
    let planned = UpstreamClient::build_request_plan(&payload(), &req, "claude-fixture", true)
        .unwrap()
        .body
        .unwrap();
    assert_eq!(planned, body);
    assert!(planned.get("system").is_none());
}

#[test]
fn claude_native_identity_beta_accept_and_auth_are_preserved() {
    let req = anthropic::normalize_messages(
        json!({"model":"claude-fixture","messages":[],"stream":true}),
    )
    .unwrap();
    let extra = HashMap::from([
        ("user-agent".into(), claude_wire::USER_AGENT.into()),
        ("x-app".into(), "cli".into()),
        ("x-stainless-os".into(), "Windows".into()),
        ("x-stainless-arch".into(), "x64".into()),
        ("x-stainless-retry-count".into(), "2".into()),
        ("accept".into(), "application/json".into()),
        ("anthropic-beta".into(), "caller-beta".into()),
        ("anthropic-version".into(), "2023-06-01".into()),
        ("x-claude-code-session-id".into(), "session-fixture".into()),
        ("x-api-key".into(), "untrusted-client-key".into()),
    ]);
    let h = build_inference_headers(&payload(), &req, "claude-fixture", Some(&extra));
    for (key, value) in &extra {
        if key != "x-api-key" {
            assert_eq!(h[key].to_str().unwrap(), value);
        }
    }
    assert_eq!(h["x-api-key"], "selected-fixture");
    assert!(!h.contains_key("originator"));
}

#[test]
fn claude_sdk_defaults_describe_platform_without_fake_session() {
    let req = anthropic::normalize_messages(
        json!({"model":"claude-fixture","messages":[],"stream":true}),
    )
    .unwrap();
    let extra = HashMap::from([
        ("user-agent".into(), "private-sdk".into()),
        ("x-stainless-os".into(), "private-device".into()),
    ]);
    let h = build_inference_headers(&payload(), &req, "claude-fixture", Some(&extra));
    assert_eq!(h["user-agent"], claude_wire::USER_AGENT);
    assert_eq!(h["x-stainless-os"], claude_wire::platform());
    assert_eq!(h["x-stainless-arch"], claude_wire::architecture());
    assert_eq!(h["accept"], "application/json");
    assert!(!h.contains_key("x-claude-code-session-id"));
}

#[test]
fn claude_query_keeps_beta_encoding_without_exporting_credentials() {
    for value in [
        "beta=true",
        "beta=false",
        "beta=claude-code-20250219",
        "beta=%74rue",
    ] {
        assert_eq!(claude_wire::query(value).unwrap(), value);
    }
    for value in [
        "api_key=secret",
        "beta=true&access_token=secret",
        "beta=true&beta=false",
        "beta=",
    ] {
        assert!(claude_wire::query(value).is_err());
    }
}
