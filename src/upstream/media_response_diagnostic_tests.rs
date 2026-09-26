use super::{lumalabs_response_helpers, suno_response_helpers, udio_response_helpers};
use crate::error::{ErrorKind, FallbackHint, GatewayError};
use axum::response::IntoResponse;
use serde_json::{json, Value};

const PROVIDER: &str = "selected_media_provider";
const OMITTED: &str = "[upstream detail omitted: safe processing limit exceeded]";
const RATE_LIMIT_BODY: &str =
    r#"{"error":{"message":"retry later","code":"body_code"},"retry_after":2}"#;

#[derive(Clone, Copy, Debug)]
enum MediaProvider {
    LumaLabs,
    Suno,
    Udio,
}

const PROVIDERS: [MediaProvider; 3] = [
    MediaProvider::LumaLabs,
    MediaProvider::Suno,
    MediaProvider::Udio,
];

impl MediaProvider {
    fn failure(self, message: Option<&str>, status: u16, body: &str) -> GatewayError {
        let stdout = json!({
            "ok": false,
            "status": 503,
            "error": {
                "code": "worker_code",
                "message": message,
                "status": status,
                "body": body,
            },
        })
        .to_string();
        match self {
            Self::LumaLabs => {
                lumalabs_response_helpers::parse_lumalabs_browser_worker_verified_output(
                    &stdout,
                    "stderr fallback",
                    PROVIDER,
                )
                .expect_err("LumaLabs worker failure")
            }
            Self::Suno => suno_response_helpers::parse_suno_browser_worker_verified_output(
                &stdout,
                "stderr fallback",
                PROVIDER,
            )
            .expect_err("Suno worker failure"),
            Self::Udio => udio_response_helpers::parse_udio_browser_worker_verified_output(
                &stdout,
                "stderr fallback",
                PROVIDER,
            )
            .expect_err("Udio worker failure"),
        }
    }
}

type SunoParser = fn(&str) -> Result<Value, GatewayError>;
const SUNO_ENDPOINTS: [(SunoParser, &str); 3] = [
    (
        suno_response_helpers::parse_suno_challenge_probe_body,
        "suno_invalid_challenge_probe_body",
    ),
    (
        suno_response_helpers::parse_suno_generation_body,
        "suno_invalid_generation_body",
    ),
    (
        suno_response_helpers::parse_suno_feed_body,
        "suno_invalid_feed_body",
    ),
];

#[test]
fn worker_override_redacts_auth_without_changing_metadata() {
    for provider in PROVIDERS {
        for (message, secret) in [
            (
                "Authorization: Bearer worker-bearer-value",
                "worker-bearer-value",
            ),
            ("Cookie: sid=worker-cookie-value", "worker-cookie-value"),
            (
                "api_key=\"worker key with spaces\"",
                "worker key with spaces",
            ),
            ("X-API-Key: worker-api-value", "worker-api-value"),
        ] {
            let error = provider.failure(Some(message), 429, RATE_LIMIT_BODY);
            assert!(!error.message.contains(secret), "{provider:?}");
            assert!(error.message.contains("[REDACTED]"), "{provider:?}");
            assert_eq!(error.kind, ErrorKind::RateLimit);
            assert_eq!(error.http_status, Some(429));
            assert_eq!(error.code.as_deref(), Some("worker_code"));
            assert_eq!(error.provider_name.as_deref(), Some(PROVIDER));
            assert!(error.retryable);
            assert!(matches!(
                error.fallback_hint,
                FallbackHint::Retry {
                    delay_ms: 2_000,
                    ..
                }
            ));
        }
    }
}

#[test]
fn worker_override_removes_controls_and_bounds_unicode() {
    let message = " \u{754c}\n\t\0\u{001b}".repeat(250);
    for provider in PROVIDERS {
        let error = provider.failure(Some(&message), 429, RATE_LIMIT_BODY);
        assert!(!error.message.chars().any(char::is_control), "{provider:?}");
        assert!(error.message.starts_with('\u{754c}'), "{provider:?}");
        assert!(error.message.chars().count() <= 512, "{provider:?}");
        assert!(error.message.ends_with("..."), "{provider:?}");
    }
}

#[test]
fn worker_override_budget_uses_bytes_and_admits_exact_limit() {
    for at_limit in [
        "a".repeat(16 * 1024),
        format!("{}!", "\u{754c}".repeat(5_461)),
    ] {
        assert_eq!(at_limit.len(), 16 * 1024);
        let over_limit = format!("{at_limit}?");
        for provider in PROVIDERS {
            let admitted = provider.failure(Some(&at_limit), 429, RATE_LIMIT_BODY);
            assert!(!admitted.message.contains(OMITTED), "{provider:?}");
            assert!(!admitted.message.is_empty(), "{provider:?}");
            let omitted = provider.failure(Some(&over_limit), 429, RATE_LIMIT_BODY);
            assert_eq!(omitted.message, OMITTED, "{provider:?}");
        }
    }
}

