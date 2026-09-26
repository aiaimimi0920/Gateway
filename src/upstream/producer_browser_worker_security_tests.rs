use serde_json::json;

use super::browser_worker_message::enrich;
use super::producer_media_helpers::parse_producer_browser_worker_verified_output;

fn assert_safe_message(message: &str) {
    assert!(message.contains("[REDACTED]"));
    assert!(!message.contains("fake-message-secret"));
    assert!(!message.contains("fake-body-secret"));
    assert!(!message.chars().any(char::is_control));
    assert!(message.chars().count() <= 512);
}

#[test]
fn browser_worker_message_enrichment_sanitizes_and_bounds_distinct_parts() {
    let body = format!(
        "upstream body\r\nsk-test-fake-body-secret-123456\n{}",
        "界".repeat(700)
    );
    let message = enrich(
        "worker failed token=fake-message-secret\n".to_string(),
        Some(body),
    );

    assert!(message.starts_with("worker failed token=[REDACTED] upstream body:"));
    assert_safe_message(&message);
}

#[test]
fn browser_worker_message_enrichment_is_utf8_boundary_safe() {
    let message = enrich(
        "worker failed".to_string(),
        Some(format!("token=fake-body-secret\n{}", "界".repeat(700))),
    );

    assert_safe_message(&message);
}

#[test]
fn browser_worker_message_enrichment_omits_oversized_parts_before_sanitizing() {
    let oversized = format!("{}token=fake-oversized-tail-secret", "x".repeat(20_000));
    let message = enrich(oversized.clone(), None);
    let body = enrich("worker failed".to_string(), Some(oversized));

    assert_eq!(
        message,
        "[upstream detail omitted: safe processing limit exceeded]"
    );
    assert_eq!(
        body,
        "worker failed upstream body: [upstream detail omitted: safe processing limit exceeded]"
    );
    assert!(!message.contains("fake-oversized-tail-secret"));
    assert!(!body.contains("fake-oversized-tail-secret"));
    assert!(message.chars().count() <= 512);
    assert!(body.chars().count() <= 512);
}

#[test]
fn producer_worker_failure_preserves_contract_while_sanitizing_message() {
    let output = json!({
        "ok": false,
        "status": 400,
        "error": {
            "code": "producer_worker_blocked",
            "message": "challenge token=fake-message-secret\n",
            "status": 422,
            "body": format!(
                "Bearer fake-body-secret\r\n{}",
                "界".repeat(700)
            )
        }
    })
    .to_string();

    let error = parse_producer_browser_worker_verified_output(&output, "", "producer_compatible")
        .expect_err("failed worker output should remain an error");

    assert_eq!(error.http_status, Some(422));
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    assert_eq!(error.code.as_deref(), Some("producer_worker_blocked"));
    assert_safe_message(&error.message);
}

#[test]
fn producer_worker_failure_omits_oversized_body_before_classifying() {
    let oversized = format!("{}token=fake-oversized-body-tail", "x".repeat(20_000));
    let output = json!({
        "ok": false,
        "status": 503,
        "error": { "body": oversized }
    })
    .to_string();

    let error = parse_producer_browser_worker_verified_output(&output, "", "producer_compatible")
        .expect_err("failed worker output should remain an error");

    assert_eq!(error.http_status, Some(503));
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    assert!(error.message.contains("safe processing limit exceeded"));
    assert!(!error.message.contains("fake-oversized-body-tail"));
    assert!(error.message.chars().count() <= 512);
}

#[test]
fn producer_worker_failure_omits_oversized_stderr_before_classifying() {
    let stderr = format!("{}token=fake-oversized-stderr-tail", "x".repeat(20_000));
    let output = json!({ "ok": false, "status": 503 }).to_string();

    let error =
        parse_producer_browser_worker_verified_output(&output, &stderr, "producer_compatible")
            .expect_err("failed worker output should remain an error");

    assert_eq!(error.http_status, Some(503));
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    assert!(error.message.contains("safe processing limit exceeded"));
    assert!(!error.message.contains("fake-oversized-stderr-tail"));
    assert!(error.message.chars().count() <= 512);
}
