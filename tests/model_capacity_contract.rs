//! Capacity evidence is transient, not a blanket exemption for client errors.
use neuro_gateway::error::{classify_upstream_error, ErrorKind};
use neuro_gateway::provider_failure::{
    classify_provider_failure, ProviderFailureClass, ProviderFailureScope,
};
use neuro_gateway::retry::{should_retry, RetryPolicy};
use serde_json::json;

#[test]
fn precise_capacity_evidence_can_retry_without_rewriting_status_or_code() {
    for status in [400, 429, 503] {
        let body = json!({"error":{"code":"model_capacity_exhausted",
            "message":"Selected model is at capacity. Please try a different model.",
            "retry_after": 3}})
        .to_string();
        let error = classify_upstream_error(status, &body, Some("fixture"));
        assert_eq!(serde_json::to_value(error.kind).unwrap(), "model_capacity");
        assert_eq!(error.http_status, Some(status));
        assert_eq!(error.code.as_deref(), Some("model_capacity_exhausted"));
        assert_eq!(error.upstream_retry_after_ms, Some(3_000));
        assert!(should_retry(&error, &RetryPolicy::default()));
        let disabled = RetryPolicy {
            retryable_statuses: vec![],
            ..Default::default()
        };
        assert!(!should_retry(&error, &disabled));
        let failure = classify_provider_failure(
            error.http_status,
            error.code.as_deref(),
            Some(&error.message),
        );
        assert_eq!(failure.class, ProviderFailureClass::ProviderTransient);
        assert_eq!(failure.scope, ProviderFailureScope::CredentialModel);
        assert!(!failure.permanent);
    }
}

#[test]
fn capacity_phrase_is_scoped_to_error_fields_and_does_not_override_auth_or_validation() {
    for (status, body) in [
        (
            400,
            json!({"error":{"message":"Invalid input"},"prompt":"Selected model is at capacity"}),
        ),
        (
            400,
            json!({"error":{"code":"invalid_value","message":"Selected model is at capacity"}}),
        ),
        (401, json!({"error":{"code":"model_capacity_exhausted"}})),
        (
            403,
            json!({"error":{"message":"Selected model is at capacity"}}),
        ),
        (404, json!({"error":{"code":"model_capacity_exhausted"}})),
        (
            400,
            json!({"error":{"message":"Expected text: selected model is at capacity"}}),
        ),
    ] {
        let error = classify_upstream_error(status, &body.to_string(), None);
        assert!(
            !should_retry(&error, &RetryPolicy::default()),
            "{status}: {body}"
        );
        assert_ne!(serde_json::to_value(error.kind).unwrap(), "model_capacity");
    }
    assert_eq!(
        classify_upstream_error(429, "rate limit", None).kind,
        ErrorKind::RateLimit
    );
}

#[test]
fn explicit_capacity_phrase_without_a_code_is_supported() {
    let body = json!({"error":{"message":"No capacity available for model gpt-fixture"}});
    let error = classify_upstream_error(400, &body.to_string(), None);
    assert!(should_retry(&error, &RetryPolicy::default()));
    assert!(error.code.is_none());
}
