//! Pure admission tests never construct a network request or start a recovery worker.
use super::{material, request, test_support as wire};
use serde_json::json;
#[test]
fn qwen_web_current_material_fixed_headers_and_unsafe_variants() {
    let p = wire::payload();
    let headers = material::headers(&p).unwrap();
    assert_eq!(headers["source"], "web");
    assert_eq!(headers["version"], "0.3.36");
    assert!(headers["authorization"].is_sensitive());
    assert!(headers["cookie"].is_sensitive());
    assert!(headers["timezone"].to_str().unwrap().ends_with("GMT+0000"));
    for (key, value) in [
        ("Cookie", "x=1; x=2"),
        ("cookie", "x=1"),
        ("User-Agent", "bad\nagent"),
        ("Cookie", "x=a b"),
    ] {
        let mut p = wire::payload();
        p.headers.insert(key.into(), value.into());
        assert!(material::headers(&p).is_err());
    }
    for url in [
        "http://chat.qwen.ai",
        "https://user@chat.qwen.ai",
        "https://chat.qwen.ai/?x",
        "https://chat.qwen.ai/#x",
        "https://auth.qwen.ai",
        "https://chat.qwen.ai/api/v2",
    ] {
        let mut p = wire::payload();
        p.base_url = url.into();
        assert!(material::headers(&p).is_err(), "{url}");
    }
}
#[test]
fn qwen_web_current_request_does_not_flatten_history_tools_or_unsupported_parameters() {
    let mut r = wire::request();
    r.messages.push(r.messages[0].clone());
    assert!(request::validate(&r, "model").is_err());
    for key in [
        "temperature",
        "max_tokens",
        "instructions",
        "tools",
        "previous_response_id",
        "unknown",
    ] {
        let mut r = wire::request();
        r.raw_body[key] = json!("unsupported");
        assert!(request::validate(&r, "model").is_err(), "{key}");
    }
    let mut r = wire::request();
    r.messages[0].role = crate::protocol::canonical::MessageRole::System;
    assert!(request::validate(&r, "model").is_err());
    assert_eq!(
        request::validate(&wire::request(), "model").unwrap(),
        " 合成问题 "
    );
}
