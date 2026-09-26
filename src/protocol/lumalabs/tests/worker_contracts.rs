use super::*;

#[test]
fn lumalabs_unsupported_request_plan_error_matches_contract() {
    let err = unsupported_request_plan_error();
    assert_eq!(err.http_status, Some(400));
    assert_eq!(err.code.as_deref(), Some("unsupported_lumalabs_endpoint"));
    assert!(err.message.contains("image, video, and audio-generation"));
}

#[test]
fn plan_lumalabs_chat_endpoint_rejected_locally() {
    let payload = make_payload("lumalabs_compatible", "https://app.lumalabs.ai");
    let mut req = make_request(json!({ "prompt": "ignored" }));
    req.endpoint_kind = EndpointKind::ChatCompletions;
    let err = UpstreamClient::build_request_plan(&payload, &req, "uni-1", false)
        .expect_err("lumalabs chat requests should be rejected");
    assert_eq!(err.http_status, Some(400));
    assert_eq!(err.code.as_deref(), Some("unsupported_lumalabs_endpoint"));
}

#[test]
fn unsupported_image_inputs_error_matches_contract() {
    let err = unsupported_image_inputs_error();
    assert_eq!(err.http_status, Some(400));
    assert_eq!(
        err.code.as_deref(),
        Some("unsupported_lumalabs_image_inputs")
    );
    assert_eq!(
        err.message.as_str(),
        "LumaLabs image generation currently supports prompt-only requests and does not accept image uploads or masks."
    );
}

#[test]
fn extract_remote_executor_signed_url_reads_payload_and_rejects_missing_value() {
    let ok = extract_remote_executor_signed_url(&json!({
        "signedUrl": "https://cdn.example.com/out.png?sig=test"
    }))
    .expect("signed url should be extracted");
    assert_eq!(ok, "https://cdn.example.com/out.png?sig=test");

    let err = extract_remote_executor_signed_url(&json!({ "ok": true }))
        .expect_err("missing signedUrl should be rejected");
    assert_eq!(err.http_status, Some(500));
    assert_eq!(
        err.code.as_deref(),
        Some("lumalabs_browser_executor_missing_signed_url")
    );
    assert_eq!(
        err.message.as_str(),
        "Remote LumaLabs browser executor succeeded without a signedUrl."
    );
}

#[test]
fn extract_browser_worker_signed_url_rejects_missing_value() {
    let ok = extract_browser_worker_signed_url(Some(
        "https://cdn.example.com/out.png?sig=test".to_string(),
    ))
    .expect("signed url should be returned");
    assert_eq!(ok, "https://cdn.example.com/out.png?sig=test");

    let err =
        extract_browser_worker_signed_url(None).expect_err("missing signed url should be rejected");
    assert_eq!(err.http_status, Some(500));
    assert_eq!(
        err.code.as_deref(),
        Some("lumalabs_browser_worker_missing_signed_url")
    );
    assert_eq!(
        err.message.as_str(),
        "LumaLabs browser worker reported success without a signed URL."
    );
}

#[test]
fn empty_browser_worker_output_error_formats_stderr_fallback() {
    let with_stderr = empty_browser_worker_output_error("permission denied");
    assert_eq!(with_stderr.http_status, Some(500));
    assert_eq!(
        with_stderr.provider_name.as_deref(),
        Some("lumalabs_compatible")
    );
    assert_eq!(
        with_stderr.code.as_deref(),
        Some("lumalabs_browser_worker_empty_output")
    );
    assert_eq!(
        with_stderr.message.as_str(),
        "LumaLabs browser worker did not return JSON output. stderr: permission denied"
    );

    let empty = empty_browser_worker_output_error("");
    assert_eq!(
        empty.message.as_str(),
        "LumaLabs browser worker did not return JSON output. stderr: <empty>"
    );
}

#[test]
fn browser_worker_output_parse_error_formats_error_and_stdout() {
    let err = browser_worker_output_parse_error("expected value", "{\"oops\":");
    assert_eq!(err.http_status, Some(500));
    assert_eq!(err.provider_name.as_deref(), Some("lumalabs_compatible"));
    assert_eq!(
        err.code.as_deref(),
        Some("lumalabs_browser_worker_output_parse_failed")
    );
    assert_eq!(
        err.message.as_str(),
        "Failed to parse LumaLabs browser worker output: expected value. stdout: {\"oops\":"
    );
}

#[test]
fn browser_worker_wait_failed_error_formats_cause() {
    let err = browser_worker_wait_failed_error("The pipe has been ended");
    assert_eq!(err.http_status, Some(500));
    assert_eq!(err.provider_name.as_deref(), Some("lumalabs_compatible"));
    assert_eq!(
        err.code.as_deref(),
        Some("lumalabs_browser_worker_wait_failed")
    );
    assert_eq!(
        err.message.as_str(),
        "LumaLabs browser worker failed before producing output: The pipe has been ended"
    );
}

#[test]
fn browser_worker_stdin_error_formats_cause() {
    let err = browser_worker_stdin_error("broken pipe");
    assert_eq!(err.http_status, Some(500));
    assert_eq!(err.provider_name.as_deref(), Some("lumalabs_compatible"));
    assert_eq!(
        err.code.as_deref(),
        Some("lumalabs_browser_worker_stdin_failed")
    );
    assert_eq!(
        err.message.as_str(),
        "Failed to write LumaLabs browser worker input: broken pipe"
    );
}

#[test]
fn browser_worker_spawn_failed_error_hides_script_path_and_formats_cause() {
    let err = browser_worker_spawn_failed_error(
        std::path::Path::new("C:/tmp/lumalabs-browser-worker.mjs"),
        "The system cannot find the file specified. (os error 2)",
    );
    assert_eq!(err.http_status, Some(500));
    assert_eq!(err.provider_name.as_deref(), Some("lumalabs_compatible"));
    assert_eq!(
        err.code.as_deref(),
        Some("lumalabs_browser_worker_spawn_failed")
    );
    assert_eq!(
        err.message.as_str(),
        "Failed to launch LumaLabs browser worker: The system cannot find the file specified. (os error 2)"
    );
}

#[test]
fn browser_worker_input_serialize_error_formats_cause() {
    let err = browser_worker_input_serialize_error("missing field `prompt`");
    assert_eq!(err.http_status, Some(500));
    assert_eq!(err.provider_name.as_deref(), Some("lumalabs_compatible"));
    assert_eq!(
        err.code.as_deref(),
        Some("lumalabs_browser_worker_input_serialize_failed")
    );
    assert_eq!(
        err.message.as_str(),
        "Failed to serialize LumaLabs browser worker input: missing field `prompt`"
    );
}