#[test]
fn worker_classification_uses_raw_body_before_sanitizing_override() {
    let body = r#"{"error":{"message":"api_key=\"rate limit exceeded\""},"retry_after":2}"#;
    for provider in PROVIDERS {
        let error = provider.failure(Some("upstream denied"), 400, body);
        assert_eq!(error.kind, ErrorKind::RateLimit, "{provider:?}");
        assert_eq!(error.http_status, Some(400));
        assert_eq!(error.message, "upstream denied");
        assert!(matches!(
            error.fallback_hint,
            FallbackHint::Retry {
                delay_ms: 2_000,
                ..
            }
        ));
    }
}

#[test]
fn empty_worker_message_still_overrides_body_message() {
    for provider in PROVIDERS {
        let empty = provider.failure(Some(""), 429, RATE_LIMIT_BODY);
        assert_eq!(empty.message, "", "{provider:?}");
        let absent = provider.failure(None, 429, RATE_LIMIT_BODY);
        assert_eq!(absent.message, "retry later", "{provider:?}");
    }
}

#[tokio::test]
async fn worker_error_http_response_preserves_rate_limit_headers() {
    for provider in PROVIDERS {
        let error = provider.failure(Some("Cookie: sid=http-worker-secret"), 429, RATE_LIMIT_BODY);
        let response = error.into_response();
        assert_eq!(response.status(), 429);
        assert_eq!(response.headers()["retry-after"], "2");
        let bytes = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .expect("bounded error body");
        let body: Value = serde_json::from_slice(&bytes).expect("error JSON");
        let message = body["error"]["message"].as_str().expect("message");
        assert!(!message.contains("http-worker-secret"), "{provider:?}");
        assert!(message.contains("[REDACTED]"), "{provider:?}");
        assert_eq!(body["error"]["type"], "RateLimit");
        assert_eq!(body["error"]["code"], "worker_code");
    }
}

#[test]
fn suno_json_error_redacts_before_legacy_preview_boundary() {
    let secret = "preview_first_value preview_second_value preview_third_value";
    let body = format!("{} api_key=\"{secret}\"", "x".repeat(150));
    assert!(body.len() > 200);
    for (parse, code) in SUNO_ENDPOINTS {
        let error = parse(&body).expect_err("malformed JSON");
        assert!(!error.message.contains("preview_"), "{code}");
        assert!(error.message.contains("[REDACTED]"), "{code}");
        assert!(error.message.chars().count() <= 512);
        assert_eq!(error.code.as_deref(), Some(code));
    }
}

#[test]
fn suno_json_error_omits_oversized_body() {
    let body = format!(
        "{} api_key=\"oversized-body-secret\"",
        "x".repeat(16 * 1024 + 1)
    );
    for (parse, code) in SUNO_ENDPOINTS {
        let error = parse(&body).expect_err("malformed oversized JSON");
        assert!(error.message.contains(OMITTED), "{code}");
        assert!(!error.message.contains("xxxxxxxx"), "{code}");
        assert!(!error.message.contains("oversized-body-secret"), "{code}");
        assert!(error.message.chars().count() <= 512);
    }
}

#[test]
fn suno_json_error_keeps_empty_body_and_endpoint_codes() {
    for (parse, code) in SUNO_ENDPOINTS {
        let error = parse(" \r\n\t ").expect_err("empty JSON body");
        assert!(error.message.contains("<empty body>"));
        assert_eq!(error.kind, ErrorKind::ServerError);
        assert_eq!(error.http_status, Some(500));
        assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
        assert_eq!(error.code.as_deref(), Some(code));
    }
}

#[test]
fn suno_valid_json_is_returned_without_sanitization() {
    for expected in [
        Value::Null,
        json!(["ordinary payload", 1, null]),
        json!("Bearer successful-payload-value"),
        json!({
            "api_key": "successful-payload-secret",
            "message": "Cookie: preserve-successful-payload",
            "data": "\u{754c}".repeat(10_000),
        }),
    ] {
        let source = expected.to_string();
        for (parse, code) in SUNO_ENDPOINTS {
            assert_eq!(parse(&source).expect("valid JSON"), expected, "{code}");
        }
    }
}
