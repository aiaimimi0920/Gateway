//! Regressions obtained from real Codex exec/TUI captures, using synthetic data only.
use neuro_gateway::{
    protocol::responses::normalize_responses,
    routing::candidate::ProviderAccountPayload,
    upstream::{client::UpstreamClient, headers::build_inference_headers},
};
use serde_json::{json, Value};
use std::collections::HashMap;

fn payload() -> ProviderAccountPayload {
    serde_json::from_value(
        json!({"adapter":"openai_compatible", "base_url":"https://fixture.invalid",
        "api_key":"selected-fixture", "request_client_profile":"codex_cli"}),
    )
    .unwrap()
}

fn title_request() -> Value {
    json!({"model":"gpt-fixture", "stream":true, "instructions":"title instructions",
        "input":[{"id":"msg_fixture", "type":"message", "role":"developer",
            "content":[{"type":"input_text","text":"Preserve developer priority."}]}],
        "tools":[], "tool_choice":"auto", "parallel_tool_calls":true,
        "store":false, "include":["reasoning.encrypted_content"],
        "reasoning":{"effort":"low","summary":"auto"},
        "prompt_cache_key":"fixture-session", "client_metadata":{"thread_id":"fixture-thread"},
        "future_vendor_field":{"opaque":[1,2,3]}})
}

#[test]
fn codex_native_empty_tools_preserve_every_original_json_value() {
    let original = title_request();
    let req = normalize_responses(original.clone()).unwrap();
    let plan = UpstreamClient::build_request_plan(&payload(), &req, "gpt-fixture", true).unwrap();
    assert_eq!(plan.body.unwrap(), original);
}

#[test]
fn codex_native_unknown_tools_are_not_an_injection_signal() {
    let mut original = title_request();
    original["tools"] = json!([{"type":"future_codex_tool", "opaque":{"schema":"untouched"}}]);
    let req = normalize_responses(original.clone()).unwrap();
    let plan = UpstreamClient::build_request_plan(&payload(), &req, "gpt-fixture", true).unwrap();
    assert_eq!(plan.body.unwrap(), original);
}

#[test]
fn codex_native_identity_is_retained_without_inventing_version_or_browser_headers() {
    let req = normalize_responses(title_request()).unwrap();
    for (origin, ua) in [
        (
            "codex_exec",
            "codex_exec/0.155.1 (Windows 10.0.26200; x86_64) unknown (codex_exec; 0.155.1)",
        ),
        (
            "codex-tui",
            "codex-tui/0.155.1 (Windows 10.0.26200; x86_64) xterm-256color (codex-tui; 0.155.1)",
        ),
    ] {
        let extra = HashMap::from([
            ("originator".into(), origin.into()),
            ("user-agent".into(), ua.into()),
            ("session-id".into(), "fixture-session".into()),
            (
                "x-codex-beta-features".into(),
                "remote_compaction_v2".into(),
            ),
        ]);
        let headers = build_inference_headers(&payload(), &req, "gpt-fixture", Some(&extra));
        assert_eq!(headers["originator"], origin);
        assert_eq!(headers["user-agent"], ua);
        assert_eq!(headers["accept"], "text/event-stream");
        assert!(!headers.contains_key("version"));
        assert_eq!(headers["session-id"], "fixture-session");
        assert_eq!(headers["authorization"], "Bearer selected-fixture");
    }
}

#[test]
fn codex_native_explicit_accept_and_provider_authentication_remain_authoritative() {
    let req = normalize_responses(title_request()).unwrap();
    let extra = HashMap::from([
        ("accept".into(), "*/*".into()),
        ("session-id".into(), "inbound-session".into()),
    ]);
    let mut provider = payload();
    let headers = build_inference_headers(&provider, &req, "gpt-fixture", Some(&extra));
    assert_eq!(headers["accept"], "*/*");
    provider
        .headers
        .insert("Accept".into(), "application/json".into());
    let headers = build_inference_headers(&provider, &req, "gpt-fixture", Some(&extra));
    assert_eq!(headers["accept"], "application/json");
    provider.adapter = "custom_http".into();
    provider.auth_header_name = Some("session-id".into());
    provider.auth_token = Some("provider-owned".into());
    let headers = build_inference_headers(&provider, &req, "gpt-fixture", Some(&extra));
    assert_eq!(headers["session-id"], "provider-owned");
}
