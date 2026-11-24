//! CPA is a pinned software profile, not an authentication or endpoint adapter.
use neuro_gateway::{
    protocol::responses::normalize_responses,
    routing::candidate::ProviderAccountPayload,
    upstream::{client::UpstreamClient, headers::build_inference_headers},
};
use serde_json::json;
use std::collections::HashMap;

fn payload() -> ProviderAccountPayload {
    serde_json::from_value(json!({"adapter":"openai_compatible",
        "base_url":"https://relay.example", "api_key":"selected-fixture",
        "request_client_profile":"cpa"}))
    .unwrap()
}

#[test]
fn cpa_codex_negotiates_without_inventing_client_version_or_session() {
    for stream in [false, true] {
        let req =
            normalize_responses(json!({"model":"gpt-fixture","input":"hello","stream":stream}))
                .unwrap();
        let h = build_inference_headers(&payload(), &req, "gpt-fixture", None);
        assert!(!h.contains_key("version"), "CPA does not default Version");
        assert_eq!(
            h["accept"],
            if stream {
                "text/event-stream"
            } else {
                "application/json"
            }
        );
        assert_eq!(h["connection"], "Keep-Alive");
        assert_eq!(h["originator"], "codex-tui");
        for name in [
            "session-id",
            "thread-id",
            "chatgpt-account-id",
            "x-claude-code-session-id",
        ] {
            assert!(!h.contains_key(name));
        }
    }
}

#[test]
fn cpa_keeps_real_codex_continuation_and_version() {
    let req = normalize_responses(json!({"model":"gpt-fixture","input":"hello"})).unwrap();
    let extra = HashMap::from([
        ("version".into(), "caller-version".into()),
        ("session-id".into(), "real-session".into()),
        ("x-codex-turn-state".into(), "opaque-state".into()),
        ("authorization".into(), "Bearer inbound-secret".into()),
    ]);
    let h = build_inference_headers(&payload(), &req, "gpt-fixture", Some(&extra));
    assert_eq!(h["version"], "caller-version");
    assert_eq!(h["session-id"], "real-session");
    assert_eq!(h["x-codex-turn-state"], "opaque-state");
    assert_eq!(h["authorization"], "Bearer selected-fixture");
}

#[test]
fn cpa_claude_has_its_own_pinned_device_and_third_party_stream_negotiation() {
    let extra = HashMap::from([
        (
            "user-agent".into(),
            "claude-cli/2.1.278 (external, cli)".into(),
        ),
        ("x-app".into(), "cli".into()),
        ("x-stainless-os".into(), "Windows".into()),
        ("x-stainless-arch".into(), "x64".into()),
        (
            "x-codex-turn-metadata".into(),
            "private-installation".into(),
        ),
        ("session-id".into(), "codex-session".into()),
    ]);
    for stream in [false, true] {
        let req = normalize_responses(
            json!({"model":"claude-opus-fixture","input":"hello","stream":stream}),
        )
        .unwrap();
        let h = build_inference_headers(&payload(), &req, "claude-opus-fixture", Some(&extra));
        assert_eq!(h["user-agent"], "claude-cli/2.1.280 (external, cli)");
        assert_eq!(h["x-stainless-os"], "MacOS");
        assert_eq!(h["x-stainless-arch"], "arm64");
        assert_eq!(
            h["accept"],
            if stream {
                "text/event-stream"
            } else {
                "application/json"
            }
        );
        assert_eq!(
            h["accept-encoding"],
            if stream {
                "identity"
            } else {
                "gzip, deflate, br, zstd"
            }
        );
        assert!(!h.contains_key("originator"));
        assert!(!h.contains_key("x-codex-turn-metadata"));
        assert!(!h.contains_key("session-id"));
    }
}

#[test]
fn cpa_operator_auth_and_business_protocol_remain_authoritative() {
    let mut p = payload();
    p.adapter = "custom_http".into();
    p.auth_header_name = Some("user-agent".into());
    p.auth_token = Some("private-auth".into());
    p.headers
        .insert("accept".into(), "application/operator".into());
    let req =
        normalize_responses(json!({"model":"gpt-fixture","input":"business","temperature":0.3}))
            .unwrap();
    let h = build_inference_headers(&p, &req, "gpt-fixture", None);
    assert_eq!(h["user-agent"], "private-auth");
    assert_eq!(h["accept"], "application/operator");
    let plan = UpstreamClient::build_request_plan(&payload(), &req, "gpt-fixture", false).unwrap();
    assert_eq!(plan.url, "https://relay.example/v1/responses");
    let body = plan.body.unwrap();
    assert_eq!(body["input"], "business");
    assert_eq!(body["temperature"], 0.3);
}

#[test]
fn cpa_official_host_check_is_exact_and_operator_headers_win() {
    let req = normalize_responses(json!({"model":"claude-fixture","input":"hello","stream":true}))
        .unwrap();
    for (url, expected) in [
        ("https://api.anthropic.com", "application/json"),
        (
            "https://api.anthropic.com.evil.example",
            "text/event-stream",
        ),
        (
            "https://relay.example/api.anthropic.com",
            "text/event-stream",
        ),
    ] {
        let mut p = payload();
        p.base_url = url.into();
        let h = build_inference_headers(&p, &req, "claude-fixture", None);
        assert_eq!(h["accept"], expected);
        p.headers
            .insert("user-agent".into(), "operator-identity".into());
        p.headers
            .insert("accept".into(), "application/operator".into());
        let h = build_inference_headers(&p, &req, "claude-fixture", None);
        assert_eq!(h["user-agent"], "claude-cli/2.1.280 (external, cli)");
        assert_eq!(h["accept"], "application/operator");
    }
}
