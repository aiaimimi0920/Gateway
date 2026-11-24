//! Real-context translation must not invent accounts, conversations or privacy settings.
use neuro_gateway::protocol::responses::normalize_responses;
use neuro_gateway::routing::candidate::ProviderAccountPayload;
use neuro_gateway::upstream::headers::build_inference_headers;
use serde_json::json;
use std::collections::HashMap;

const SESSION: &str = "11111111-1111-4111-8111-111111111111";
const TURN: &str = "22222222-2222-4222-8222-222222222222";
const PARENT: &str = "33333333-3333-4333-8333-333333333333";

fn payload() -> ProviderAccountPayload {
    serde_json::from_value(
        json!({"adapter":"openai_compatible","base_url":"https://example.com",
        "api_key":"provider-fixture","request_client_profile":"workbuddy"}),
    )
    .unwrap()
}

fn input() -> HashMap<String, String> {
    HashMap::from([
        (
            "user-agent".into(),
            "codex_exec/0.155.1 (Windows; x64)".into(),
        ),
        ("originator".into(), "codex_exec".into()),
        ("thread-id".into(), SESSION.into()),
        ("session-id".into(), SESSION.into()),
        ("x-request-id".into(), PARENT.into()),
        ("accept".into(), "text/event-stream".into()),
        (
            "x-codex-turn-metadata".into(),
            json!({"session_id":SESSION,"thread_id":SESSION,
            "turn_id":TURN,"root_turn_id":TURN,"request_kind":"turn","agent_name":"/root",
            "installation_id":"not-provider-context"})
            .to_string(),
        ),
        (
            "x-codex-beta-features".into(),
            "remote_compaction_v2".into(),
        ),
    ])
}

fn headers(
    p: &ProviderAccountPayload,
    input: &HashMap<String, String>,
) -> rquest::header::HeaderMap {
    let req = normalize_responses(json!({"model":"fixture","input":"hello"})).unwrap();
    build_inference_headers(p, &req, "fixture", Some(input))
}

#[test]
fn maps_real_codex_session_turn_and_request_without_private_identity() {
    let h = headers(&payload(), &input());
    assert_eq!(h["x-conversation-id"], SESSION);
    assert_eq!(h["x-conversation-request-id"], TURN.replace('-', ""));
    assert_eq!(h["x-root-request-id"], TURN.replace('-', ""));
    assert_eq!(h["x-request-id"], PARENT.replace('-', ""));
    assert_eq!(h["x-conversation-message-id"], h["x-request-id"]);
    assert_eq!(h["x-agent-type"], "main");
    assert_eq!(h["x-agent-intent"], "craft");
    assert_eq!(h["x-agent-purpose"], "conversation");
    assert_eq!(h["accept"], "application/json");
    assert_eq!(h["authorization"], "Bearer provider-fixture");
    for name in [
        "session-id",
        "thread-id",
        "originator",
        "x-codex-turn-metadata",
        "x-codex-beta-features",
        "x-api-key",
        "x-user-id",
        "x-private-data",
        "x-compaction",
    ] {
        assert!(!h.contains_key(name), "unexpected field {name}");
    }
}

#[test]
fn preserves_real_session_across_turns_without_collapsing_turns() {
    let a = headers(&payload(), &input());
    let mut next = input();
    next.insert(
        "x-codex-turn-metadata".into(),
        json!({"thread_id":SESSION,"turn_id":PARENT,
        "root_turn_id":PARENT,"request_kind":"turn","agent_name":"/root"})
        .to_string(),
    );
    let b = headers(&payload(), &next);
    assert_eq!(a["x-conversation-id"], b["x-conversation-id"]);
    assert_ne!(
        a["x-conversation-request-id"],
        b["x-conversation-request-id"]
    );
}

