use super::*;
use crate::routing::candidate::ProviderAccountPayload;
use serde_json::json;

fn payload(adapter: &str, flow: serde_json::Value) -> ProviderAccountPayload {
    serde_json::from_value(json!({"adapter":adapter,"base_url":"https://fixture.test", "api_key":"fixture", "call_flow":flow})).unwrap()
}

#[test]
fn legacy_flow_borrows_without_mutating_payload_or_serialization() {
    let p = payload("openai_compatible", serde_json::Value::Null);
    let before = serde_json::to_value(&p).unwrap();
    let plan = prepare(&p).unwrap();
    assert!(matches!(plan.payload, std::borrow::Cow::Borrowed(_)));
    assert_eq!(before, serde_json::to_value(plan.payload.as_ref()).unwrap());
    assert!(before.get("call_flow").is_none());
}

#[test]
fn independent_api_format_and_fingerprint_keep_authentication() {
    let p = payload(
        "openai_compatible",
        json!({"call_adapter":"anthropic","fingerprint_adapter":"codex_cli","special_adapter":"none"}),
    );
    let plan = prepare(&p).unwrap();
    assert_eq!(plan.payload.adapter, "anthropic_compatible");
    assert_eq!(
        plan.payload.request_client_profile,
        Some(RequestClientProfile::CodexCli)
    );
    assert_eq!(plan.payload.api_key, "fixture");
    assert_eq!(plan.executor, "direct_http");
    assert_eq!(p.adapter, "openai_compatible");
}

#[test]
fn specialized_executor_has_canonical_input_and_session_fingerprint() {
    for (adapter, special) in [
        ("gemini_web_compatible", "gemini_web"),
        ("gemini_canvas_web_reverse_compatible", "gemini_canvas"),
        ("chatgpt_web_reverse_compatible", "chatgpt_web"),
        ("aistudio_web_reverse_compatible", "aistudio_web"),
    ] {
        let p = payload(
            adapter,
            json!({"call_adapter":"auto","fingerprint_adapter":"session","special_adapter":special}),
        );
        let plan = prepare(&p).unwrap();
        assert!(plan.executor_owns_fingerprint);
        assert_eq!(plan.executor, adapter);
    }
}

#[test]
fn unsupported_combinations_fail_instead_of_discarding_an_adapter() {
    for (adapter, flow) in [
        ("openai_compatible", json!({"special_adapter":"gemini_web"})),
        ("gemini_web_compatible", json!({"special_adapter":"none"})),
        (
            "gemini_web_compatible",
            json!({"special_adapter":"gemini_canvas"}),
        ),
        (
            "gemini_canvas_web_reverse_compatible",
            json!({"special_adapter":"gemini_web"}),
        ),
        ("gemini_web_compatible", json!({"call_adapter":"anthropic"})),
        (
            "gemini_web_compatible",
            json!({"fingerprint_adapter":"codex_cli"}),
        ),
        (
            "openai_compatible",
            json!({"fingerprint_adapter":"session"}),
        ),
    ] {
        assert!(prepare(&payload(adapter, flow)).is_err());
    }
}

#[test]
fn unknown_flow_options_fail_closed() {
    for flow in [
        json!({"call_adapter":"unknown"}),
        json!({"fingerprint_adapter":"unknown"}),
        json!({"special_adapter":"unknown"}),
        json!({"fingerprint_adaptor":"auto"}),
    ] {
        assert!(serde_json::from_value::<ProviderCallFlow>(flow).is_err());
    }
}
