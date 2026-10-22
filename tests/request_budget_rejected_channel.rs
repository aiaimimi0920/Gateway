//! Explicit pre-execution rejection may advance a tool request, not replay it blindly.
use neuro_gateway::error::{classify_upstream_error, GatewayError};
use neuro_gateway::pipeline::request_budget::{RequestBudget, MAX_REQUEST_ATTEMPTS};
use neuro_gateway::protocol::canonical::{CanonicalRelayRequest, EndpointKind};
use serde_json::json;

#[tokio::test]
async fn balance_rejection_is_quota_and_allows_only_next_candidate() {
    use neuro_gateway::error::{ErrorKind, FallbackHint};
    for status in [402, 403] {
        let error = classify_upstream_error(
            status,
            r#"{"error":{"code":"bad_response_status_code","message":"Insufficient account balance"}}"#,
            Some("fixture"),
        );
        assert_eq!(error.kind, ErrorKind::InsufficientQuota);
        assert_eq!(error.http_status, Some(status));
        assert_eq!(error.code.as_deref(), Some("bad_response_status_code"));
        assert!(matches!(
            error.fallback_hint,
            FallbackHint::FallbackProvider { .. }
        ));
        let budget = RequestBudget::for_request(&request());
        budget.begin_attempt().unwrap();
        budget.allow_rejected_candidate_fallback(&error);
        budget.begin_attempt().unwrap();
        assert!(budget.begin_attempt().is_err());
    }
    for (status, message) in [
        (403, "Permission denied"),
        (403, "Content policy violation"),
        (401, "Insufficient account balance"),
        (500, "Insufficient account balance"),
    ] {
        let error = classify_upstream_error(
            status,
            &json!({"error":{"message":message}}).to_string(),
            None,
        );
        assert_ne!(error.kind, ErrorKind::InsufficientQuota);
        let budget = RequestBudget::for_request(&request());
        budget.begin_attempt().unwrap();
        budget.allow_rejected_candidate_fallback(&error);
        assert!(budget.begin_attempt().is_err());
    }
}

#[tokio::test]
async fn preconsume_quota_rejection_allows_next_provider_without_replaying_unknown_403() {
    use neuro_gateway::error::{ErrorKind, FallbackHint};
    let message = "Not enough quota to pre-consume for this request: ＄13.406424 remaining, ＄18.228000 required (request id: fixture)";
    for status in [402, 403] {
        let error = classify_upstream_error(
            status,
            &json!({"error":{"message":message}}).to_string(),
            Some("fixture"),
        );
        assert_eq!(error.kind, ErrorKind::InsufficientQuota);
        assert!(matches!(
            error.fallback_hint,
            FallbackHint::FallbackProvider { .. }
        ));
        let budget = RequestBudget::for_request(&request());
        budget.begin_attempt().unwrap();
        budget.allow_rejected_candidate_fallback(&error);
        budget.begin_attempt().unwrap();
        assert!(budget.begin_attempt().is_err());
    }
    for (status, text) in [
        (401, message),
        (500, message),
        (403, "Not enough quota"),
        (403, "Permission denied"),
    ] {
        let error =
            classify_upstream_error(status, &json!({"error":{"message":text}}).to_string(), None);
        let budget = RequestBudget::for_request(&request());
        budget.begin_attempt().unwrap();
        budget.allow_rejected_candidate_fallback(&error);
        assert!(budget.begin_attempt().is_err());
    }
}

fn request() -> CanonicalRelayRequest {
    serde_json::from_value(json!({
        "protocol_family":"open_ai", "endpoint_kind":"responses",
        "requested_model":"gpt-6.1-sol", "stream":true,
        "messages":[], "tools":[{"type":"function", "name":"shell", "raw":{}}],
        "raw_body":{}
    }))
    .unwrap()
}

fn rejection() -> GatewayError {
    classify_upstream_error(
        503,
        &json!({"error":{
            "code":"model_not_found", "message":
            "No available channel for model gpt-6.1-sol under group default (distributor)"
        }})
        .to_string(),
        Some("fixture"),
    )
}

#[tokio::test]
async fn tool_request_can_reach_next_candidate_after_explicit_rejection() {
    let req = request();
    let budget = RequestBudget::for_request(&req);
    budget.configure(&req, None).unwrap();
    assert_eq!(budget.remaining_attempts(), 1);
    budget.begin_attempt().unwrap();
    budget.allow_rejected_candidate_fallback(&rejection());
    budget.allow_rejected_candidate_fallback(&rejection()); // Duplicate observers cannot mint extra slots.
    assert_eq!(budget.remaining_attempts(), 1);
    assert_eq!(
        budget
            .run(async {
                budget.begin_attempt()?;
                Ok("next provider success")
            })
            .await
            .unwrap(),
        "next provider success"
    );
    budget.allow_rejected_candidate_fallback(&rejection());
    assert!(budget.begin_attempt().is_err());
}

#[tokio::test]
async fn repeated_rejections_still_obey_global_attempt_cap() {
    let budget = RequestBudget::for_request(&request());
    for _ in 0..MAX_REQUEST_ATTEMPTS {
        budget.begin_attempt().unwrap();
        budget.allow_rejected_candidate_fallback(&rejection());
    }
    assert_eq!(
        budget
            .begin_attempt()
            .unwrap_err()
            .request_budget_stop_reason(),
        Some("attempt_limit")
    );
    budget.allow_rejected_candidate_fallback(&rejection());
    assert!(budget.begin_attempt().is_err());
}

#[tokio::test]
async fn unknown_failure_after_rejection_does_not_grant_a_third_send() {
    let budget = RequestBudget::for_request(&request());
    budget.begin_attempt().unwrap();
    budget.allow_rejected_candidate_fallback(&rejection());
    budget.begin_attempt().unwrap();
    budget.allow_rejected_candidate_fallback(&GatewayError::server_error("unknown execution"));
    assert!(budget.begin_attempt().is_err());
    assert_eq!(budget.attempts(), 2);
}

#[tokio::test]
async fn uncertain_failures_and_non_conversation_creates_do_not_gain_slots() {
    for error in [
        GatewayError::server_error("unknown execution"),
        GatewayError::service_unavailable("No available channel for model x (distributor)"),
        GatewayError::bad_request("invalid tools").with_code("model_not_found"),
    ] {
        let budget = RequestBudget::for_request(&request());
        budget.begin_attempt().unwrap();
        budget.allow_rejected_candidate_fallback(&error);
        assert!(budget.begin_attempt().is_err());
    }
    let mut req = request();
    req.endpoint_kind = EndpointKind::ImagesGenerations;
    let budget = RequestBudget::for_request(&req);
    budget.begin_attempt().unwrap();
    budget.allow_rejected_candidate_fallback(&rejection());
    assert!(budget.begin_attempt().is_err());
}

#[tokio::test(start_paused = true)]
async fn rejection_does_not_extend_deadline() {
    let req = request();
    let budget = RequestBudget::for_request(&req);
    budget.configure(&req, Some(1)).unwrap();
    budget.begin_attempt().unwrap();
    tokio::time::advance(std::time::Duration::from_secs(1)).await;
    budget.allow_rejected_candidate_fallback(&rejection());
    assert_eq!(
        budget
            .begin_attempt()
            .unwrap_err()
            .request_budget_stop_reason(),
        Some("deadline")
    );
}