#[test]
fn maps_actual_parent_and_does_not_guess_compaction_state() {
    let mut i = input();
    i.insert("x-codex-parent-thread-id".into(), PARENT.into());
    let h = headers(&payload(), &i);
    assert_eq!(h["x-parent-conversation-id"], PARENT);
    assert_eq!(h["x-agent-type"], "subagent");
    assert!(!h.contains_key("x-context-compacted"));
    assert!(!h.contains_key("x-codex-parent-thread-id"));
}

#[test]
fn conflicting_or_missing_sessions_are_not_invented() {
    for session in [Some(PARENT), None] {
        let mut i = input();
        if let Some(session) = session {
            i.insert("session-id".into(), session.into());
        } else {
            i.remove("session-id");
            i.remove("thread-id");
            i.remove("x-codex-turn-metadata");
        }
        let h = headers(&payload(), &i);
        assert!(!h.contains_key("x-conversation-id"));
        assert!(!h.contains_key("x-conversation-request-id"));
        assert!(!h.contains_key("x-agent-purpose"));
    }
}

#[test]
fn b3_and_w3c_trace_fields_are_consistent_and_preserve_sampling() {
    for supplied in [false, true] {
        let mut i = input();
        if supplied {
            i.insert(
                "traceparent".into(),
                "00-11111111111111111111111111111111-2222222222222222-00".into(),
            );
        }
        let h = headers(&payload(), &i);
        let parts: Vec<_> = h["traceparent"].to_str().unwrap().split('-').collect();
        assert_eq!(h["x-trace-id"], parts[1]);
        assert_eq!(h["x-b3-traceid"], parts[1]);
        assert_eq!(h["x-b3-spanid"], parts[2]);
        let sampled = if supplied { "0" } else { "1" };
        assert_eq!(h["x-b3-sampled"], sampled);
        assert_eq!(h["b3"], format!("{}-{}-{sampled}", parts[1], parts[2]));
    }
}

#[test]
fn operator_headers_win_and_non_codex_clients_are_not_fabricated() {
    let mut p = payload();
    p.headers
        .insert("Accept".into(), "application/custom".into());
    p.headers
        .insert("X-Conversation-ID".into(), "operator-session".into());
    p.headers
        .insert("X-Request-ID".into(), "operator-request".into());
    let h = headers(&p, &input());
    assert_eq!(h["accept"], "application/custom");
    assert_eq!(h["x-conversation-id"], "operator-session");
    assert_eq!(h["x-request-id"], "operator-request");
    let mut i = input();
    i.remove("originator");
    let h = headers(&payload(), &i);
    assert!(!h.contains_key("x-conversation-id"));
    assert!(!h.contains_key("traceparent"));
    assert!(!h.contains_key("x-codex-turn-metadata"));
}

#[test]
fn responses_body_keeps_business_metadata_without_codex_installation_fields() {
    use neuro_gateway::upstream::client::UpstreamClient;
    let req = normalize_responses(json!({"model":"fixture","input":"hello",
        "client_metadata":{"session_id":SESSION,"business":"keep",
            "x-codex-installation-id":"private-machine","x-codex-turn-metadata":"private-state"}}))
    .unwrap();
    let body = UpstreamClient::build_request_plan(&payload(), &req, "mapped", false)
        .unwrap()
        .body
        .unwrap();
    assert_eq!(
        body["client_metadata"],
        json!({"session_id":SESSION,"business":"keep"})
    );
    assert_eq!(body["model"], "mapped");
}

#[test]
fn custom_auth_cannot_be_reinterpreted_as_message_or_trace_context() {
    for key in ["X-Request-ID", "traceparent"] {
        let mut p = payload();
        p.adapter = "custom_http".into();
        p.auth_header_name = Some(key.into());
        p.api_key = SESSION.into();
        let h = headers(&p, &input());
        assert_eq!(h[key], SESSION);
        if key == "X-Request-ID" {
            assert!(!h.contains_key("x-conversation-message-id"));
        } else {
            assert!(!h.contains_key("b3"));
        }
    }
}
