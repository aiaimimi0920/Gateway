mod fixture;
mod targets;

use crate::error::{ErrorKind, FallbackHint};
use crate::retry::{execute_with_retry, execute_with_retry_after_admission_observed, RetryPolicy};
use axum::http::StatusCode;
use fixture::Server;
use std::time::Duration;
use targets::{client, Target};

const BODY: &str = r#"{"error":{"message":"fixture error","code":"fixture_code","retry_after":9}}"#;

#[tokio::test]
async fn generic_and_official_http_errors_preserve_header_precedence() {
    let client = client();
    for target in [
        Target::Buffered,
        Target::Stream,
        Target::Forced,
        Target::Json,
        Target::Binary,
        Target::OfficialBuffered,
        Target::OfficialStream,
        Target::OfficialForced,
        Target::Anthropic,
    ] {
        let server = Server::start(
            StatusCode::TOO_MANY_REQUESTS,
            &[
                ("retry-after-ms", "25"),
                ("x-ms-retry-after-ms", "30"),
                ("retry-after", "8"),
                ("set-cookie", "session=synthetic-header-secret"),
            ],
            BODY,
        )
        .await;
        let error = target.invoke(&client, &server.url).await.unwrap_err();
        assert_eq!(server.calls().len(), 1, "{target:?}: {error:?}");
        assert_eq!(error.kind, ErrorKind::RateLimit, "{target:?}");
        assert_eq!(error.code.as_deref(), Some("fixture_code"));
        assert!(
            matches!(
                error.fallback_hint,
                FallbackHint::Retry { delay_ms: 25, .. }
            ),
            "{target:?}: {error:?}"
        );
        assert!(!format!("{error:?}").contains("synthetic-header-secret"));
        server.finish().await;
    }
}

#[tokio::test]
async fn server_failure_honors_headers_without_promoting_permanent_errors() {
    let client = client();
    for (status, retryable) in [
        (503, true),
        (400, false),
        (401, false),
        (403, false),
        (404, false),
    ] {
        let server = Server::start(
            StatusCode::from_u16(status).unwrap(),
            &[("retry-after", "2")],
            BODY,
        )
        .await;
        let error = Target::Buffered
            .invoke(&client, &server.url)
            .await
            .unwrap_err();
        assert_eq!(
            crate::retry::should_retry(&error, &RetryPolicy::default()),
            retryable
        );
        assert_eq!(error.http_status, Some(status));
        assert_eq!(error.code.as_deref(), Some("fixture_code"));
        if retryable {
            assert!(matches!(
                error.fallback_hint,
                FallbackHint::Retry {
                    delay_ms: 2_000,
                    ..
                }
            ));
        }
        server.finish().await;
    }
}

#[tokio::test]
async fn invalid_headers_fall_back_to_body_without_losing_error_identity() {
    let server = Server::start(
        StatusCode::TOO_MANY_REQUESTS,
        &[("retry-after-ms", "-1"), ("retry-after", "bad")],
        BODY,
    )
    .await;
    let error = Target::Stream
        .invoke(&client(), &server.url)
        .await
        .unwrap_err();
    assert!(matches!(
        error.fallback_hint,
        FallbackHint::Retry {
            delay_ms: 9_000,
            ..
        }
    ));
    assert_eq!(error.message, "fixture error");
    assert_eq!(error.code.as_deref(), Some("fixture_code"));
    server.finish().await;
}

#[tokio::test]
async fn real_http_retry_apis_wait_for_the_transported_hint() {
    let client = client();
    for admitted in [false, true] {
        let server = Server::start(
            StatusCode::TOO_MANY_REQUESTS,
            &[("retry-after-ms", "25")],
            BODY,
        )
        .await;
        let policy = RetryPolicy {
            max_retries: 1,
            initial_delay: Duration::ZERO,
            ..Default::default()
        };
        let f = || Target::Buffered.invoke(&client, &server.url);
        let error = if admitted {
            execute_with_retry_after_admission_observed(f, || async { Ok(()) }, |_| {}, &policy)
                .await
        } else {
            execute_with_retry(f, &policy).await
        }
        .unwrap_err();
        let calls = server.calls();
        assert_eq!(calls.len(), 2);
        assert!(calls[1].duration_since(calls[0]) >= Duration::from_millis(25));
        assert_eq!(error.code.as_deref(), Some("fixture_code"));
        server.finish().await;
    }
}
