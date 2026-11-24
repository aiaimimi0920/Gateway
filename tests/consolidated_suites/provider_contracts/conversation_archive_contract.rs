use serde_json::json;

use neuro_gateway::conversation_archive::{
    archive_user_id, is_conversation_archive_endpoint, sanitize_archive_value,
    truncate_archive_text,
};
use neuro_gateway::protocol::canonical::EndpointKind;
use neuro_gateway::provider_failure::{
    classify_provider_failure, ProviderFailureClass, ProviderFailureScope,
};

#[test]
fn archive_user_id_prefers_explicit_neuro_user_over_session_user() {
    assert_eq!(
        archive_user_id(Some(" user-from-header "), Some("session-user")).as_deref(),
        Some("user-from-header")
    );
}

#[test]
fn archive_user_id_falls_back_to_session_user_and_rejects_blank_values() {
    assert_eq!(
        archive_user_id(Some("   "), Some(" session-user ")).as_deref(),
        Some("session-user")
    );
    assert_eq!(archive_user_id(Some("   "), Some("  ")), None);
}

#[test]
fn archive_sanitizer_redacts_nested_secret_fields_but_preserves_prompt_text() {
    let sanitized = sanitize_archive_value(&json!({
        "messages": [
            {"role": "user", "content": "Tell me whether the word token appears in this sentence."}
        ],
        "headers": {
            "Authorization": "Bearer should-not-survive",
            "Cookie": "session=should-not-survive",
            "X-Trace": "trace-1"
        },
        "payload": {
            "apiKey": "sk-should-not-survive",
            "refresh_token": "refresh-should-not-survive",
            "nested": {
                "password": "pw-should-not-survive",
                "ordinary": "keep-me"
            }
        }
    }));

    assert_eq!(sanitized["headers"]["Authorization"], "[REDACTED]");
    assert_eq!(sanitized["headers"]["Cookie"], "[REDACTED]");
    assert_eq!(sanitized["headers"]["X-Trace"], "trace-1");
    assert_eq!(sanitized["payload"]["apiKey"], "[REDACTED]");
    assert_eq!(sanitized["payload"]["refresh_token"], "[REDACTED]");
    assert_eq!(sanitized["payload"]["nested"]["password"], "[REDACTED]");
    assert_eq!(sanitized["payload"]["nested"]["ordinary"], "keep-me");
    assert_eq!(
        sanitized["messages"][0]["content"],
        "Tell me whether the word token appears in this sentence."
    );
}

#[test]
fn archive_text_truncation_is_byte_safe_and_reports_status() {
    let (text, truncated) = truncate_archive_text("你好abcdef", 8);

    assert!(truncated);
    assert_eq!(text, "你好ab");
    assert!(text.is_char_boundary(text.len()));
}

#[test]
fn archive_endpoint_gate_only_accepts_conversation_like_endpoints() {
    assert!(is_conversation_archive_endpoint(
        EndpointKind::ChatCompletions
    ));
    assert!(is_conversation_archive_endpoint(EndpointKind::Responses));
    assert!(is_conversation_archive_endpoint(EndpointKind::Messages));
    assert!(!is_conversation_archive_endpoint(EndpointKind::Embeddings));
    assert!(!is_conversation_archive_endpoint(
        EndpointKind::ImagesGenerations
    ));
}

#[test]
fn provider_failure_classification_keeps_credential_model_and_transient_scopes_separate() {
    let invalid =
        classify_provider_failure(Some(401), Some("invalid_api_key"), Some("invalid api key"));
    assert_eq!(invalid.class, ProviderFailureClass::CredentialInvalid);
    assert_eq!(invalid.scope, ProviderFailureScope::Credential);
    assert!(invalid.permanent);

    let unsupported = classify_provider_failure(
        Some(404),
        Some("model_not_found"),
        Some("model does not exist"),
    );
    assert_eq!(unsupported.class, ProviderFailureClass::ModelUnsupported);
    assert_eq!(unsupported.scope, ProviderFailureScope::CredentialModel);
    assert!(unsupported.permanent);

    let rate_limited = classify_provider_failure(
        Some(429),
        Some("rate_limit_exceeded"),
        Some("too many requests"),
    );
    assert_eq!(rate_limited.class, ProviderFailureClass::RateLimited);
    assert_eq!(rate_limited.scope, ProviderFailureScope::CredentialModel);
    assert!(!rate_limited.permanent);

    let upstream = classify_provider_failure(Some(503), None, Some("upstream unavailable"));
    assert_eq!(upstream.class, ProviderFailureClass::ProviderTransient);
    assert_eq!(upstream.scope, ProviderFailureScope::Provider);
    assert!(!upstream.permanent);
}
