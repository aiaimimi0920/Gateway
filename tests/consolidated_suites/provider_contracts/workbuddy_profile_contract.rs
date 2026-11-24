//! WorkBuddy identity must remain independent from protocol, credentials and session state.
use neuro_gateway::protocol::responses::normalize_responses;
use neuro_gateway::routing::candidate::ProviderAccountPayload;
use neuro_gateway::upstream::client::UpstreamClient;
use neuro_gateway::upstream::headers::build_inference_headers;
use serde_json::json;
use std::collections::HashMap;

fn payload(adapter: &str) -> ProviderAccountPayload {
    serde_json::from_value(json!({"adapter":adapter,"base_url":"https://relay.example",
        "api_key":"provider-fixture","request_client_profile":"workbuddy"}))
    .unwrap()
}

#[test]
fn workbuddy_identity_is_independent_of_model_protocol_and_inbound_client() {
    for model in ["gpt-fixture", "claude-fixture", "gemini-fixture"] {
        let req = normalize_responses(json!({"model":model,"input":"hello"})).unwrap();
        for adapter in ["openai_compatible", "anthropic_compatible"] {
            let extra = HashMap::from([
                (
                    "user-agent".into(),
                    "claude-cli/2.1.278 (external, cli)".into(),
                ),
                ("x-app".into(), "cli".into()),
                ("originator".into(), "codex_exec".into()),
                ("authorization".into(), "Bearer caller-secret".into()),
                ("x-stainless-package-version".into(), "stale".into()),
            ]);
            let h = build_inference_headers(&payload(adapter), &req, model, Some(&extra));
            assert_eq!(
                h["user-agent"],
                "WorkBuddy/5.8.4 workbuddy-ai/5.8.4 CLI/2.161.0"
            );
            assert_eq!(h["x-ide-type"], "WorkBuddy");
            assert_eq!(h["x-ide-name"], "WorkBuddy");
            assert_eq!(h["x-ide-version"], "5.8.4");
            assert_eq!(h["x-product"], "SaaS");
            assert_eq!(h["x-codebuddy-request"], "1");
            assert_eq!(h["x-stainless-package-version"], "6.25.0");
            for name in [
                "originator",
                "x-app",
                "x-user-id",
                "x-conversation-id",
                "x-private-data",
            ] {
                assert!(!h.contains_key(name), "unexpected identity/state: {name}");
            }
            if adapter == "openai_compatible" {
                assert_eq!(h["authorization"], "Bearer provider-fixture");
                assert!(!h.contains_key("x-api-key"));
            } else {
                assert_eq!(h["x-api-key"], "provider-fixture");
                assert!(!h.contains_key("authorization"));
            }
            assert!(!h.contains_key("anthropic-beta"));
        }
    }
}

#[test]
fn workbuddy_does_not_inject_codex_body_defaults_or_change_protocol() {
    let req = normalize_responses(json!({"model":"gpt-fixture","input":"hello"})).unwrap();
    let plan = UpstreamClient::build_request_plan(
        &payload("openai_compatible"),
        &req,
        "gpt-fixture",
        false,
    )
    .unwrap();
    assert_eq!(plan.url, "https://relay.example/v1/responses");
    let body = plan.body.unwrap();
    for name in ["store", "include", "parallel_tool_calls", "instructions"] {
        assert!(body.get(name).is_none(), "unexpected Codex default: {name}");
    }
}

#[test]
fn workbuddy_custom_authentication_wins_over_identity() {
    let mut p = payload("custom_http");
    p.auth_header_name = Some("X-Product".into());
    let req = normalize_responses(json!({"model":"gpt-fixture","input":"hello"})).unwrap();
    let h = build_inference_headers(&p, &req, "gpt-fixture", None);
    assert_eq!(h["x-product"], "provider-fixture");
}

