//! Genuine modeled/unmodeled AWS failures remain failures after HTTP 200; never replay inference.
#![cfg(feature = "family-bedrock-converse-official-api")]
#[path = "bedrock_exception_contract/routes.rs"]
mod routes;
#[path = "bedrock_exception_contract/runtime.rs"]
mod runtime;
#[path = "console_contract_support/mod.rs"]
mod support;
#[path = "bedrock_upstream_eventstream_contract/fixtures.rs"]
mod wire;

use neuro_gateway::{
    error::{ErrorKind, FallbackHint},
    upstream::bedrock_converse_official_api_common::accumulate_stream,
};
use serde_json::json;

const PRIVATE: &str = "private-fixture-secret";
const CASES: &[(&str, &str, u16, ErrorKind)] = &[
    (
        "internalServerException",
        "bedrock_internal_server_exception",
        500,
        ErrorKind::ServerError,
    ),
    (
        "modelStreamErrorException",
        "bedrock_model_stream_error_exception",
        424,
        ErrorKind::ServerError,
    ),
    (
        "validationException",
        "bedrock_validation_exception",
        400,
        ErrorKind::BadRequest,
    ),
    (
        "throttlingException",
        "bedrock_throttling_exception",
        429,
        ErrorKind::RateLimit,
    ),
    (
        "serviceUnavailableException",
        "bedrock_service_unavailable_exception",
        503,
        ErrorKind::ServiceUnavailable,
    ),
];

fn failure(kind: &str) -> Vec<u8> {
    wire::frame(
        &[
            (":message-type", "exception"),
            (":exception-type", kind),
            (":content-type", "application/json"),
        ],
        &json!({"message":PRIVATE,"originalMessage":PRIVATE,"originalStatusCode":401}),
    )
}

#[tokio::test]
async fn modeled_failures_have_safe_semantic_classification_without_replay_permission() {
    for &(name, code, status, kind) in CASES {
        let mut sequence = wire::sequence();
        sequence.push(failure(name)); // Even a complete-looking answer is not successful before EOF.
        let error = accumulate_stream(wire::response(sequence), "fixture-model")
            .await
            .unwrap_err();
        assert_eq!(error.code.as_deref(), Some(code), "{name}");
        assert_eq!(error.http_status, Some(status), "{name}");
        assert_eq!(error.kind, kind, "{name}");
        assert!(!error.retryable && matches!(error.fallback_hint, FallbackHint::Abort { .. }));
        assert!(!format!("{error:?}").contains(PRIVATE));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("bedrock_converse_compatible")
        );
    }
}

#[tokio::test]
async fn unknown_failures_are_sanitized_and_never_grant_replay() {
    for bytes in [
        failure(PRIVATE),
        wire::frame(
            &[
                (":message-type", "error"),
                (":error-code", PRIVATE),
                (":error-message", PRIVATE),
            ],
            &json!({}),
        ),
    ] {
        let error = accumulate_stream(wire::response(vec![bytes]), "fixture-model")
            .await
            .unwrap_err();
        assert_eq!(error.code.as_deref(), Some("bedrock_stream_upstream_error"));
        assert_eq!(error.http_status, Some(502));
        assert!(!error.retryable && matches!(error.fallback_hint, FallbackHint::Abort { .. }));
        assert!(!format!("{error:?}").contains(PRIVATE));
    }
}

#[tokio::test]
async fn exception_payload_and_discriminator_conflicts_are_protocol_errors_not_modeled_evidence() {
    for body in [
        json!(null),
        json!([]),
        json!({"message":false}),
        json!({"message":PRIVATE,"originalStatusCode":1.5}),
    ] {
        let bytes = wire::frame(
            &[
                (":message-type", "exception"),
                (":exception-type", "modelStreamErrorException"),
            ],
            &body,
        );
        let error = accumulate_stream(wire::response(vec![bytes]), "fixture-model")
            .await
            .unwrap_err();
        assert_eq!(error.code.as_deref(), Some("bedrock_stream_invalid"));
        assert!(!format!("{error:?}").contains(PRIVATE));
    }
    for headers in [
        vec![
            (":message-type", "exception"),
            (":exception-type", "throttlingException"),
            (":event-type", "messageStart"),
        ],
        vec![
            (":message-type", "exception"),
            (":exception-type", "throttlingException"),
            (":error-code", PRIVATE),
        ],
        vec![
            (":message-type", "error"),
            (":error-code", PRIVATE),
            (":error-message", PRIVATE),
            (":exception-type", "throttlingException"),
        ],
        vec![(":message-type", "error"), (":error-code", PRIVATE)],
    ] {
        let error = accumulate_stream(
            wire::response(vec![wire::frame(&headers, &json!({}))]),
            "fixture-model",
        )
        .await
        .unwrap_err();
        assert_eq!(error.code.as_deref(), Some("bedrock_stream_invalid"));
    }
}

#[tokio::test]
async fn optional_exception_fields_remain_optional_and_unknown_members_do_not_leak() {
    for body in [
        json!({}),
        json!({"message":null,"originalStatusCode":null,"originalMessage":null}),
        json!({"futureField":PRIVATE,"originalStatusCode":-1}),
    ] {
        let bytes = wire::frame(
            &[
                (":message-type", "exception"),
                (":exception-type", "modelStreamErrorException"),
            ],
            &body,
        );
        let error = accumulate_stream(wire::response(vec![bytes]), "fixture-model")
            .await
            .unwrap_err();
        assert_eq!(
            error.code.as_deref(),
            Some("bedrock_model_stream_error_exception")
        );
        assert!(!format!("{error:?}").contains(PRIVATE));
    }
}

#[tokio::test]
async fn only_the_selected_modeled_shape_validates_its_own_optional_members() {
    for (kind, body, code) in [
        (
            "validationException",
            json!({"message":"escaped\nprivate-fixture-secret","originalStatusCode":"unknown member"}),
            "bedrock_validation_exception",
        ),
        (
            "futureException",
            json!({"message":false,"originalStatusCode":[]}),
            "bedrock_stream_upstream_error",
        ),
    ] {
        let bytes = wire::frame(
            &[(":message-type", "exception"), (":exception-type", kind)],
            &body,
        );
        let error = accumulate_stream(wire::response(vec![bytes]), "fixture-model")
            .await
            .unwrap_err();
        assert_eq!(error.code.as_deref(), Some(code));
        assert!(!format!("{error:?}").contains(PRIVATE));
    }
    let bytes = wire::frame(
        &[
            (":message-type", "exception"),
            (":exception-type", "modelStreamErrorException"),
        ],
        &json!([null, null, null]),
    );
    let error = accumulate_stream(wire::response(vec![bytes]), "fixture-model")
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("bedrock_stream_invalid"));
}
