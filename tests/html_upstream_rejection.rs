//! HTML diagnostic refusals may use normal fallback, but never manufacture replay slots.
use neuro_gateway::error::{classify_upstream_error, ErrorKind, FallbackHint};
use neuro_gateway::pipeline::request_budget::RequestBudget;
use neuro_gateway::protocol::canonical::CanonicalRelayRequest;
use serde_json::json;

const PAGE: &str = "<!DOCTYPE html><html lang=\"zh-CN\"><head><title>系统终端 - 异常状态诊断</title><script src=\"https://example.test\"></script></head></html>";

#[test]
fn diagnostic_html_selects_next_candidate_without_exposing_markup() {
    let error = classify_upstream_error(403, PAGE, Some("fixture"));
    assert_eq!(error.kind, ErrorKind::ServiceUnavailable);
    assert_eq!(error.code.as_deref(), Some("upstream_html_diagnostic"));
    assert!(matches!(
        error.fallback_hint,
        FallbackHint::FallbackProvider { .. }
    ));
    assert!(!error.retryable);
    assert!(!error.message.contains('<'));
    assert!(!error.message.contains("example.test"));
}

#[test]
fn html_does_not_prove_safe_replay_of_tool_requests() {
    let request: CanonicalRelayRequest = serde_json::from_value(json!({
        "protocol_family":"open_ai", "endpoint_kind":"responses",
        "requested_model":"fixture", "stream":true, "messages":[],
        "tools":[{"type":"function", "name":"shell", "raw":{}}], "raw_body":{}
    }))
    .unwrap();
    let budget = RequestBudget::for_request(&request);
    budget.begin_attempt().unwrap();
    budget.allow_rejected_candidate_fallback(&classify_upstream_error(403, PAGE, None));
    assert!(budget.begin_attempt().is_err());
}

#[test]
fn unknown_html_and_explicit_denials_still_abort() {
    for body in [
        "<html><title>Forbidden</title></html>".to_owned(),
        format!("{PAGE} Content policy violation"),
        format!("{PAGE} Permission denied"),
        format!("{PAGE} {} Permission denied", " ".repeat(8192)),
        r#"{"error":{"message":"Forbidden by policy"}}"#.to_owned(),
    ] {
        let error = classify_upstream_error(403, &body, None);
        assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
    }
}
