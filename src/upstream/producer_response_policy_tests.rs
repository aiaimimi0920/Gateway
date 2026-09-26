use super::*;
use crate::protocol::producer::{normalize_image_generations, PRODUCER_IMAGE_DEFAULT_MODEL};

#[test]
fn classify_producer_image_request_failure_preserves_upstream_contract() {
    let error = classify_producer_image_request_failure(502, "gateway time-out");

    assert_eq!(error.http_status, Some(502));
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
}

#[test]
fn finalize_producer_image_retry_error_falls_back_to_exhausted_contract() {
    let error = finalize_producer_image_retry_error(None, "producer_compatible");

    assert_eq!(error.http_status, Some(500));
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    assert_eq!(
        error.code.as_deref(),
        Some("producer_image_retry_exhausted")
    );
}

#[test]
fn finalize_producer_image_retry_error_preserves_last_error_contract() {
    let result = finalize_producer_image_retry_error(
        Some(
            GatewayError::service_unavailable("upstream unavailable")
                .with_code("producer_upstream_failed"),
        ),
        "producer_compatible",
    );

    assert_eq!(result.http_status, Some(503));
    assert_eq!(result.code.as_deref(), Some("producer_upstream_failed"));
    assert_eq!(result.message, "upstream unavailable");
}

#[test]
fn resolve_producer_image_attempt_preserves_success_contract() {
    let req = normalize_image_generations(json!({
        "prompt": "holographic album cover",
        "type": "clip"
    }))
    .unwrap();

    let result = resolve_producer_image_attempt(
        200,
        "{\"image_id\":\"img-42\"}",
        &req,
        PRODUCER_IMAGE_DEFAULT_MODEL,
        Some("e30.eyJpc3MiOiJodHRwczovL2RlbW8tcHJvamVjdC5zdXBhYmFzZS5jby9hdXRoL3YxIiwic3ViIjoidXNlci0xMjMifQ.sig"),
        "producer_compatible",
        true,
    );

    match result {
        ProducerImageAttemptResolution::Success(body) => {
            assert_eq!(body["object"], "image.generation");
            assert_eq!(body["image_id"], "img-42");
        }
        ProducerImageAttemptResolution::Retry(error)
        | ProducerImageAttemptResolution::Fail(error) => {
            panic!("expected success, got error: {}", error.message)
        }
    }
}

#[test]
fn resolve_producer_image_attempt_marks_retryable_failure_contract() {
    let req = normalize_image_generations(json!({
        "prompt": "holographic album cover",
        "type": "clip"
    }))
    .unwrap();

    let result = resolve_producer_image_attempt(
        503,
        "service unavailable",
        &req,
        PRODUCER_IMAGE_DEFAULT_MODEL,
        None,
        "producer_compatible",
        true,
    );

    match result {
        ProducerImageAttemptResolution::Retry(error) => {
            assert_eq!(error.http_status, Some(503));
            assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
        }
        ProducerImageAttemptResolution::Success(_) => panic!("expected retry, got success"),
        ProducerImageAttemptResolution::Fail(error) => {
            panic!("expected retry, got fail: {}", error.message)
        }
    }
}

#[test]
fn resolve_producer_image_attempt_preserves_terminal_failure_contract() {
    let req = normalize_image_generations(json!({
        "prompt": "holographic album cover",
        "type": "clip"
    }))
    .unwrap();

    let result = resolve_producer_image_attempt(
        400,
        "validation failed",
        &req,
        PRODUCER_IMAGE_DEFAULT_MODEL,
        None,
        "producer_compatible",
        true,
    );

    match result {
        ProducerImageAttemptResolution::Fail(error) => {
            assert_eq!(error.http_status, Some(400));
            assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
        }
        ProducerImageAttemptResolution::Success(_) => panic!("expected fail, got success"),
        ProducerImageAttemptResolution::Retry(error) => {
            panic!("expected fail, got retry: {}", error.message)
        }
    }
}

