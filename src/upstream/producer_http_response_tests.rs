use super::*;
use crate::protocol::producer::{normalize_image_generations, PRODUCER_IMAGE_DEFAULT_MODEL};

#[test]
fn producer_http_video_missing_conversation_id_error_matches_contract() {
    let error = producer_http_video_missing_conversation_id_error("producer_compatible");
    assert_eq!(error.http_status, Some(500));
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    assert_eq!(
        error.code.as_deref(),
        Some("producer_http_video_missing_conversation_id")
    );
    assert_eq!(
        error.message.as_str(),
        "Producer video bootstrap completed without returning a conversation_id."
    );
}

#[test]
fn producer_http_video_missing_video_proposal_error_matches_contract() {
    let error = producer_http_video_missing_video_proposal_error("producer_compatible");
    assert_eq!(error.http_status, Some(500));
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    assert_eq!(
        error.code.as_deref(),
        Some("producer_http_video_missing_video_proposal")
    );
    assert_eq!(
        error.message.as_str(),
        "Producer video flow did not complete the required video__propose_music_video step before confirmation."
    );
}

#[test]
fn producer_http_video_missing_video_job_id_error_matches_contract() {
    let error = producer_http_video_missing_video_job_id_error("producer_compatible");
    assert_eq!(error.http_status, Some(500));
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    assert_eq!(
        error.code.as_deref(),
        Some("producer_http_video_missing_video_job_id")
    );
    assert_eq!(
        error.message.as_str(),
        "Producer conversation completed without returning a music-video job_id."
    );
}

#[test]
fn producer_invalid_video_status_response_error_matches_contract() {
    let error = producer_invalid_video_status_response_error("producer_compatible");
    assert_eq!(error.http_status, Some(500));
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    assert_eq!(
        error.code.as_deref(),
        Some("producer_invalid_video_status_response")
    );
    assert_eq!(
        error.message.as_str(),
        "Producer.ai music-video status response was not valid JSON."
    );
}

#[test]
fn parse_producer_image_generation_response_builds_success_body() {
    let req = normalize_image_generations(json!({
        "prompt": "holographic album cover",
        "type": "clip"
    }))
    .unwrap();

    let parsed = parse_producer_image_generation_response(
        "{\"image_id\":\"img-42\"}",
        &req,
        PRODUCER_IMAGE_DEFAULT_MODEL,
        Some("e30.eyJpc3MiOiJodHRwczovL2RlbW8tcHJvamVjdC5zdXBhYmFzZS5jby9hdXRoL3YxIiwic3ViIjoidXNlci0xMjMifQ.sig"),
        "producer_compatible",
    )
    .expect("producer image generation response");

    assert_eq!(parsed["object"], "image.generation");
    assert_eq!(parsed["image_id"], "img-42");
    assert_eq!(parsed["data"][0]["image_id"], "img-42");
}

#[test]
fn parse_producer_image_generation_response_rejects_invalid_json_contract() {
    let req = normalize_image_generations(json!({
        "prompt": "holographic album cover",
        "type": "clip"
    }))
    .unwrap();

    let error = parse_producer_image_generation_response(
        "not-json",
        &req,
        PRODUCER_IMAGE_DEFAULT_MODEL,
        None,
        "producer_compatible",
    )
    .expect_err("invalid JSON should fail");

    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.code.as_deref(),
        Some("producer_invalid_image_response")
    );
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
}

#[test]
fn parse_producer_video_status_response_reads_json_object() {
    let parsed = parse_producer_video_status_response(
        "{\"status\":\"completed\",\"asset\":\"https://cdn.example.com/video.mp4\"}",
        "producer_compatible",
    )
    .expect("producer video status response");
    assert_eq!(parsed["status"], "completed");
    assert_eq!(parsed["asset"], "https://cdn.example.com/video.mp4");
}

#[test]
fn parse_producer_video_status_response_rejects_invalid_json_contract() {
    let error = parse_producer_video_status_response("not-json", "producer_compatible")
        .expect_err("invalid JSON should fail");
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.code.as_deref(),
        Some("producer_invalid_video_status_response")
    );
    assert_eq!(
        error.message.as_str(),
        "Producer.ai music-video status response was not valid JSON."
    );
}

#[test]
fn parse_producer_video_status_http_response_reads_success_contract() {
    let parsed = parse_producer_video_status_http_response(
        200,
        "{\"status\":\"completed\",\"asset\":\"https://cdn.example.com/video.mp4\"}",
        "producer_compatible",
    )
    .expect("producer video status http response");

    assert_eq!(parsed["status"], "completed");
    assert_eq!(parsed["asset"], "https://cdn.example.com/video.mp4");
}

#[test]
fn parse_producer_video_status_http_response_preserves_failure_contract() {
    let error = parse_producer_video_status_http_response(
        503,
        "service unavailable",
        "producer_compatible",
    )
    .expect_err("non-2xx should fail");

    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.code.as_deref(),
        Some("producer_http_video_status_failed")
    );
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
}

#[test]
fn producer_invalid_image_response_error_matches_contract() {
    let error = producer_invalid_image_response_error("producer_compatible");
    assert_eq!(error.http_status, Some(500));
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    assert_eq!(
        error.code.as_deref(),
        Some("producer_invalid_image_response")
    );
    assert_eq!(
        error.message.as_str(),
        "Producer.ai image response was not valid JSON."
    );
}

