//! Only a recognized pre-output diagnostic refusal may grant bounded conversation fallback.
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

fn tool_request(endpoint: &str) -> CanonicalRelayRequest {
    serde_json::from_value(json!({
        "protocol_family":"open_ai", "endpoint_kind":endpoint,
        "requested_model":"fixture", "stream":true, "messages":[],
        "tools":[{"type":"function", "name":"shell", "raw":{}}], "raw_body":{}
    }))
    .unwrap()
}

#[test]
fn diagnostic_refusal_grants_only_next_slot_up_to_shared_limit() {
    let budget = RequestBudget::for_request(&tool_request("responses"));
    let error = classify_upstream_error(403, PAGE, None);
    budget.allow_rejected_candidate_fallback(&error);
    assert_eq!(budget.remaining_attempts(), 1);
    for attempts in 1..=6 {
        budget.begin_attempt().unwrap();
        budget.allow_rejected_candidate_fallback(&error);
        assert_eq!(budget.attempts(), attempts);
        assert_eq!(budget.remaining_attempts(), u32::from(attempts < 6));
    }
    assert_eq!(
        budget
            .begin_attempt()
            .unwrap_err()
            .request_budget_stop_reason(),
        Some("attempt_limit")
    );
    budget.allow_rejected_candidate_fallback(&error);
    assert!(
        budget.begin_attempt().is_err(),
        "cannot revive a stopped budget"
    );
}

#[tokio::test(start_paused = true)]
async fn diagnostic_refusal_cannot_reset_deadline_or_handoff() {
    let request = tool_request("responses");
    let error = classify_upstream_error(403, PAGE, None);
    let budget = RequestBudget::for_request(&request);
    budget.configure(&request, Some(1)).unwrap();
    budget.begin_attempt().unwrap();
    tokio::time::advance(std::time::Duration::from_secs(1)).await;
    budget.allow_rejected_candidate_fallback(&error);
    assert_eq!(
        budget
            .begin_attempt()
            .unwrap_err()
            .request_budget_stop_reason(),
        Some("deadline")
    );

    let handed_off = RequestBudget::for_request(&request);
    handed_off
        .run(async {
            handed_off.begin_attempt()?;
            Ok(())
        })
        .await
        .unwrap();
    handed_off.allow_rejected_candidate_fallback(&error);
    assert_eq!(handed_off.remaining_attempts(), 0);
    assert_eq!(
        handed_off
            .begin_attempt()
            .unwrap_err()
            .request_budget_stop_reason(),
        Some("response_handed_off")
    );
}

#[test]
fn diagnostic_refusal_does_not_authorize_media_creation_replay() {
    let budget = RequestBudget::for_request(&tool_request("images_generations"));
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
        r#"{"error":{"code":"upstream_html_diagnostic","message":"Forbidden by policy"}}"#
            .to_owned(),
    ] {
        let error = classify_upstream_error(403, &body, None);
        assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
        let budget = RequestBudget::for_request(&tool_request("responses"));
        budget.begin_attempt().unwrap();
        budget.allow_rejected_candidate_fallback(&error);
        assert!(budget.begin_attempt().is_err());
    }
}
