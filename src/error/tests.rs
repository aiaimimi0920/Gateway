use super::classification::{kind_from_body_keywords, kind_from_http_status};
use super::*;

const TEST_JWT: &str = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJzZWNyZXQtdXNlciJ9.c2lnbmF0dXJl";

#[test]
fn provider_error_message_sanitizer_redacts_secrets_and_bounds_output() {
    let raw = format!(
        "invalid api key\nAuthorization: Bearer bearer-secret-value\nBasic dXNlcjpwYXNzd29yZA==\napi_key=sk-proj-super-secret\ntoken='plain-token-secret'\ntoken=Bearer bearer-assignment-secret\nCookie: session=private-session; csrftoken=private-cookie\njwt={TEST_JWT}\n{}",
        "x".repeat(PROVIDER_ERROR_MESSAGE_MAX_CHARS * 2)
    );

    let sanitized = sanitize_provider_error_message(&raw);

    for secret in [
        "bearer-secret-value",
        "dXNlcjpwYXNzd29yZA==",
        "sk-proj-super-secret",
        "plain-token-secret",
        "bearer-assignment-secret",
        "private-session",
        "private-cookie",
        TEST_JWT,
    ] {
        assert!(
            !sanitized.contains(secret),
            "sanitized provider error leaked {secret}: {sanitized}"
        );
    }
    assert!(sanitized.contains("invalid api key"));
    assert!(sanitized.contains("[REDACTED]"));
    assert!(sanitized.chars().count() <= PROVIDER_ERROR_MESSAGE_MAX_CHARS);
}

// ── retryability ─────────────────────────────────────────────────────

#[test]
fn retryable_kinds_are_marked_retryable() {
    for kind in [
        ErrorKind::RateLimit,
        ErrorKind::ServerError,
        ErrorKind::Network,
        ErrorKind::ServiceUnavailable,
        ErrorKind::Timeout,
    ] {
        assert!(is_kind_retryable(kind), "{kind:?} should be retryable");
    }
}

#[test]
fn non_retryable_kinds_are_not_retryable() {
    for kind in [
        ErrorKind::Authentication,
        ErrorKind::BadRequest,
        ErrorKind::ContentFilter,
        ErrorKind::ContextLength,
        ErrorKind::InsufficientQuota,
        ErrorKind::ModelNotFound,
        ErrorKind::Unknown,
    ] {
        assert!(!is_kind_retryable(kind), "{kind:?} should NOT be retryable");
    }
}

// ── kind_from_http_status ────────────────────────────────────────────

#[test]
fn http_status_mapping_covers_common_codes() {
    assert_eq!(kind_from_http_status(400), Some(ErrorKind::BadRequest));
    assert_eq!(kind_from_http_status(401), Some(ErrorKind::Authentication));
    assert_eq!(kind_from_http_status(403), Some(ErrorKind::Authentication));
    assert_eq!(kind_from_http_status(404), Some(ErrorKind::ModelNotFound));
    assert_eq!(kind_from_http_status(429), Some(ErrorKind::RateLimit));
    assert_eq!(kind_from_http_status(500), Some(ErrorKind::ServerError));
    assert_eq!(kind_from_http_status(502), Some(ErrorKind::ServerError));
    assert_eq!(kind_from_http_status(503), Some(ErrorKind::ServerError));
    assert_eq!(kind_from_http_status(504), Some(ErrorKind::Timeout));
}

#[test]
fn http_status_returns_none_for_unmapped_codes() {
    assert_eq!(kind_from_http_status(200), None);
    assert_eq!(kind_from_http_status(418), None);
}

// ── keyword scanning ─────────────────────────────────────────────────

#[test]
fn keyword_detects_rate_limit() {
    assert_eq!(
        kind_from_body_keywords("You have exceeded your rate limit"),
        Some(ErrorKind::RateLimit)
    );
    assert_eq!(
        kind_from_body_keywords("rate_limit exceeded"),
        Some(ErrorKind::RateLimit)
    );
}

#[test]
fn keyword_detects_context_length() {
    assert_eq!(
        kind_from_body_keywords("prompt too long for context length"),
        Some(ErrorKind::ContextLength)
    );
    assert_eq!(
        kind_from_body_keywords("tokens exceed the maximum"),
        Some(ErrorKind::ContextLength)
    );
}

