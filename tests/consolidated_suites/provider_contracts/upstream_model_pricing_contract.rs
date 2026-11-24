//! Relay pricing compilation is an upstream admission failure, not caller validation.
use neuro_gateway::error::{classify_upstream_error, ErrorKind};
use neuro_gateway::provider_failure::{
    classify_provider_failure, ProviderFailureClass, ProviderFailureScope,
};
use neuro_gateway::retry::{should_retry, RetryPolicy};

const MESSAGE: &str =
    "model gpt-6.1-sol tiered expr run failed: expr compile error: unknown name cc12h (1:62)";

#[test]
fn pricing_compile_error_is_provider_failure_not_bad_request() {
    let body = serde_json::json!({"error": {"message": MESSAGE, "type": "new_api_error", "code": "model_price_error"}});
    let error = classify_upstream_error(400, &body.to_string(), Some("fixture-relay"));
    assert_eq!(error.kind, ErrorKind::ServerError);
    assert!(error.should_fallback());
    assert!(!error.retryable);
    assert!(!should_retry(&error, &RetryPolicy::default()));
    assert_eq!(error.http_status, Some(400));
    assert_eq!(error.code.as_deref(), Some("model_price_error"));
    assert_eq!(error.provider_name.as_deref(), Some("fixture-relay"));
    assert_eq!(error.message, MESSAGE);
    let feedback = classify_provider_failure(
        error.http_status,
        error.code.as_deref(),
        Some(&error.message),
    );
    assert_eq!(feedback.class, ProviderFailureClass::ProviderTransient);
    assert_eq!(feedback.scope, ProviderFailureScope::CredentialModel);
    assert!(!feedback.permanent);
}

#[test]
fn pricing_prefix_without_code_is_recognized_without_scanning_other_json_fields() {
    for body in [
        MESSAGE.to_string(),
        serde_json::json!({"message": MESSAGE}).to_string(),
        serde_json::json!({"error": {"message": MESSAGE}}).to_string(),
    ] {
        let error = classify_upstream_error(400, &body, None);
        assert_eq!(error.kind, ErrorKind::ServerError);
        assert!(error.should_fallback());
        assert!(!error.retryable);
    }
    for body in [
        serde_json::json!({"error": {"message": "invalid input"}, "prompt": MESSAGE}),
        serde_json::json!({"request": {"message": MESSAGE}, "code": "model_price_error"}),
    ] {
        let error = classify_upstream_error(400, &body.to_string(), None);
        assert_eq!(error.kind, ErrorKind::BadRequest);
        assert!(!error.should_fallback());
    }
}

#[test]
fn pricing_signal_cannot_override_auth_validation_or_arbitrary_errors() {
    for (status, code, message) in [
        (400, "invalid_request_error", MESSAGE),
        (400, "model_price_error", "invalid user-defined expression"),
        (
            400,
            "model_price_error",
            "A user quoted: model m tiered expr run failed: expr compile error: bad",
        ),
        (
            400,
            "model_price_error",
            "model  tiered expr run failed: expr compile error: bad",
        ),
        (
            400,
            "model_price_error",
            "model m tiered expr run failed: expr compile error:",
        ),
        (401, "model_price_error", MESSAGE),
        (403, "model_price_error", MESSAGE),
    ] {
        let body = serde_json::json!({"error": {"message": message, "code": code}});
        let error = classify_upstream_error(status, &body.to_string(), None);
        assert_ne!(error.kind, ErrorKind::ServerError);
        let feedback = classify_provider_failure(Some(status), Some(code), Some(message));
        assert_ne!(feedback.class, ProviderFailureClass::ProviderTransient);
    }
}

#[test]
fn pricing_error_diagnostics_are_bounded_and_sanitized() {
    let message = format!(
        "{MESSAGE}; api_key=sk-fixture-secret-value {}",
        "x".repeat(1500)
    );
    let body = serde_json::json!({"error": {"code": "model_price_error", "message": message}});
    let error = classify_upstream_error(400, &body.to_string(), None);
    assert_eq!(error.kind, ErrorKind::ServerError);
    assert!(!error.message.contains("sk-fixture-secret-value"));
    assert!(
        error.message.chars().count() <= neuro_gateway::error::PROVIDER_ERROR_MESSAGE_MAX_CHARS
    );
    let oversized = format!("{MESSAGE}{}", "x".repeat(4096));
    let body = serde_json::json!({"error": {"code": "model_price_error", "message": oversized}});
    assert_eq!(
        classify_upstream_error(400, &body.to_string(), None).kind,
        ErrorKind::BadRequest
    );
}

fn tool_request() -> neuro_gateway::protocol::canonical::CanonicalRelayRequest {
    serde_json::from_value(serde_json::json!({
        "protocol_family":"open_ai", "endpoint_kind":"responses",
        "requested_model":"gpt-6.1-sol", "stream":true,
        "messages":[], "tools":[{"type":"function", "name":"fixture_tool", "raw":{}}],
        "raw_body":{}
    }))
    .unwrap()
}

#[tokio::test(start_paused = true)]
async fn pricing_fallback_does_not_reopen_media_deadlines_or_handed_off_streams() {
    use neuro_gateway::pipeline::request_budget::RequestBudget;
    use neuro_gateway::protocol::canonical::EndpointKind;
    let body = serde_json::json!({"error": {"code": "model_price_error", "message": MESSAGE}});
    let error = classify_upstream_error(400, &body.to_string(), None);
    for endpoint in [
        EndpointKind::ImagesGenerations,
        EndpointKind::VideosGenerations,
        EndpointKind::MusicGenerations,
    ] {
        let mut req = tool_request();
        req.endpoint_kind = endpoint;
        let budget = RequestBudget::for_request(&req);
        budget.begin_attempt().unwrap();
        budget.allow_rejected_candidate_fallback(&error);
        assert!(budget.begin_attempt().is_err());
    }
    let req = tool_request();
    let handed_off = RequestBudget::for_request(&req);
    handed_off
        .run(async {
            handed_off.begin_attempt()?;
            Ok(())
        })
        .await
        .unwrap();
    handed_off.allow_rejected_candidate_fallback(&error);
    assert_eq!(
        handed_off
            .begin_attempt()
            .unwrap_err()
            .request_budget_stop_reason(),
        Some("response_handed_off")
    );
    let expired = RequestBudget::for_request(&req);
    expired.configure(&req, Some(1)).unwrap();
    expired.begin_attempt().unwrap();
    tokio::time::advance(std::time::Duration::from_secs(1)).await;
    expired.allow_rejected_candidate_fallback(&error);
    assert_eq!(
        expired
            .begin_attempt()
            .unwrap_err()
            .request_budget_stop_reason(),
        Some("deadline")
    );
}

#[test]
fn pricing_code_alone_cannot_grant_tool_replay() {
    use neuro_gateway::pipeline::request_budget::RequestBudget;
    let error = neuro_gateway::error::GatewayError::server_error("unknown execution result")
        .with_code("model_price_error");
    let budget = RequestBudget::for_request(&tool_request());
    budget.begin_attempt().unwrap();
    budget.allow_rejected_candidate_fallback(&error);
    assert!(budget.begin_attempt().is_err());
}
