use super::*;

#[test]
fn unsupported_media_endpoint_error_matches_contract() {
    let err = unsupported_media_endpoint_error();
    assert_eq!(err.http_status, Some(400));
    assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
    assert_eq!(err.code.as_deref(), Some("unsupported_suno_endpoint"));
}

#[test]
fn missing_browser_worker_result_error_matches_contract() {
    let err = missing_browser_worker_result_error();
    assert_eq!(err.http_status, Some(500));
    assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
    assert_eq!(
        err.code.as_deref(),
        Some("suno_browser_worker_missing_result")
    );
    assert_eq!(
        err.message.as_str(),
        "Suno browser worker reported success without a result payload."
    );
}

#[test]
fn missing_browser_worker_clips_error_matches_contract() {
    let err = missing_browser_worker_clips_error();
    assert_eq!(err.http_status, Some(500));
    assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
    assert_eq!(err.code.as_deref(), Some("suno_missing_feed_clips"));
    assert_eq!(
        err.message.as_str(),
        "Suno browser worker completed without returning any clips."
    );
}

#[test]
fn empty_browser_worker_output_error_formats_stderr_fallback() {
    let with_stderr = empty_browser_worker_output_error("permission denied");
    assert_eq!(with_stderr.http_status, Some(500));
    assert_eq!(
        with_stderr.provider_name.as_deref(),
        Some("suno_compatible")
    );
    assert_eq!(
        with_stderr.code.as_deref(),
        Some("suno_browser_worker_empty_output")
    );
    assert_eq!(
        with_stderr.message.as_str(),
        "Suno browser worker did not return JSON output. stderr: permission denied"
    );

    let empty = empty_browser_worker_output_error("");
    assert_eq!(
        empty.message.as_str(),
        "Suno browser worker did not return JSON output. stderr: <empty>"
    );
}

#[test]
fn browser_worker_output_parse_error_formats_error_and_stdout() {
    let err = browser_worker_output_parse_error("expected value", "{\"oops\":");
    assert_eq!(err.http_status, Some(500));
    assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
    assert_eq!(
        err.code.as_deref(),
        Some("suno_browser_worker_output_parse_failed")
    );
    assert_eq!(
        err.message.as_str(),
        "Failed to parse Suno browser worker output: expected value. stdout: {\"oops\":"
    );
}

#[test]
fn browser_worker_wait_failed_error_formats_cause() {
    let err = browser_worker_wait_failed_error("The pipe has been ended");
    assert_eq!(err.http_status, Some(500));
    assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
    assert_eq!(err.code.as_deref(), Some("suno_browser_worker_wait_failed"));
    assert_eq!(
        err.message.as_str(),
        "Suno browser worker failed before producing output: The pipe has been ended"
    );
}

#[test]
fn browser_worker_timeout_error_matches_contract() {
    let err = browser_worker_timeout_error();
    assert_eq!(err.http_status, Some(500));
    assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
    assert_eq!(err.code.as_deref(), Some("suno_browser_worker_timeout"));
    assert_eq!(
        err.message.as_str(),
        "Suno browser worker timed out before producing output."
    );
}

#[test]
fn browser_worker_stdin_error_formats_cause() {
    let err = browser_worker_stdin_error("broken pipe");
    assert_eq!(err.http_status, Some(500));
    assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
    assert_eq!(
        err.code.as_deref(),
        Some("suno_browser_worker_stdin_failed")
    );
    assert_eq!(
        err.message.as_str(),
        "Failed to write Suno browser worker input: broken pipe"
    );
}

#[test]
fn browser_worker_spawn_failed_error_hides_script_path_and_formats_cause() {
    let err = browser_worker_spawn_failed_error(
        std::path::Path::new("C:/tmp/suno-browser-worker.mjs"),
        "The system cannot find the file specified. (os error 2)",
    );
    assert_eq!(err.http_status, Some(500));
    assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
    assert_eq!(
        err.code.as_deref(),
        Some("suno_browser_worker_spawn_failed")
    );
    assert_eq!(
        err.message.as_str(),
        "Failed to launch Suno browser worker: The system cannot find the file specified. (os error 2)"
    );
}

#[test]
fn browser_worker_input_serialize_error_formats_cause() {
    let err = browser_worker_input_serialize_error("missing field `prompt`");
    assert_eq!(err.http_status, Some(500));
    assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
    assert_eq!(
        err.code.as_deref(),
        Some("suno_browser_worker_input_serialize_failed")
    );
    assert_eq!(
        err.message.as_str(),
        "Failed to serialize Suno browser worker input: missing field `prompt`"
    );
}

#[test]
fn remote_browser_worker_result_parse_error_formats_cause() {
    let err = remote_browser_worker_result_parse_error("expected value");
    assert_eq!(err.http_status, Some(500));
    assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
    assert_eq!(err.code.as_deref(), Some("suno_remote_result_parse_failed"));
    assert_eq!(
        err.message.as_str(),
        "Failed to parse remote Suno browser worker result: expected value"
    );
}
