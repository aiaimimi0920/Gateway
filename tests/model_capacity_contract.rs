//! Capacity evidence is transient, not a blanket exemption for client errors.
use neuro_gateway::error::{classify_upstream_error, ErrorKind};
use neuro_gateway::provider_failure::{
    classify_provider_failure, ProviderFailureClass, ProviderFailureScope,
};
use neuro_gateway::retry::{should_retry, RetryPolicy};
use serde_json::json;

#[test]
fn codex_capacity_refusal_is_not_an_ordinary_turn_rate_limit() {
    let error = classify_upstream_error(
        429,
        r#"{"error":{"code":"turn_rate_limit_exceeded","message":"Codex is at capacity, please try again later"}}"#,
        Some("chatgpt"),
    );
    assert_eq!(error.kind, ErrorKind::ModelCapacity);
    assert_eq!(error.http_status, Some(429));
    let failure = classify_provider_failure(
        error.http_status,
        error.code.as_deref(),
        Some(&error.message),
    );
    assert_eq!(failure.class, ProviderFailureClass::ProviderTransient);
    assert_eq!(failure.scope, ProviderFailureScope::CredentialModel);
    for message in [
        "Too many turns, please slow down",
        "Invalid prompt: Codex is at capacity",
    ] {
        let body = json!({"error":{"code":"turn_rate_limit_exceeded","message":message}});
        assert_eq!(
            classify_upstream_error(429, &body.to_string(), None).kind,
            ErrorKind::RateLimit
        );
    }
}

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
    let oversized = json!({"error":{"code":"x".repeat(129),
        "message":"Model is at capacity"}});
    assert_eq!(
        classify_upstream_error(429, &oversized.to_string(), None).kind,
        ErrorKind::RateLimit
    );
    // The shared extractor trims padding before retaining the code: a padded
    // known code remains bounded evidence, not a large cached error identity.
    let padded = json!({"error":{"code":format!("{}model_capacity_exhausted", " ".repeat(129)),
        "message":"Model is at capacity"}});
    let error = classify_upstream_error(429, &padded.to_string(), None);
    assert_eq!(error.kind, ErrorKind::ModelCapacity);
    assert_eq!(error.code.as_deref(), Some("model_capacity_exhausted"));
}

#[test]
fn explicit_capacity_phrase_without_a_code_is_supported() {
    let body = json!({"error":{"message":"No capacity available for model gpt-fixture"}});
    let error = classify_upstream_error(400, &body.to_string(), None);
    assert!(should_retry(&error, &RetryPolicy::default()));
    assert!(error.code.is_none());
}

#[tokio::test]
async fn capacity_response_preserves_status_code_message_and_rounds_retry_after_up() {
    use axum::response::IntoResponse;
    use http_body_util::BodyExt;
    for (delay, seconds) in [(2_501, "3"), (u64::MAX, "18446744073709552")] {
        let mut error = classify_upstream_error(
            503,
            r#"{"error":{"code":"model_capacity_exhausted","message":"Model is at capacity"}}"#,
            None,
        );
        error.fallback_hint = neuro_gateway::error::FallbackHint::Retry {
            delay_ms: delay,
            reason: "fixture".into(),
        };
        let response = error.into_response();
        assert_eq!(
            response.status(),
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(response.headers()["retry-after"], seconds);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["error"]["code"], "model_capacity_exhausted");
        assert_eq!(body["error"]["message"], "Model is at capacity");
    }
}
