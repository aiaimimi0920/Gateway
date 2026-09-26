use super::*;

#[test]
fn parse_producer_browser_worker_output_reads_result_contract() {
    let parsed = parse_producer_browser_worker_output(
        "{\"ok\":true,\"status\":200,\"result\":{\"conversation_id\":\"conv-1\"}}",
        "",
    )
    .expect("producer worker output");

    assert!(parsed.ok);
    assert_eq!(parsed.status, Some(200));
    assert_eq!(
        parsed
            .result
            .as_ref()
            .and_then(|value| value.get("conversation_id")),
        Some(&json!("conv-1"))
    );
}

#[test]
fn parse_producer_browser_worker_output_rejects_empty_stdout_contract() {
    let error = parse_producer_browser_worker_output("", "permission denied")
        .expect_err("empty stdout should fail");
    assert_eq!(
        error.code.as_deref(),
        Some("producer_browser_worker_empty_output")
    );
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
}

#[test]
fn extract_producer_browser_worker_success_reads_result_payload() {
    let result = parse_producer_browser_worker_output(
        "{\"ok\":true,\"status\":200,\"result\":{\"conversation_id\":\"conv-1\"}}",
        "",
    )
    .expect("producer worker output");

    let payload = extract_producer_browser_worker_success(result).expect("producer worker success");
    assert_eq!(payload["conversation_id"], "conv-1");
}

#[test]
fn extract_producer_browser_worker_success_rejects_missing_result_contract() {
    let result = parse_producer_browser_worker_output("{\"ok\":true,\"status\":200}", "")
        .expect("producer worker output");

    let error = extract_producer_browser_worker_success(result)
        .expect_err("missing result payload should fail");
    assert_eq!(
        error.code.as_deref(),
        Some("producer_browser_worker_missing_result")
    );
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
}

#[test]
fn classify_producer_browser_worker_failure_prefers_worker_error_contract() {
    let result = parse_producer_browser_worker_output(
        "{\"ok\":false,\"status\":400,\"error\":{\"code\":\"producer_worker_blocked\",\"message\":\"challenge required\",\"status\":422,\"body\":\"{\\\"detail\\\":\\\"Unauthorized\\\"}\"}}",
        "",
    )
    .expect("producer worker output");

    let error = classify_producer_browser_worker_failure(result, "", "producer_compatible");
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    assert_eq!(error.code.as_deref(), Some("producer_worker_blocked"));
    assert_eq!(
        error.message,
        "challenge required upstream body: {\"detail\":\"Unauthorized\"}"
    );
    assert_eq!(error.http_status, Some(422));
}

#[test]
fn classify_producer_browser_worker_failure_uses_stderr_when_body_missing() {
    let result = parse_producer_browser_worker_output("{\"ok\":false,\"status\":429}", "")
        .expect("producer worker output");

    let error = classify_producer_browser_worker_failure(
        result,
        "permission denied",
        "producer_compatible",
    );
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    assert_eq!(error.http_status, Some(429));
    assert!(error.message.contains("permission denied"));
}

#[test]
fn resolve_producer_browser_worker_result_reads_result_payload_contract() {
    let result = parse_producer_browser_worker_output(
        "{\"ok\":true,\"status\":200,\"result\":{\"conversation_id\":\"conv-1\"}}",
        "",
    )
    .expect("producer worker output");

    let payload = resolve_producer_browser_worker_result(result, "", "producer_compatible")
        .expect("ok worker result should succeed");
    assert_eq!(payload["conversation_id"], "conv-1");
}

#[test]
fn resolve_producer_browser_worker_result_preserves_failure_contract() {
    let result = parse_producer_browser_worker_output(
        "{\"ok\":false,\"status\":400,\"error\":{\"code\":\"producer_worker_blocked\",\"message\":\"challenge required\",\"status\":422,\"body\":\"{\\\"detail\\\":\\\"Unauthorized\\\"}\"}}",
        "",
    )
    .expect("producer worker output");

    let error = resolve_producer_browser_worker_result(result, "", "producer_compatible")
        .expect_err("failed worker result should error");
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    assert_eq!(error.code.as_deref(), Some("producer_worker_blocked"));
    assert_eq!(
        error.message,
        "challenge required upstream body: {\"detail\":\"Unauthorized\"}"
    );
    assert_eq!(error.http_status, Some(422));
}

#[test]
fn parse_producer_browser_worker_verified_output_reads_result_payload_contract() {
    let payload = parse_producer_browser_worker_verified_output(
        "{\"ok\":true,\"status\":200,\"result\":{\"conversation_id\":\"conv-1\"}}",
        "",
        "producer_compatible",
    )
    .expect("verified worker output");
    assert_eq!(payload["conversation_id"], "conv-1");
}

#[test]
fn parse_producer_browser_worker_verified_output_preserves_failure_contract() {
    let error = parse_producer_browser_worker_verified_output(
        "{\"ok\":false,\"status\":400,\"error\":{\"code\":\"producer_worker_blocked\",\"message\":\"challenge required\",\"status\":422,\"body\":\"{\\\"detail\\\":\\\"Unauthorized\\\"}\"}}",
        "",
        "producer_compatible",
    )
    .expect_err("failed worker output should error");
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    assert_eq!(error.code.as_deref(), Some("producer_worker_blocked"));
    assert_eq!(
        error.message,
        "challenge required upstream body: {\"detail\":\"Unauthorized\"}"
    );
    assert_eq!(error.http_status, Some(422));
}