#[test]
fn parse_producer_music_stream_http_response_reads_success_contract() {
    let body = parse_producer_music_stream_http_response(
        200,
        [
            "event: conversation_id",
            "data: {\"id\":\"conv-music-1\"}",
            "",
            "event: generated-title",
            "data: {\"title\":\"Neon Dreams\"}",
            "",
            "event: complete",
            "data: {}",
            "",
        ]
        .join("\n")
        .as_str(),
        "producer_compatible",
        PRODUCER_IMAGE_DEFAULT_MODEL,
        "music-job-1",
    )
    .expect("stream response should parse");

    assert_eq!(body["object"], "music.generation");
    assert_eq!(body["job_id"], "music-job-1");
    assert_eq!(body["generated_title"], "Neon Dreams");
    assert_eq!(body["completed"], true);
}

#[test]
fn parse_producer_music_stream_http_response_preserves_failure_contract() {
    let error = parse_producer_music_stream_http_response(
        503,
        "service unavailable",
        "producer_compatible",
        PRODUCER_IMAGE_DEFAULT_MODEL,
        "music-job-1",
    )
    .expect_err("non-2xx should fail");

    assert_eq!(error.http_status, Some(503));
    assert!(error.message.contains("service unavailable"));
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
}

#[test]
fn should_fallback_producer_video_http_error_respects_auth_and_status_contract() {
    assert!(!should_fallback_producer_video_http_error(
        &GatewayError::unauthorized("auth blocked").with_code("producer_http_video_timeout")
    ));
    assert!(should_fallback_producer_video_http_error(
        &GatewayError::service_unavailable("upstream unavailable")
            .with_code("producer_http_video_timeout")
    ));
    assert!(!should_fallback_producer_video_http_error(
        &GatewayError::server_error("final producer failure")
            .with_code("producer_http_video_failed")
    ));
}

#[test]
fn should_retry_producer_image_request_matches_timeout_contract() {
    assert!(should_retry_producer_image_request(
        503,
        "service unavailable"
    ));
    assert!(should_retry_producer_image_request(
        500,
        "Gateway time-out while rendering"
    ));
    assert!(should_retry_producer_image_request(
        500,
        "temporarily unavailable"
    ));
    assert!(!should_retry_producer_image_request(
        400,
        "validation failed"
    ));
}

#[test]
fn producer_image_max_attempts_matches_retry_contract() {
    assert_eq!(PRODUCER_IMAGE_MAX_ATTEMPTS, 2);
}

#[test]
fn ensure_successful_producer_http_status_accepts_2xx_contract() {
    let result = ensure_successful_producer_http_status(204, "", "producer_compatible", None);

    assert!(result.is_ok());
}

#[test]
fn ensure_successful_producer_http_status_preserves_upstream_error_contract() {
    let error = ensure_successful_producer_http_status(
        503,
        "service unavailable",
        "producer_compatible",
        None,
    )
    .expect_err("expected non-2xx to fail");

    assert_eq!(error.http_status, Some(503));
    assert!(error.message.contains("service unavailable"));
    assert_eq!(error.code, None);
}

#[test]
fn ensure_successful_producer_http_status_attaches_override_code_contract() {
    let error = ensure_successful_producer_http_status(
        500,
        "status failed",
        "producer_compatible",
        Some("producer_http_video_status_failed"),
    )
    .expect_err("expected non-2xx to fail");

    assert_eq!(
        error.code.as_deref(),
        Some("producer_http_video_status_failed")
    );
    assert_eq!(error.http_status, Some(500));
}

#[test]
fn ensure_successful_producer_message_stream_status_accepts_2xx_contract() {
    let result = ensure_successful_producer_message_stream_status(204, "", "producer_compatible");

    assert!(result.is_ok());
}

#[test]
fn ensure_successful_producer_message_stream_status_preserves_failure_contract() {
    let error = ensure_successful_producer_message_stream_status(
        503,
        "service unavailable",
        "producer_compatible",
    )
    .expect_err("expected stream status to fail");

    assert_eq!(error.http_status, Some(503));
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    assert!(error.message.contains("service unavailable"));
}
