//! Protocol boundary tests use the archived pending/review/session schemas, without a live account.
use super::*;

#[test]
fn luma_mcp_creation_review_validates_identity_contract_assets_and_limits() {
    let mut value = pending("g1", &["a1", "v1"])["pending"].clone();
    let parsed = creation::review(&value, "r1").unwrap().unwrap();
    assert!(!parsed.answerable());
    let mut ready = parsed.clone();
    ready.assets = creation::review_assets(&assets(&["a1", "v1"]), &ready.show).unwrap();
    assert!(ready.answerable());
    assert_eq!(ready.assets[1]["kind"], "video");
    value["run_id"] = json!("r2");
    assert!(creation::review(&value, "r1").is_err());
    value["run_id"] = json!("r1");
    value["interaction"]["answer_format"] = Value::Null;
    assert!(!creation::review(&value, "r1")
        .unwrap()
        .unwrap()
        .answerable());
    value["interaction"]["params"] = json!({"large":"x".repeat(25000)});
    assert!(creation::review(&value, "r1").is_err());
    let mut unsafe_assets = assets(&["a1", "v1"]);
    unsafe_assets["assets"][1]["url"] = json!("https://127.0.0.1/private");
    assert!(creation::review_assets(&unsafe_assets, &parsed.show).is_err());
    assert!(creation::review_assets(&assets(&["a1", "a1"]), &parsed.show).is_err());
    let mut truncated = pending("g1", &[])["pending"].clone();
    truncated["truncated"] = json!(true);
    assert!(!creation::review(&truncated, "r1")
        .unwrap()
        .unwrap()
        .answerable());
}

#[test]
fn luma_mcp_creation_review_cursor_requires_original_session_and_forward_progress() {
    let delivered =
        json!({"status":"delivered","session_id":"session:s1","run_id":"run:r1","seq":null});
    assert_eq!(creation::delivered(&delivered, "s1", "r1").unwrap(), 0);
    assert!(creation::delivered(&delivered, "s1", "other").is_err());
    let valid = page(3, false);
    assert_eq!(creation::session_update(&valid, "s1", 2).unwrap(), 3);
    for (key, value) in [
        ("session_id", json!("foreign")),
        ("next_after_seq", json!(1)),
        ("events", json!(["not an event"])),
        ("status", json!("unknown")),
    ] {
        let mut invalid = valid.clone();
        invalid[key] = value;
        assert!(creation::session_update(&invalid, "s1", 2).is_err());
    }
    assert!(creation::session_update(&page(2, true), "s1", 2).is_err());
    let mut working = json!({"session_id":"s1","status":"working","timed_out":true,"next_after_seq":2,"events":[]});
    assert_eq!(creation::session_update(&working, "s1", 2).unwrap(), 2);
    working["next_after_seq"] = json!(3);
    assert!(creation::session_update(&working, "s1", 2).is_err());
}

#[test]
fn luma_mcp_creation_review_ignores_unsupported_or_ambiguous_waiting_actions() {
    let direct = pending("g1", &[])["pending"].clone();
    let gate = json!({"kind":"gate","run_id":"r1","answer_type":"user.gate_verdict",
        "invocation_id":"g2","answer_format":direct["interaction"]["answer_format"]});
    let mut page = page(3, false);
    page["waiting"] = json!([gate.clone()]);
    assert!(creation::select_review(&direct, Some(&page), "r1")
        .unwrap()
        .is_none());
    page["waiting"] = json!([gate.clone(), gate.clone()]);
    assert!(creation::select_review(&Value::Null, Some(&page), "r1")
        .unwrap()
        .is_none());
    for (key, value) in [
        ("run_id", json!("foreign")),
        ("answer_type", json!("user.tool_result")),
        ("answer_format", Value::Null),
    ] {
        let mut unsupported = gate.clone();
        unsupported[key] = value;
        page["waiting"] = json!([unsupported]);
        assert!(creation::select_review(&Value::Null, Some(&page), "r1")
            .unwrap()
            .is_none());
    }
}
