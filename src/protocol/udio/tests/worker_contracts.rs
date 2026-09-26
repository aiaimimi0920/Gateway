use super::*;

#[test]
fn udio_unsupported_request_plan_error_matches_contract() {
    let err = unsupported_request_plan_error();
    assert_eq!(err.http_status, Some(400));
    assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
    assert_eq!(err.code.as_deref(), Some("unsupported_udio_endpoint"));
}

#[test]
fn plan_udio_chat_endpoint_rejected_locally() {
    let payload = make_payload("udio_compatible", "https://www.udio.com");
    let mut req = make_request(json!({ "prompt": "ignored" }));
    req.endpoint_kind = EndpointKind::ChatCompletions;
    let err = crate::upstream::client::UpstreamClient::build_request_plan(
        &payload,
        &req,
        "udio-music",
        false,
    )
    .expect_err("udio chat requests should be rejected");
    assert_eq!(err.http_status, Some(400));
    assert_eq!(err.code.as_deref(), Some("unsupported_udio_endpoint"));
}

#[test]
fn missing_browser_runtime_error_matches_contract() {
    let err = missing_browser_runtime_error();
    assert_eq!(err.http_status, Some(500));
    assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
    assert_eq!(err.code.as_deref(), Some("missing_udio_browser_runtime"));
}

#[test]
fn missing_browser_worker_result_error_matches_contract() {
    let err = missing_browser_worker_result_error();
    assert_eq!(err.http_status, Some(500));
    assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
    assert_eq!(
        err.code.as_deref(),
        Some("udio_browser_worker_missing_result")
    );
    assert_eq!(
        err.message.as_str(),
        "Udio browser worker reported success without a result payload."
    );
}

#[test]
fn missing_browser_worker_track_ids_error_matches_contract() {
    let err = missing_browser_worker_track_ids_error();
    assert_eq!(err.http_status, Some(500));
    assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
    assert_eq!(err.code.as_deref(), Some("udio_missing_track_ids"));
    assert_eq!(
        err.message.as_str(),
        "Udio browser worker completed without returning any track ids."
    );
}

#[test]
fn empty_browser_worker_output_error_formats_stderr_fallback() {
    let with_stderr = empty_browser_worker_output_error("permission denied");
    assert_eq!(with_stderr.http_status, Some(500));
    assert_eq!(
        with_stderr.provider_name.as_deref(),
        Some("udio_compatible")
    );
    assert_eq!(
        with_stderr.code.as_deref(),
        Some("udio_browser_worker_empty_output")
    );
    assert_eq!(
        with_stderr.message.as_str(),
        "Udio browser worker did not return JSON output. stderr: permission denied"
    );

    let empty = empty_browser_worker_output_error("");
    assert_eq!(
        empty.message.as_str(),
        "Udio browser worker did not return JSON output. stderr: <empty>"
    );
}

#[test]
fn browser_worker_output_parse_error_formats_error_and_stdout() {
    let err = browser_worker_output_parse_error("expected value", "{\"oops\":");
    assert_eq!(err.http_status, Some(500));
    assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
    assert_eq!(
        err.code.as_deref(),
        Some("udio_browser_worker_output_parse_failed")
    );
    assert_eq!(
        err.message.as_str(),
        "Failed to parse Udio browser worker output: expected value. stdout: {\"oops\":"
    );
}

#[test]
fn browser_worker_wait_failed_error_formats_cause() {
    let err = browser_worker_wait_failed_error("The pipe has been ended");
    assert_eq!(err.http_status, Some(500));
    assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
    assert_eq!(err.code.as_deref(), Some("udio_browser_worker_wait_failed"));
    assert_eq!(
        err.message.as_str(),
        "Udio browser worker failed before producing output: The pipe has been ended"
    );
}

#[test]
fn browser_worker_timeout_error_matches_contract() {
    let err = browser_worker_timeout_error();
    assert_eq!(err.http_status, Some(500));
    assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
    assert_eq!(err.code.as_deref(), Some("udio_browser_worker_timeout"));
    assert_eq!(
        err.message.as_str(),
        "Udio browser worker timed out before producing output."
    );
}

#[test]
fn browser_worker_stdin_error_formats_cause() {
    let err = browser_worker_stdin_error("broken pipe");
    assert_eq!(err.http_status, Some(500));
    assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
    assert_eq!(
        err.code.as_deref(),
        Some("udio_browser_worker_stdin_failed")
    );
    assert_eq!(
        err.message.as_str(),
        "Failed to write Udio browser worker input: broken pipe"
    );
}

#[test]
fn browser_worker_spawn_failed_error_hides_script_path_and_formats_cause() {
    let err = browser_worker_spawn_failed_error(
        std::path::Path::new("C:/tmp/udio-browser-worker.mjs"),
        "The system cannot find the file specified. (os error 2)",
    );
    assert_eq!(err.http_status, Some(500));
    assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
    assert_eq!(
        err.code.as_deref(),
        Some("udio_browser_worker_spawn_failed")
    );
    assert_eq!(
        err.message.as_str(),
        "Failed to launch Udio browser worker: The system cannot find the file specified. (os error 2)"
    );
}

#[test]
fn browser_worker_input_serialize_error_formats_cause() {
    let err = browser_worker_input_serialize_error("missing field `prompt`");
    assert_eq!(err.http_status, Some(500));
    assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
    assert_eq!(
        err.code.as_deref(),
        Some("udio_browser_worker_input_serialize_failed")
    );
    assert_eq!(
        err.message.as_str(),
        "Failed to serialize Udio browser worker input: missing field `prompt`"
    );
}

#[test]
fn remote_browser_worker_result_parse_error_formats_cause() {
    let err = remote_browser_worker_result_parse_error("expected value");
    assert_eq!(err.http_status, Some(500));
    assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
    assert_eq!(err.code.as_deref(), Some("udio_remote_result_parse_failed"));
    assert_eq!(
        err.message.as_str(),
        "Failed to parse remote Udio browser worker result: expected value"
    );
}