#[test]
fn workbuddy_provider_inheritance_and_account_override_roundtrip() {
    let document = json!({"providers":[{"id":"relay","preset":"openai","base_url":"https://relay.example",
        "request_client_profile":"workbuddy","credentials":[
        {"id":"one","api_key":"fixture","supported_models":["inherit"]},
        {"id":"two","api_key":"fixture","supported_models":["override"],"request_client_profile":"codex_cli"}]}]});
    let typed: neuro_gateway::routing::config::RouteConfigYaml =
        serde_json::from_value(document).unwrap();
    assert_eq!(
        serde_json::to_value(&typed).unwrap()["providers"][0]["request_client_profile"],
        "workbuddy"
    );
    let store = neuro_gateway::routing::config::RouteConfigStore::from_document(typed).unwrap();
    for (model, profile) in [("inherit", "workbuddy"), ("override", "codex_cli")] {
        assert_eq!(
            serde_json::to_value(
                store.resolve_candidates(Some(model))[0]
                    .payload
                    .request_client_profile
            )
            .unwrap(),
            profile
        );
    }
}

#[test]
fn switching_away_clears_workbuddy_software_but_preserves_operator_policy() {
    let req = normalize_responses(json!({"model":"gpt-fixture","input":"hello"})).unwrap();
    let mut p = payload("openai_compatible");
    for (key, value) in [
        ("x-ide-type", "WorkBuddy"),
        ("x-ide-name", "WorkBuddy"),
        ("x-ide-version", "5.8.4"),
        ("x-product", "SaaS"),
        ("x-codebuddy-request", "1"),
        ("x-requested-with", "XMLHttpRequest"),
        ("x-private-data", "operator-policy"),
        ("accept", "application/custom"),
    ] {
        p.headers.insert(key.into(), value.into());
    }
    let h = build_inference_headers(&p, &req, "gpt-fixture", None);
    assert_eq!(h["accept"], "application/custom");
    assert_eq!(h["x-private-data"], "operator-policy");
    p.request_client_profile =
        Some(neuro_gateway::protocol::request_profile::RequestClientProfile::CodexCli);
    let h = build_inference_headers(&p, &req, "gpt-fixture", None);
    for name in [
        "x-ide-type",
        "x-ide-name",
        "x-ide-version",
        "x-product",
        "x-codebuddy-request",
        "x-requested-with",
    ] {
        assert!(!h.contains_key(name));
    }
    assert_eq!(h["x-private-data"], "operator-policy");
}

#[test]
fn workbuddy_preserves_only_unchanged_message_context() {
    use neuro_gateway::protocol::openai::normalize_chat_completions;
    let body = json!({"model":"fixture","messages":[{"role":"user","content":"hello",
        "agent":"main","conversationRequestId":"request-fixture"}]});
    let mut req = normalize_chat_completions(body.clone()).unwrap();
    let p = payload("openai_compatible");
    let plan = UpstreamClient::build_request_plan(&p, &req, "mapped", false).unwrap();
    let out = plan.body.unwrap();
    assert_eq!(out["messages"], body["messages"]);
    assert_eq!(out["model"], "mapped");
    req.messages[0].content.clear();
    let out = UpstreamClient::build_request_plan(&p, &req, "mapped", false)
        .unwrap()
        .body
        .unwrap();
    assert!(out["messages"][0].get("agent").is_none());
    assert!(out["messages"][0].get("conversationRequestId").is_none());
}

#[test]
fn workbuddy_context_does_not_leak_to_another_style() {
    let req = normalize_responses(json!({"model":"fixture","input":"hello"})).unwrap();
    let extra = HashMap::from([
        ("x-conversation-id".into(), "real-session".into()),
        ("b3".into(), "real-trace".into()),
        ("x-api-key".into(), "caller-secret".into()),
    ]);
    let mut p = payload("openai_compatible");
    let h = build_inference_headers(&p, &req, "fixture", Some(&extra));
    assert_eq!(h["x-conversation-id"], "real-session");
    assert_eq!(h["b3"], "real-trace");
    assert!(!h.contains_key("x-api-key"));
    assert_eq!(h["accept-encoding"], "gzip, deflate, br");
    assert_eq!(h["connection"], "keep-alive");
    p.request_client_profile = None;
    let h = build_inference_headers(&p, &req, "fixture", Some(&extra));
    assert!(!h.contains_key("x-conversation-id"));
    assert!(!h.contains_key("b3"));
}
