//! Completion errors never retain provider data, while exact code categories remain usable.
use super::super::terminal::{Completion, CompletionError};
use super::*;
use crate::{
    error::{ErrorKind, GatewayError},
    provider_failure::{classify_provider_failure, ProviderFailureClass},
};
use std::{
    error::Error,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    task::Poll,
};

fn failure(mut payload: Value) -> CompletionError {
    payload["type"] = json!("response.failed");
    Completion::default()
        .observe(&SseFrame {
            event_name: Some("response.failed".into()),
            data: payload.to_string(),
        })
        .expect_err("Explicit failure must never complete")
}
fn assert_safe(error: &(dyn Error + 'static), private: &str) {
    let mut current = Some(error);
    for _ in 0..8 {
        let Some(error) = current else { return };
        for text in [error.to_string(), format!("{error:?}")] {
            assert!(
                !text.contains(private),
                "Provider data retained in error chain"
            );
            assert!(
                text.len() < 512,
                "Completion error must have a constant local bound"
            );
        }
        current = error.source();
    }
    assert!(current.is_none(), "Unexpected unbounded error chain");
}

#[test]
fn responses_terminal_private_prose_codes_and_source_chains_are_fixed_and_bounded() {
    let private = "私有-upstream-marker";
    let text = format!(
        "{}\nCookie: synthetic-cookie invalid_api_key rate_limit",
        private.repeat(1024)
    );
    for payload in [
        json!({"error":{"message":text,"code":text,"type":text}}),
        json!({"response":{"error":{"message":text,"code":text}}}),
        json!({"error":text}),
        json!({"message":text}),
        json!({"error":{"message":text}}),
    ] {
        let error = failure(payload);
        assert!(!format!("{error:?}").contains(private));
        let gateway = error.clone().gateway();
        assert_eq!(
            gateway.message,
            "Responses upstream stream failed before completion."
        );
        assert_eq!(
            gateway.code.as_deref(),
            Some("responses_stream_upstream_error")
        );
        assert_safe(&gateway, private);
        let protocol = error.protocol();
        assert!(protocol.is_upstream_failure());
        assert_safe(&protocol, private);
        assert_eq!(
            classify_provider_failure(None, gateway.code.as_deref(), Some(&gateway.message)).class,
            ProviderFailureClass::Unknown
        );
    }
}

#[test]
fn responses_terminal_exact_structured_categories_preserve_classification_not_retry_policy() {
    use ProviderFailureClass::*;
    for (codes, expected) in [
        (
            &[
                "invalid_api_key",
                "invalid_token",
                "authentication_error",
                "unauthorized",
            ][..],
            CredentialInvalid,
        ),
        (
            &["token_expired", "expired_token", "session_expired"][..],
            CredentialExpired,
        ),
        (
            &[
                "model_not_found",
                "model_not_supported",
                "unsupported_model",
            ][..],
            ModelUnsupported,
        ),
        (
            &[
                "insufficient_quota",
                "quota_exceeded",
                "quota_exhausted",
                "billing_hard_limit_reached",
            ][..],
            QuotaExhausted,
        ),
        (
            &[
                "rate_limit_exceeded",
                "rate_limit_error",
                "rate_limited",
                "too_many_requests",
            ][..],
            RateLimited,
        ),
        (
            &[
                "content_filter",
                "content_policy_violation",
                "safety_violation",
                "content_filter_error",
            ][..],
            ContentRejected,
        ),
    ] {
        for code in codes {
            for field in ["code", "type"] {
                let private = "private-upstream-detail";
                // Misleading prose must not override the exact structured category.
                let mut detail = json!({"message":format!("{private} invalid_api_key rate_limit")});
                detail[field] = json!(code);
                let error = failure(json!({"response":{"error":detail}}));
                let gateway = error.clone().gateway();
                assert_eq!(gateway.kind, ErrorKind::ServerError);
                assert_eq!(gateway.http_status, Some(500));
                assert_eq!(
                    gateway.code.as_deref(),
                    Some("responses_stream_upstream_error")
                );
                let baseline = GatewayError::server_error("baseline");
                assert_eq!(gateway.retryable, baseline.retryable);
                assert_eq!(
                    serde_json::to_value(&gateway.fallback_hint).unwrap(),
                    serde_json::to_value(&baseline.fallback_hint).unwrap()
                );
                assert!(
                    gateway.upstream_retry_after_ms.is_none() && gateway.native_transport.is_none()
                );
                assert_eq!(
                    classify_provider_failure(
                        None,
                        gateway.code.as_deref(),
                        Some(&gateway.message)
                    )
                    .class,
                    expected
                );
                let protocol = error.protocol();
                assert!(protocol.is_upstream_failure());
                assert_eq!(
                    classify_provider_failure(None, None, Some(&protocol.to_string())).class,
                    expected
                );
                assert_safe(&gateway, private);
                assert_safe(&protocol, private);
            }
        }
    }
}

#[test]
fn responses_terminal_unknown_or_conflicting_codes_cannot_inject_health_categories() {
    for code in [
        json!("invalid_api_key-private"),
        json!(" rate_limit_exceeded"),
        json!("RATE_LIMIT_EXCEEDED"),
        json!("private\ninvalid_token"),
        json!(null),
        json!(["invalid_api_key"]),
        json!({"type":"invalid_api_key"}),
    ] {
        let gateway = failure(json!({"error":{"code":code,"type":"invalid_api_key","message":"invalid_api_key private"}})).gateway();
        assert_eq!(
            gateway.message,
            "Responses upstream stream failed before completion."
        );
    }
    let gateway = failure(json!({"error":null,"response":{"error":{"code":"rate_limit_exceeded","message":"private"}}})).gateway();
    assert_eq!(
        classify_provider_failure(None, None, Some(&gateway.message)).class,
        ProviderFailureClass::RateLimited
    );
}

struct DropProbe(Arc<AtomicBool>);
impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn responses_terminal_fragmented_failure_stops_source_without_success_or_private_output() {
    for event in ["error", "response.failed", "response.cancelled"] {
        let private = "私有-private-upstream-detail";
        let first = format_sse_event(
            None,
            &json!({"type":"response.output_text.delta","delta":"partial"}).to_string(),
        );
        let failing = format_sse_event(
            Some(event),
            &json!({"type":event,"response":{"error":{"code":"quota_exceeded","message":private}}})
                .to_string(),
        );
        let mut chunks = std::collections::VecDeque::from([Bytes::from(first)]);
        chunks.extend(
            failing
                .as_bytes()
                .iter()
                .map(|byte| Bytes::copy_from_slice(&[*byte])),
        );
        let expected_polls = chunks.len();
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = calls.clone();
        let dropped = Arc::new(AtomicBool::new(false));
        let probe = DropProbe(dropped.clone());
        let source = futures::stream::poll_fn(move |_| {
            let _keep_alive = &probe;
            counted.fetch_add(1, Ordering::SeqCst);
            Poll::Ready(Some(Ok::<_, rquest::Error>(
                chunks
                    .pop_front()
                    .expect("Polled upstream after terminal error"),
            )))
        });
        let (output, errors) = collect_stream_output(translate_responses_sse_to_openai_chat(
            source,
            "synthetic-model".into(),
        ))
        .await;
        assert!(output.contains("partial"));
        assert!(!output.contains(private) && !output.contains("[DONE]"));
        for data in output
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
        {
            let payload: Value = serde_json::from_str(data).unwrap();
            assert!(payload["choices"][0]["finish_reason"].is_null());
        }
        assert_eq!(errors.len(), 1);
        assert!(
            errors[0].contains("responses_stream_upstream_error")
                && errors[0].contains("quota_exhausted")
        );
        assert!(!errors[0].contains(private));
        assert_eq!(calls.load(Ordering::SeqCst), expected_polls);
        assert!(dropped.load(Ordering::SeqCst));
    }
}
