//! Production-library error exports are safe; native failure bytes remain transparent.
use bytes::Bytes;
use futures::{stream, Stream, StreamExt};
use neuro_gateway::{
    error::{ErrorKind, GatewayError},
    protocol::{
        responses::{
            accumulate_responses_sse_stream_with_error, require_responses_completion,
            translate_responses_sse_to_openai_chat_with_error,
        },
        stream_error::StreamError,
    },
    provider_failure::{classify_provider_failure, ProviderFailureClass},
    upstream::stream::{StreamTermination, TrackedStream},
};
use serde_json::{json, Value};
use std::{
    error::Error as StdError,
    sync::{Arc, Mutex},
};

type Error = StreamError<std::io::Error>;
const PRIVATE: &str = "私有-private-upstream-detail";
const PARTIAL: &str = "data: {\"type\":\"response.output_text.delta\",\"delta\":\"partial\"}\n\n";

fn frame(event: &str, mut payload: Value) -> String {
    payload["type"] = json!(event);
    format!("event: {event}\ndata: {payload}\n\n")
}

fn source(wire: &str) -> impl Stream<Item = Result<Bytes, Error>> + Send + 'static {
    stream::iter(vec![Ok(Bytes::copy_from_slice(wire.as_bytes()))])
}

fn assert_safe(error: &(dyn StdError + 'static)) {
    let mut next = Some(error);
    for _ in 0..8 {
        let Some(error) = next else { return };
        for message in [error.to_string(), format!("{error:?}")] {
            assert!(!message.contains(PRIVATE), "Private provider data escaped");
            assert!(message.len() < 512, "Error export is not locally bounded");
        }
        next = error.source();
    }
    assert!(next.is_none(), "Unexpected error source cycle");
}

#[tokio::test]
async fn buffered_failure_never_exports_provider_prose_or_unrecognized_codes() {
    let detail = format!("{} invalid_api_key rate_limit", PRIVATE.repeat(1024));
    for payload in [
        json!({"error":{"message":detail,"code":detail,"type":"invalid_api_key"}}),
        json!({"response":{"error":{"message":detail,"code":detail}}}),
        json!({"error":detail}),
        json!({"message":detail}),
    ] {
        for event in ["error", "response.failed", "response.cancelled"] {
            let wire = format!("{PARTIAL}{}", frame(event, payload.clone()));
            let error = accumulate_responses_sse_stream_with_error(Box::pin(source(&wire)), "test")
                .await
                .expect_err("Explicit failure cannot return partial success");
            assert_eq!(
                error.code.as_deref(),
                Some("responses_stream_upstream_error")
            );
            assert_eq!(
                error.message,
                "Responses upstream stream failed before completion."
            );
            assert_eq!(
                classify_provider_failure(None, error.code.as_deref(), Some(&error.message)).class,
                ProviderFailureClass::Unknown
            );
            assert_safe(&error);
        }
    }
}

#[tokio::test]
async fn structured_categories_keep_existing_error_policy_without_provider_text() {
    use ProviderFailureClass::*;
    for (code, expected) in [
        ("invalid_api_key", CredentialInvalid),
        ("token_expired", CredentialExpired),
        ("model_not_found", ModelUnsupported),
        ("quota_exceeded", QuotaExhausted),
        ("rate_limit_exceeded", RateLimited),
        ("content_filter", ContentRejected),
    ] {
        let wire = frame(
            "response.failed",
            json!({"error":null,"response":{"error":{
            "code":code,"message":format!("{PRIVATE} invalid_api_key rate_limit")}}}),
        );
        let error = accumulate_responses_sse_stream_with_error(Box::pin(source(&wire)), "test")
            .await
            .unwrap_err();
        assert_safe(&error);
        assert_eq!(
            classify_provider_failure(None, error.code.as_deref(), Some(&error.message)).class,
            expected
        );
        let baseline = GatewayError::server_error("baseline");
        assert_eq!(error.kind, ErrorKind::ServerError);
        assert_eq!(error.http_status, Some(500));
        assert_eq!(error.retryable, baseline.retryable);
        assert_eq!(
            serde_json::to_value(&error.fallback_hint).unwrap(),
            serde_json::to_value(&baseline.fallback_hint).unwrap()
        );
        assert!(error.upstream_retry_after_ms.is_none() && error.native_transport.is_none());
    }
}

#[tokio::test]
async fn translated_byte_fragmented_failures_interrupt_without_success_or_private_errors() {
    for event in ["error", "response.failed", "response.cancelled"] {
        let wire = format!(
            "{PARTIAL}{}",
            frame(
                event,
                json!({"response":{"error":{
            "code":"quota_exceeded","message":PRIVATE}}})
            )
        );
        // Fragment inside multibyte UTF-8 as well as SSE/JSON boundaries.
        let chunks: Vec<Result<Bytes, Error>> = wire
            .as_bytes()
            .iter()
            .map(|byte| Ok(Bytes::copy_from_slice(&[*byte])))
            .collect();
        let translated =
            translate_responses_sse_to_openai_chat_with_error(stream::iter(chunks), "test".into());
        let observed = Arc::new(Mutex::new(Vec::new()));
        let capture = observed.clone();
        let items = TrackedStream::new_with_error(translated, move |metrics, success| {
            capture.lock().unwrap().push((metrics.termination, success));
        })
        .collect::<Vec<_>>()
        .await;
        let errors: Vec<_> = items
            .iter()
            .filter_map(|item| item.as_ref().err())
            .collect();
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], Error::Protocol(_)));
        assert_safe(errors[0]);
        assert!(errors[0]
            .to_string()
            .contains("responses_stream_upstream_error"));
        assert_eq!(
            classify_provider_failure(None, None, Some(&errors[0].to_string())).class,
            ProviderFailureClass::QuotaExhausted
        );
        let output: Vec<_> = items
            .into_iter()
            .filter_map(Result::ok)
            .flat_map(|bytes| bytes.to_vec())
            .collect();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("partial"));
        assert!(!output.contains(PRIVATE) && !output.contains("[DONE]"));
        for data in output
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
        {
            let payload: Value = serde_json::from_str(data).unwrap();
            assert!(payload["choices"][0]["finish_reason"].is_null());
        }
        assert_eq!(
            &*observed.lock().unwrap(),
            &[(Some(StreamTermination::Interrupted), false)]
        );
    }
}

