use super::*;
use serde_json::json;

#[test]
fn billing_matches_pinned_capture() {
    assert_eq!(
        body::billing("Reply CPA_OK only."),
        "x-anthropic-billing-header: cc_version=2.1.280.b5a; cc_entrypoint=cli;"
    );
}

#[test]
fn identity_is_stable_scoped_and_does_not_leak_seed() {
    let a = identity::user_id("secret-a", "session", None);
    assert_eq!(a, identity::user_id("secret-a", "session", None));
    assert_ne!(a, identity::user_id("secret-b", "session", None));
    assert!(!a.contains("secret-a"));
    let value: Value = serde_json::from_str(&a).unwrap();
    assert_eq!(value["device_id"].as_str().unwrap().len(), 64);
    assert_eq!(
        uuid::Uuid::parse_str(value["account_uuid"].as_str().unwrap())
            .unwrap()
            .get_version_num(),
        5
    );
}

#[test]
fn root_stays_stable_when_conversation_grows() {
    let original = json!({"messages":[{"role":"user","content":"hello"}]});
    let mut continued = original.clone();
    continued["messages"].as_array_mut().unwrap().extend([
        json!({"role":"assistant","content":"answer"}),
        json!({"role":"user","content":"next"}),
    ]);
    assert_eq!(
        identity::session(&original, None, "caller-a"),
        identity::session(&continued, None, "caller-a")
    );
    assert_ne!(
        identity::session(&original, None, "caller-a"),
        identity::session(&original, None, "caller-b")
    );
}

#[test]
fn shaping_preserves_generation_and_explicit_cache_ttl() {
    let mut body = json!({"model":"claude-opus-5", "max_tokens":17,"temperature":0.3,
        "messages":[{"role":"user","content":[{"type":"text","text":"hello",
            "cache_control":{"type":"ephemeral","ttl":"5m"}}]}]});
    let original = body.clone();
    prepare(
        &mut body,
        &original,
        "synthetic-key",
        Some("thread:fixture"),
        "caller",
    )
    .unwrap();
    assert_eq!(body["max_tokens"], 17);
    assert_eq!(body["temperature"], 0.3);
    assert_eq!(
        body.pointer("/messages/0/content/1/cache_control/ttl")
            .unwrap(),
        "5m"
    );
    assert_eq!(
        session_header(&body).unwrap(),
        identity::project_session("thread:fixture")
    );
    assert!(beta_headers(&body).ends_with("extended-cache-ttl-2025-04-11"));
}

#[test]
fn rejects_missing_seed_and_oauth_authority() {
    for seed in ["", "sk-ant-oat-test"] {
        assert!(prepare(&mut json!({}), &json!({}), seed, None, "").is_err());
    }
}

#[test]
fn control_characters_cannot_be_hidden_by_trimming() {
    assert!(identity::valid_label("\nabc").is_none());
    assert_eq!(identity::valid_label(" abc "), Some("abc"));
}

#[test]
fn legacy_lookup_is_exact_and_provider_independent() {
    assert!(betas::legacy("provider/CLAUDE-OPUS-4-6"));
    assert!(!betas::legacy("claude-opus-4-6-custom"));
}

#[test]
fn rejected_body_is_not_partially_mutated() {
    let original = json!({"system":[{"type":"image"}],"messages":[]});
    let mut body = original.clone();
    assert!(prepare(&mut body, &original, "key", None, "caller").is_err());
    assert_eq!(body, original);
    assert!(prepare(&mut Value::Null, &original, "key", None, "caller").is_err());
}

#[test]
fn image_only_conversations_have_stable_distinct_roots() {
    let body = json!({"messages":[{"role":"user","content":[
        {"type":"image","source":{"type":"base64","media_type":"image/png","data":"a"}}]}]});
    let mut other = body.clone();
    other["messages"][0]["content"][0]["source"]["data"] = json!("b");
    assert_eq!(
        identity::session(&body, None, ""),
        identity::session(&body, None, "")
    );
    assert_ne!(
        identity::session(&body, None, ""),
        identity::session(&other, None, "")
    );
}

#[test]
fn unknown_block_map_order_and_cache_controls_do_not_change_root() {
    let a = json!({"messages":[{"role":"user","content":[{"z":2,"a":1}]}]});
    let b = json!({"messages":[{"role":"user","content":[{"a":1,"cache_control":{},"z":2}]}]});
    assert_eq!(
        identity::session(&a, None, ""),
        identity::session(&b, None, "")
    );
}

#[test]
fn ordinary_messages_body_matches_pinned_cpa_capture() {
    // Synthetic-key CPA capture: gateway-e2e-20261021-194326/cpa/capture.json.
    // Session and date are pinned separately, not mistaken for derivation coverage.
    let mut payload = json!({"model":"claude-opus-5","max_tokens":32,"stream":true,
        "messages":[{"role":"user","content":"Reply CPA_OK only."}]});
    body::shape(&mut payload, "2026-10-21").unwrap();
    let user_id = identity::user_id(
        "synthetic-cpa-rust-isolation-key",
        "d9a788df-e036-5a9a-b3c5-8d594d6fecbf",
        None,
    );
    assert_eq!(
        user_id,
        concat!(
            "{\"device_id\":\"72dde53fd06c668ec5a22dba7cd254d55eeed1036102ed760faffd94dfec5b83\",",
            "\"account_uuid\":\"e0a74ac3-4a20-5bfe-9674-0c976a90b941\",",
            "\"session_id\":\"d9a788df-e036-5a9a-b3c5-8d594d6fecbf\"}"
        )
    );
    payload["metadata"] = json!({"user_id":user_id});
    let captured: Value =
        serde_json::from_str(include_str!("../fixtures/ordinary-messages.json")).unwrap();
    assert_eq!(payload, captured);
    assert_eq!(
        serde_json::to_vec(&payload).unwrap(),
        serde_json::to_vec(&captured).unwrap()
    );
    assert_eq!(serde_json::to_vec(&payload).unwrap().len(), 990);
    assert_eq!(
        beta_headers(&payload),
        concat!(
            "claude-code-20250219,oauth-2025-04-20,interleaved-thinking-2025-05-14,",
            "redact-thinking-2026-02-12,thinking-token-count-2026-05-13,",
            "context-management-2025-06-27,prompt-caching-scope-2026-01-05,",
            "mid-conversation-system-2026-04-07,mid-conversation-tool-changes-2026-07-01,",
            "effort-2025-11-24,extended-cache-ttl-2025-04-11"
        )
    );
}
