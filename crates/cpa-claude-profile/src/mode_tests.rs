use super::*;
use serde_json::json;

#[test]
fn caller_system_stays_top_level_on_first_turn_and_moves_as_separate_cached_blocks() {
    let mut body = json!({"model":"claude-opus-5","system":[{"type":"text","text":"first"},{"type":"text","text":"second"}],"messages":[{"role":"user","content":"hello"}]});
    body::shape(&mut body, "2026-10-21").unwrap();
    assert_eq!(body["system"][2]["text"], "first");
    assert_eq!(body["system"][3]["text"], "second");
    assert_eq!(body["messages"].as_array().unwrap().len(), 1);
    let mut later = json!({"model":"claude-opus-5","system":"caller","messages":[{"role":"user","content":"hello"},{"role":"assistant","content":"ok"},{"role":"user","content":"next"}]});
    body::shape(&mut later, "2026-10-21").unwrap();
    assert_eq!(later["messages"][1]["role"], "system");
    assert_eq!(
        later["messages"][1]["content"][0]["cache_control"]["ttl"],
        "1h"
    );
}

#[test]
fn explicit_and_native_regular_requests_include_extended_cache_beta() {
    let body = json!({"model":"claude-opus-5","messages":[{"role":"user","content":"hello"}]});
    for native in [false, true] {
        assert!(select_beta_headers(&body, "claude-code-20250219", native)
            .contains("extended-cache-ttl-2025-04-11"));
    }
}

#[test]
fn native_preserves_system_cache_placement_but_rebinds_credential_identity() {
    let mut body = json!({"model":"claude-opus-5","system":"native-owned",
        "messages":[{"role":"user","content":"hello"}],
        "metadata":{"user_id":"{\"session_id\":\"12345678-1234-4234-8234-123456789012\",\"extra\":\"kept\"}"}});
    let original = body.clone();
    prepare_mode(
        &mut body, &original, "selected", None, "scope", "claude", true,
    )
    .unwrap();
    assert_eq!(body["system"], original["system"]);
    assert_eq!(body["messages"], original["messages"]);
    assert!(!body.to_string().contains("cache_control"));
    assert_eq!(
        session_header(&body).unwrap(),
        "12345678-1234-4234-8234-123456789012"
    );
    let id: Value = serde_json::from_str(body["metadata"]["user_id"].as_str().unwrap()).unwrap();
    assert_eq!(id["extra"], "kept");
    assert!(id.get("device_id").is_some());
}

#[test]
fn native_beta_baseline_is_retained_and_helper_effort_is_removed() {
    let body = json!({"model":"claude-opus-5","max_tokens":1,"messages":[{"role":"user","content":"test"}]});
    assert_eq!(
        select_beta_headers(
            &body,
            "claude-code-20250219,effort-2025-11-24,custom-beta",
            true
        ),
        "claude-code-20250219,custom-beta"
    );
}

#[test]
fn helper_does_not_get_hour_cache_or_effort() {
    let original = json!({"model":"claude-opus-5","max_tokens":1,"messages":[{"role":"user","content":"test"}]});
    let mut body = original.clone();
    prepare(&mut body, &original, "key", None, "scope").unwrap();
    assert!(!body.to_string().contains("\"ttl\""));
    assert!(!beta_headers(&body).contains("effort-"));
    assert!(!beta_headers(&body).contains("extended-cache"));
}

#[test]
fn response_roots_are_stable_and_protocol_specific() {
    let response = json!({"instructions":"be concise", "input":"hello"});
    let a = identity::session_for(&response, None, "caller", "openai-response");
    assert_eq!(
        a,
        identity::session_for(&response, None, "caller", "openai-response")
    );
    assert_ne!(
        a,
        identity::session_for(&response, None, "other", "openai-response")
    );
}

#[test]
fn prefixed_maximum_length_session_label_is_not_dropped() {
    let hint = format!("header:{}", "a".repeat(256));
    assert_eq!(
        identity::session(&json!({}), Some(&hint), ""),
        identity::project_session(&hint)
    );
}
