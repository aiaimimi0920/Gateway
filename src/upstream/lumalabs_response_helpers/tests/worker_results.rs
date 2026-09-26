use super::*;

#[test]
fn parse_lumalabs_browser_worker_output_reads_result_contract() {
    let parsed = parse_lumalabs_browser_worker_output(
        "{\"ok\":true,\"signedUrl\":\"https://cdn.example.com/file.mp4\"}",
        "",
    )
    .expect("lumalabs worker output");

    assert!(parsed.ok);
    assert_eq!(
        parsed.signed_url.as_deref(),
        Some("https://cdn.example.com/file.mp4")
    );
}

#[test]
fn parse_lumalabs_browser_worker_output_rejects_empty_stdout_contract() {
    let error = parse_lumalabs_browser_worker_output("", "permission denied")
        .expect_err("empty stdout should fail");
    assert_eq!(
        error.code.as_deref(),
        Some("lumalabs_browser_worker_empty_output")
    );
    assert_eq!(error.provider_name.as_deref(), Some("lumalabs_compatible"));
}

#[test]
fn classify_lumalabs_browser_worker_failure_prefers_worker_error_contract() {
    let result = parse_lumalabs_browser_worker_output(
        "{\"ok\":false,\"error\":{\"code\":\"lumalabs_browser_failed\",\"message\":\"challenge required\",\"status\":422,\"body\":\"{\\\"detail\\\":\\\"Unauthorized\\\"}\"}}",
        "",
    )
    .expect("lumalabs worker output");

    let error = classify_lumalabs_browser_worker_failure(result, "", "lumalabs_compatible");
    assert_eq!(error.provider_name.as_deref(), Some("lumalabs_compatible"));
    assert_eq!(error.code.as_deref(), Some("lumalabs_browser_failed"));
    assert_eq!(error.message, "challenge required");
    assert_eq!(error.http_status, Some(422));
}

#[test]
fn classify_lumalabs_browser_worker_failure_uses_stderr_when_body_missing() {
    let result = parse_lumalabs_browser_worker_output("{\"ok\":false,\"error\":{}}", "")
        .expect("lumalabs worker output");

    let error = classify_lumalabs_browser_worker_failure(
        result,
        "permission denied",
        "lumalabs_compatible",
    );
    assert_eq!(error.provider_name.as_deref(), Some("lumalabs_compatible"));
    assert_eq!(error.http_status, Some(500));
    assert!(error.message.contains("permission denied"));
}

#[test]
fn extract_lumalabs_browser_worker_success_reads_signed_url_contract() {
    let result = parse_lumalabs_browser_worker_output(
        "{\"ok\":true,\"signedUrl\":\"https://cdn.example.com/file.mp4\"}",
        "",
    )
    .expect("lumalabs worker output");

    let signed_url =
        extract_lumalabs_browser_worker_success(result).expect("lumalabs worker success");
    assert_eq!(signed_url, "https://cdn.example.com/file.mp4");
}

#[test]
fn extract_lumalabs_browser_worker_success_rejects_missing_signed_url_contract() {
    let result = parse_lumalabs_browser_worker_output("{\"ok\":true}", "").expect("worker output");

    let error = extract_lumalabs_browser_worker_success(result)
        .expect_err("missing signed url should fail");
    assert_eq!(
        error.code.as_deref(),
        Some("lumalabs_browser_worker_missing_signed_url")
    );
    assert_eq!(error.provider_name.as_deref(), Some("lumalabs_compatible"));
}

#[test]
fn resolve_lumalabs_browser_worker_result_reads_success_contract() {
    let result = parse_lumalabs_browser_worker_output(
        "{\"ok\":true,\"signedUrl\":\"https://cdn.example.com/file.mp4\"}",
        "",
    )
    .expect("lumalabs worker output");

    let signed_url = resolve_lumalabs_browser_worker_result(result, "", "lumalabs_compatible")
        .expect("ok worker result should succeed");
    assert_eq!(signed_url, "https://cdn.example.com/file.mp4");
}

#[test]
fn resolve_lumalabs_browser_worker_result_preserves_failure_contract() {
    let result = parse_lumalabs_browser_worker_output(
        "{\"ok\":false,\"error\":{\"code\":\"lumalabs_browser_failed\",\"message\":\"challenge required\",\"status\":422,\"body\":\"{\\\"detail\\\":\\\"Unauthorized\\\"}\"}}",
        "",
    )
    .expect("lumalabs worker output");

    let error = resolve_lumalabs_browser_worker_result(result, "", "lumalabs_compatible")
        .expect_err("failed worker result should error");

    assert_eq!(error.provider_name.as_deref(), Some("lumalabs_compatible"));
    assert_eq!(error.code.as_deref(), Some("lumalabs_browser_failed"));
    assert_eq!(error.message, "challenge required");
    assert_eq!(error.http_status, Some(422));
}

#[test]
fn parse_lumalabs_browser_worker_verified_output_reads_success_contract() {
    let signed_url = parse_lumalabs_browser_worker_verified_output(
        "{\"ok\":true,\"signedUrl\":\"https://cdn.example.com/file.mp4\"}",
        "",
        "lumalabs_compatible",
    )
    .expect("verified worker output");

    assert_eq!(signed_url, "https://cdn.example.com/file.mp4");
}

#[test]
fn parse_lumalabs_browser_worker_verified_output_preserves_failure_contract() {
    let error = parse_lumalabs_browser_worker_verified_output(
        "{\"ok\":false,\"error\":{\"code\":\"lumalabs_browser_failed\",\"message\":\"challenge required\",\"status\":422,\"body\":\"{\\\"detail\\\":\\\"Unauthorized\\\"}\"}}",
        "",
        "lumalabs_compatible",
    )
    .expect_err("failed worker output should error");

    assert_eq!(error.provider_name.as_deref(), Some("lumalabs_compatible"));
    assert_eq!(error.code.as_deref(), Some("lumalabs_browser_failed"));
    assert_eq!(error.message, "challenge required");
    assert_eq!(error.http_status, Some(422));
}

#[test]
fn parse_lumalabs_remote_browser_executor_signed_url_reads_contract() {
    let signed_url = parse_lumalabs_remote_browser_executor_signed_url(&serde_json::json!({
        "signedUrl": "https://cdn.example.com/out.png?sig=test"
    }))
    .expect("signed url should be extracted");

    assert_eq!(signed_url, "https://cdn.example.com/out.png?sig=test");
}

#[test]
fn parse_lumalabs_remote_browser_executor_signed_url_preserves_missing_contract() {
    let error = parse_lumalabs_remote_browser_executor_signed_url(&serde_json::json!({
        "ok": true
    }))
    .expect_err("missing signedUrl should fail");

    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.code.as_deref(),
        Some("lumalabs_browser_executor_missing_signed_url")
    );
    assert_eq!(error.provider_name.as_deref(), Some("lumalabs_compatible"));
}

#[test]
fn build_lumalabs_browser_executor_service_result_wraps_signed_url_contract() {
    let body = build_lumalabs_browser_executor_service_result("https://cdn.example.com/file.mp4");

    assert_eq!(body["signedUrl"], "https://cdn.example.com/file.mp4");
}

#[test]
fn build_lumalabs_browser_executor_service_result_preserves_empty_string_contract() {
    let body = build_lumalabs_browser_executor_service_result("");

    assert_eq!(body["signedUrl"], "");
}
