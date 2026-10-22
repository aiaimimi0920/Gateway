use super::*;
use serde_json::json;

#[test]
fn explicit_mode_does_not_add_cache_markers() {
    let mut body = json!({"model":"claude-opus-5", "prompt_cache_options":{"mode":"explicit"},
        "messages":[{"role":"user","content":"hello"}]});
    body::shape(&mut body, "2026-10-21").unwrap();
    assert!(!body.to_string().contains("cache_control"));
    assert!(body.get("prompt_cache_options").is_none());
}

#[test]
fn cache_limit_preserves_last_tool_system_and_recent_messages() {
    let marker =
        json!({"type":"text","text":"hello","cache_control":{"type":"ephemeral","ttl":"1h"}});
    let mut body = json!({"prompt_cache_options":{"mode":"explicit"},
        "tools":[marker.clone(),marker.clone()], "system":[marker.clone(),marker.clone()],
        "messages":[{"role":"user","content":[marker.clone(),marker.clone()]}]});
    cache::apply(&mut body).unwrap();
    assert!(body["system"][0].get("cache_control").is_none());
    assert!(body["tools"][0].get("cache_control").is_none());
    assert!(body["system"][1].get("cache_control").is_some());
    assert!(body["tools"][1].get("cache_control").is_some());
    assert!(body["messages"][0]["content"][0]
        .get("cache_control")
        .is_some());
}

#[test]
fn later_long_ttl_is_removed_after_short_ttl() {
    let mut body = json!({
        "tools":[{"cache_control":{"type":"ephemeral","ttl":"5m"}}],
        "system":[{"cache_control":{"type":"ephemeral","ttl":"1h"}}], "messages":[]});
    cache::apply(&mut body).unwrap();
    assert_eq!(body["tools"][0]["cache_control"]["ttl"], "5m");
    assert!(body["system"][0]["cache_control"].get("ttl").is_none());
}