#[test]
fn producer_image_retry_exhausted_error_matches_contract() {
    let error = producer_image_retry_exhausted_error("producer_compatible");
    assert_eq!(error.http_status, Some(500));
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    assert_eq!(
        error.code.as_deref(),
        Some("producer_image_retry_exhausted")
    );
    assert_eq!(
        error.message.as_str(),
        "Producer.ai image request exhausted retry attempts."
    );
}

#[test]
fn producer_invalid_conversation_response_error_matches_contract() {
    let error = producer_invalid_conversation_response_error("producer_compatible");
    assert_eq!(error.http_status, Some(500));
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    assert_eq!(
        error.code.as_deref(),
        Some("producer_invalid_conversation_response")
    );
    assert_eq!(
        error.message.as_str(),
        "Producer.ai conversation response was not valid JSON."
    );
}

#[test]
fn parse_producer_conversation_job_reads_snake_case_job_id() {
    let parsed = parse_producer_conversation_job(
        "{\"job_id\":\"job-123\",\"status\":\"queued\"}",
        "producer_compatible",
    )
    .expect("producer conversation response");
    assert_eq!(parsed.job_id, "job-123");
    assert_eq!(parsed.body["status"], "queued");
}

#[test]
fn parse_producer_conversation_job_rejects_invalid_json_contract() {
    let error = parse_producer_conversation_job("not-json", "producer_compatible")
        .expect_err("invalid JSON should fail");
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.code.as_deref(),
        Some("producer_invalid_conversation_response")
    );
    assert_eq!(
        error.message.as_str(),
        "Producer.ai conversation response was not valid JSON."
    );
}

#[test]
fn parse_producer_conversation_http_job_reads_success_contract() {
    let parsed = parse_producer_conversation_http_job(
        202,
        "{\"job_id\":\"job-123\",\"status\":\"queued\"}",
        "producer_compatible",
    )
    .expect("conversation HTTP response");

    assert_eq!(parsed.job_id, "job-123");
    assert_eq!(parsed.body["status"], "queued");
}

#[test]
fn parse_producer_conversation_http_job_preserves_failure_contract() {
    let error = parse_producer_conversation_http_job(
        502,
        "{\"error\":\"gateway timeout\"}",
        "producer_compatible",
    )
    .expect_err("non-2xx should fail");

    assert_eq!(error.http_status, Some(502));
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
}

#[test]
fn parse_producer_conversation_http_job_rejects_invalid_json_contract() {
    let error = parse_producer_conversation_http_job(202, "not-json", "producer_compatible")
        .expect_err("invalid JSON should fail");

    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.code.as_deref(),
        Some("producer_invalid_conversation_response")
    );
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
}

#[test]
fn extract_producer_send_message_job_id_reads_success_contract() {
    let job_id = extract_producer_send_message_job_id(
        202,
        &json!({
            "job_id": "job-123",
            "status": "queued"
        }),
        "producer_compatible",
    )
    .expect("send response should expose job id");

    assert_eq!(job_id, "job-123");
}

#[test]
fn extract_producer_send_message_job_id_preserves_failure_contract() {
    let error = extract_producer_send_message_job_id(
        502,
        &json!({
            "error": "gateway timeout"
        }),
        "producer_compatible",
    )
    .expect_err("non-2xx should surface upstream error");

    assert_eq!(error.http_status, Some(502));
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
}

#[test]
fn parse_producer_send_message_job_reads_success_contract() {
    let job_id = parse_producer_send_message_job(
        202,
        "{\"job_id\":\"job-123\",\"status\":\"queued\"}",
        "producer_compatible",
    )
    .expect("send response should parse to job id");

    assert_eq!(job_id, "job-123");
}

#[test]
fn parse_producer_send_message_job_rejects_invalid_json_contract() {
    let error = parse_producer_send_message_job(202, "not-json", "producer_compatible")
        .expect_err("invalid JSON should fail");

    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.code.as_deref(),
        Some("producer_invalid_conversation_response")
    );
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
}

#[test]
fn producer_http_video_failed_error_matches_contract() {
    let error = producer_http_video_failed_error(
        "producer_compatible",
        "failed",
        "{\"status\":\"failed\",\"reason\":\"upstream\"}",
    );
    assert_eq!(error.http_status, Some(500));
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    assert_eq!(error.code.as_deref(), Some("producer_http_video_failed"));
    assert_eq!(
        error.message.as_str(),
        "Producer video job entered terminal status 'failed'. status payload: {\"status\":\"failed\",\"reason\":\"upstream\"}"
    );
}

#[test]
fn producer_runtime_headers_add_origin_and_referer_defaults() {
    let headers = producer_runtime_headers(&HeaderMap::new(), "https://www.producer.ai/create");
    assert_eq!(
        headers.get("accept").and_then(|value| value.to_str().ok()),
        Some("application/json, text/plain, */*")
    );
    assert_eq!(
        headers.get("origin").and_then(|value| value.to_str().ok()),
        Some("https://www.flowmusic.app")
    );
    assert_eq!(
        headers.get("referer").and_then(|value| value.to_str().ok()),
        Some("https://www.producer.ai/create")
    );
}

#[test]
fn producer_stream_headers_force_event_stream_accept() {
    let headers = producer_stream_headers(&HeaderMap::new(), "https://www.producer.ai/watch");
    assert_eq!(
        headers.get("accept").and_then(|value| value.to_str().ok()),
        Some("text/event-stream")
    );
    assert_eq!(
        headers.get("referer").and_then(|value| value.to_str().ok()),
        Some("https://www.producer.ai/watch")
    );
}