#[tokio::test]
async fn native_failure_retains_original_wire_but_only_a_fixed_typed_error() {
    let failed = Bytes::from(frame(
        "response.failed",
        json!({"response":{"error":{
        "code":format!("{PRIVATE}-invalid_api_key"),"message":PRIVATE}}}),
    ));
    let pointer = failed.as_ptr();
    let source = stream::iter(vec![
        Ok::<_, Error>(Bytes::from_static(PARTIAL.as_bytes())),
        Ok(failed.clone()),
        Ok(Bytes::from(frame(
            "response.completed",
            json!({"response":{"status":"completed","output":[]}}),
        ))),
    ]);
    let items = require_responses_completion(source)
        .collect::<Vec<_>>()
        .await;
    assert_eq!(
        items.len(),
        3,
        "Partial, original failure, then exactly one error"
    );
    assert_eq!(items[0].as_ref().unwrap().as_ref(), PARTIAL.as_bytes());
    let raw_failure = items[1].as_ref().unwrap();
    assert_eq!(raw_failure.as_ptr(), pointer);
    assert_eq!(raw_failure, &failed);
    // Transparent native bytes are intentionally not claimed to be redacted.
    assert!(std::str::from_utf8(raw_failure).unwrap().contains(PRIVATE));
    let error = items[2].as_ref().unwrap_err();
    assert!(matches!(error, Error::Protocol(_)));
    assert_safe(error);
    assert!(error
        .to_string()
        .contains("responses_stream_upstream_error"));
    assert_eq!(
        classify_provider_failure(None, None, Some(&error.to_string())).class,
        ProviderFailureClass::Unknown
    );
}