#[test]
fn keyword_detects_content_filter() {
    assert_eq!(
        kind_from_body_keywords("content filter triggered"),
        Some(ErrorKind::ContentFilter)
    );
    assert_eq!(
        kind_from_body_keywords("safety policy violation"),
        Some(ErrorKind::ContentFilter)
    );
}

#[test]
fn keyword_returns_none_for_benign_text() {
    assert_eq!(kind_from_body_keywords("everything is fine"), None);
}

// ── constructor helpers ──────────────────────────────────────────────

#[test]
fn bad_request_constructor_sets_correct_fields() {
    let e = GatewayError::bad_request("missing field");
    assert_eq!(e.kind, ErrorKind::BadRequest);
    assert_eq!(e.http_status, Some(400));
    assert!(!e.retryable);
    assert!(matches!(e.fallback_hint, FallbackHint::Abort { .. }));
}

#[test]
fn rate_limited_constructor_uses_supplied_delay() {
    let e = GatewayError::rate_limited("slow down", 3_000);
    assert_eq!(e.kind, ErrorKind::RateLimit);
    assert_eq!(e.http_status, Some(429));
    assert!(e.retryable);
    match &e.fallback_hint {
        FallbackHint::Retry { delay_ms, .. } => assert_eq!(*delay_ms, 3_000),
        other => panic!("expected Retry, got {other:?}"),
    }
}

#[test]
fn server_error_is_retryable_and_triggers_fallback() {
    let e = GatewayError::server_error("internal failure");
    assert_eq!(e.kind, ErrorKind::ServerError);
    assert!(e.retryable);
    assert!(e.should_fallback());
}

#[test]
fn unauthorized_is_not_retryable_and_aborts() {
    let e = GatewayError::unauthorized("bad key");
    assert!(!e.is_retryable());
    assert!(!e.should_fallback());
    assert!(matches!(e.fallback_hint, FallbackHint::Abort { .. }));
}

// ── classify_upstream_error ──────────────────────────────────────────

#[test]
fn classify_upstream_401_parses_openai_body() {
    let body = r#"{"error":{"message":"Invalid API key","code":"invalid_api_key"}}"#;
    let e = classify_upstream_error(401, body, Some("openai"));
    assert_eq!(e.kind, ErrorKind::Authentication);
    assert_eq!(e.provider_name.as_deref(), Some("openai"));
    assert_eq!(e.message, "Invalid API key");
    assert_eq!(e.code.as_deref(), Some("invalid_api_key"));
    assert!(!e.retryable);
}

#[test]
fn classify_upstream_error_redacts_message_without_losing_classification() {
    let body = r#"{"error":{"message":"Invalid API key sk-proj-upstream-secret; token=secondary-secret","code":"invalid_api_key"}}"#;

    let e = classify_upstream_error(401, body, Some("openai"));

    assert_eq!(e.kind, ErrorKind::Authentication);
    assert_eq!(e.code.as_deref(), Some("invalid_api_key"));
    assert!(e.message.contains("Invalid API key"));
    assert!(!e.message.contains("sk-proj-upstream-secret"));
    assert!(!e.message.contains("secondary-secret"));
}

#[test]
fn classify_upstream_sanitizes_before_truncating_a_long_jwt() {
    let body = format!(
        "eyJ{}.{}.{}",
        "a".repeat(100),
        "b".repeat(100),
        "signature-secret-value"
    );

    let e = classify_upstream_error(418, &body, None);

    assert!(!e.message.contains("eyJ"));
    assert!(!e.message.contains("signature-secret-value"));
    assert!(e.message.contains("[REDACTED]"));
}

#[test]
fn classify_network_error_redacts_secrets_embedded_in_url() {
    let invalid_header = rquest::header::HeaderValue::from_bytes(b"\n")
        .expect_err("newline must be rejected as a header value");
    let error = rquest::Error::from(invalid_header).with_url(
        rquest::Url::parse("https://provider.invalid/models?token=network-secret-value")
            .expect("test URL must parse"),
    );

    let classified = classify_network_error(&error, Some("test-provider"));

    assert_eq!(classified.kind, ErrorKind::Unknown);
    assert!(!classified.message.contains("network-secret-value"));
    assert!(classified.message.contains("token=[REDACTED]"));
    assert!(classified.message.chars().count() <= PROVIDER_ERROR_MESSAGE_MAX_CHARS);
}

#[test]
fn classify_upstream_429_uses_retry_after_from_body() {
    let body = r#"{"error":{"message":"Rate limited","retry_after":10}}"#;
    let e = classify_upstream_error(429, body, None);
    assert_eq!(e.kind, ErrorKind::RateLimit);
    assert!(e.retryable);
    match &e.fallback_hint {
        FallbackHint::Retry { delay_ms, .. } => assert_eq!(*delay_ms, 10_000),
        other => panic!("expected Retry, got {other:?}"),
    }
}

#[test]
fn classify_upstream_429_falls_back_to_default_delay() {
    let body = r#"{"error":{"message":"Rate limited"}}"#;
    let e = classify_upstream_error(429, body, None);
    match &e.fallback_hint {
        FallbackHint::Retry { delay_ms, .. } => assert_eq!(*delay_ms, 5_000),
        other => panic!("expected Retry, got {other:?}"),
    }
}

#[test]
fn classify_upstream_500_triggers_provider_fallback() {
    let body = r#"{"message":"Internal server error"}"#;
    let e = classify_upstream_error(500, body, Some("anthropic"));
    assert_eq!(e.kind, ErrorKind::ServerError);
    assert!(e.retryable);
    assert!(e.should_fallback());
}

#[test]
fn classify_upstream_falls_through_to_keyword_scan_on_unmapped_status() {
    // 418 has no status mapping — fall through to body keyword scan.
    let body = "quota exceeded for this billing cycle";
    let e = classify_upstream_error(418, body, None);
    assert_eq!(e.kind, ErrorKind::InsufficientQuota);
}

#[test]
fn classify_upstream_fully_unknown_when_no_signals() {
    let e = classify_upstream_error(418, "I'm a teapot", None);
    assert_eq!(e.kind, ErrorKind::Unknown);
    assert!(!e.retryable);
}

#[test]
fn classify_upstream_deactivated_workspace_as_authentication() {
    let e = classify_upstream_error(402, r#"{"detail":{"code":"deactivated_workspace"}}"#, None);
    assert_eq!(e.kind, ErrorKind::Authentication);
    assert!(!e.retryable);
}

#[test]
fn classify_upstream_token_invalidated_as_authentication() {
    let e = classify_upstream_error(
        401,
        r#"{"error":{"message":"Your authentication token has been invalidated.","code":"token_invalidated"}}"#,
        None,
    );
    assert_eq!(e.kind, ErrorKind::Authentication);
    assert!(!e.retryable);
}

#[test]
fn classify_upstream_appid_no_auth_error_as_authentication() {
    let e = classify_upstream_error(
        400,
        "AppIdNoAuthError: xop35qwen2b is not authorized for this appid",
        Some("xfyun"),
    );
    assert_eq!(e.kind, ErrorKind::Authentication);
    assert!(!e.retryable);
}

// ── builder extensions ───────────────────────────────────────────────

#[test]
fn builder_methods_attach_fields() {
    let e = GatewayError::server_error("boom")
        .with_provider("groq")
        .with_code("EINTERNAL");
    assert_eq!(e.provider_name.as_deref(), Some("groq"));
    assert_eq!(e.code.as_deref(), Some("EINTERNAL"));
}

// ── FallbackHint serialisation ───────────────────────────────────────

#[test]
fn fallback_hint_retry_serialises_action_tag() {
    let hint = FallbackHint::Retry {
        delay_ms: 500,
        reason: "test".to_string(),
    };
    let json = serde_json::to_value(&hint).unwrap();
    assert_eq!(json["action"], "retry");
    assert_eq!(json["delay_ms"], 500);
}

#[test]
fn fallback_hint_fallback_provider_serialises_action_tag() {
    let hint = FallbackHint::FallbackProvider {
        reason: "provider down".to_string(),
    };
    let json = serde_json::to_value(&hint).unwrap();
    assert_eq!(json["action"], "fallback_provider");
}

#[test]
fn fallback_hint_abort_serialises_action_tag() {
    let hint = FallbackHint::Abort {
        reason: "bad creds".to_string(),
    };
    let json = serde_json::to_value(&hint).unwrap();
    assert_eq!(json["action"], "abort");
}
