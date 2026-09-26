use super::*;
use crate::error::PROVIDER_ERROR_MESSAGE_MAX_CHARS;
use crate::upstream::gemini::web_reverse::{
    gemini_canvas_tts_direct_http_audio_unavailable_error,
    gemini_canvas_tts_direct_http_audio_unavailable_from_bodies,
};
use crate::upstream::gemini_canvas_runtime_error_helpers::{
    append_gateway_error_fields, append_gateway_error_summary, summarize_gateway_error,
    wrap_gemini_canvas_stream_parse_error,
};
use axum::response::IntoResponse;

const SECRET: &str = "token=fixture-token\nAuthorization: Bearer fixture-bearer\r\n";

fn assert_safe(message: &str) {
    assert!(!message.contains("fixture-token"));
    assert!(!message.contains("fixture-bearer"));
    assert!(!message.chars().any(char::is_control));
    assert!(message.chars().count() <= PROVIDER_ERROR_MESSAGE_MAX_CHARS);
}

fn oversized_secret() -> String {
    format!("{SECRET}{}", " detail".repeat(1024))
}

#[tokio::test]
async fn diagnostic_tts_constructor_sanitizes_each_dynamic_field_before_wire() {
    for index in 0..6 {
        let mut fields = ["plain"; 6];
        fields[index] = SECRET;
        assert_safe_wire(gemini_canvas_tts_direct_http_audio_unavailable_error(
            fields[0], fields[1], fields[2], fields[3], fields[4], fields[5],
        ))
        .await;
    }
    assert_safe_wire(gemini_canvas_tts_direct_http_audio_unavailable_error(
        "/app/tts",
        &"x".repeat(4096),
        "tail",
        "trigger",
        "followup",
        "export",
    ))
    .await;
}

#[tokio::test]
async fn diagnostic_tts_body_previews_redact_before_head_and_tail_cuts() {
    let body = format!("eyJ{}.{}.signature", "a".repeat(400), "b".repeat(400));
    for index in 0..4 {
        let mut bodies = ["plain"; 4];
        bodies[index] = &body;
        let error = gemini_canvas_tts_direct_http_audio_unavailable_from_bodies(
            "/app/tts", bodies[0], bodies[1], bodies[2], bodies[3],
        );
        assert!(!error.message.contains("eyJ"));
        assert!(!error.message.contains("signature"));
        assert_safe_wire(error).await;
    }
}

#[test]
fn diagnostic_tts_body_previews_preserve_short_and_empty_contracts() {
    for (body, preview) in [(" plain ", "plain"), (" \n ", "<empty>")] {
        let actual = gemini_canvas_tts_direct_http_audio_unavailable_from_bodies(
            "/app/tts", body, body, body, body,
        );
        let expected = gemini_canvas_tts_direct_http_audio_unavailable_error(
            "/app/tts", preview, preview, preview, preview, preview,
        );
        assert_eq!(actual.message, expected.message);
        assert_eq!(actual.http_status, Some(503));
        assert_eq!(actual.code, expected.code);
        assert_eq!(actual.provider_name, expected.provider_name);
    }
}

#[test]
fn diagnostic_preview_redacts_before_shortening_and_preserves_suffix() {
    let preview = compact_response_preview(&oversized_secret(), 80);
    assert_safe(&preview);
    assert!(preview.chars().count() <= 83);
    assert!(preview.ends_with("..."));
    assert_eq!(compact_response_preview("plain", 5), "plain");
    assert_eq!(compact_response_preview("plain", 3), "pla...");
    assert_eq!(compact_response_preview("", 0), "");
    assert_eq!(compact_response_preview("plain", 0), "...");
}

#[test]
fn diagnostic_summary_bounds_the_entire_status_code_message() {
    let error = GatewayError::service_unavailable(oversized_secret()).with_code("ctx_busy");
    let summary = summarize_gateway_error(&error);
    assert_safe(&summary);
    assert!(summary.starts_with("status=503, code=ctx_busy, message="));
    let error = error.with_code("c".repeat(1024));
    assert_safe(&summarize_gateway_error(&error));
    assert_eq!(error.code.as_ref().unwrap().len(), 1024);
}

#[test]
fn diagnostic_preview_redacts_credentials_crossing_the_preview_boundary() {
    let body = format!("eyJ{}.{}.signature", "a".repeat(400), "b".repeat(400));
    for limit in [220, 320] {
        let preview = compact_response_preview(&body, limit);
        assert_eq!(preview, "[REDACTED]");
        let mut error = GatewayError::service_unavailable("base");
        append_gateway_error_fields(&mut error, &[("body_preview", preview)]);
        assert_eq!(error.message, "base; body_preview=[REDACTED]");
    }
}

#[test]
fn diagnostic_stream_parse_wrapper_preserves_plain_and_empty_contracts() {
    for (body, preview) in [(" plain ", "plain"), (" \n ", "<empty>")] {
        let error = wrap_gemini_canvas_stream_parse_error(
            body,
            GatewayError::service_unavailable("parse failed").with_code("parse_code"),
        );
        assert_eq!(
            error.message,
            format!("parse failed; stream_head={preview}; stream_tail={preview}")
        );
        assert_eq!(error.code.as_deref(), Some("parse_code"));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
    }
}

#[test]
fn diagnostic_stream_parse_debug_event_does_not_log_raw_credentials() {
    if isolate_stream_parse_log_capture() {
        return;
    }
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct CaptureWriter(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for CaptureWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let capture = Arc::new(Mutex::new(Vec::new()));
    let writer = CaptureWriter(Arc::clone(&capture));
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_ansi(false)
        .without_time()
        .with_writer(move || writer.clone())
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        wrap_gemini_canvas_stream_parse_error(
            &oversized_secret(),
            GatewayError::service_unavailable("parse failed"),
        );
    });
    let output = String::from_utf8(capture.lock().unwrap().clone()).unwrap();
    assert!(
        output.contains("body_preview="),
        "captured event: {output:?}"
    );
    assert!(output.contains("[REDACTED]"));
    assert!(!output.contains("fixture-token"));
    assert!(!output.contains("fixture-bearer"));
    assert!(output.len() < 1024);
}

fn isolate_stream_parse_log_capture() -> bool {
    use std::io::Read;
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    const CHILD_ENV: &str = "GATEWAY_TEST_STREAM_PARSE_LOG_CAPTURE_CHILD";
    if std::env::var_os(CHILD_ENV).as_deref() == Some(std::ffi::OsStr::new("1")) {
        return false;
    }

    struct ReapChild(Child);
    impl Drop for ReapChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    // Other parallel tests exercise this callsite without a dispatcher. Isolate the real event,
    // rather than install a global subscriber or relax assertions when capture is empty.
    let module = module_path!().split_once("::").unwrap().1;
    let test =
        format!("{module}::diagnostic_stream_parse_debug_event_does_not_log_raw_credentials");
    let mut child = ReapChild(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &test, "--nocapture"])
            .env(CHILD_ENV, "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            let mut output = String::new();
            child
                .0
                .stdout
                .take()
                .unwrap()
                .take(8192)
                .read_to_string(&mut output)
                .unwrap();
            assert!(status.success(), "isolated capture failed: {output}");
            assert!(
                output.contains("1 passed; 0 failed"),
                "missing capture test: {output}"
            );
            return true;
        }
        assert!(Instant::now() < deadline, "isolated capture timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[tokio::test]
async fn diagnostic_stream_parse_wrapper_redacts_before_head_and_tail_selection() {
    let jwt = format!("eyJ{}.{}.signature", "a".repeat(400), "b".repeat(400));
    for body in [SECRET.to_string(), jwt, oversized_secret()] {
        let error = wrap_gemini_canvas_stream_parse_error(
            &body,
            GatewayError::service_unavailable("parse failed").with_code("parse_code"),
        );
        assert!(!error.message.contains("eyJ"));
        assert!(!error.message.contains("signature"));
        assert_safe_wire(error).await;
    }
}

#[test]
fn diagnostic_append_sanitizes_empty_and_repeated_context_without_reclassifying() {
    for summary in [None, Some(" "), Some(SECRET)] {
        let error = GatewayError::service_unavailable(SECRET)
            .with_code("ctx_busy")
            .with_provider("gemini_canvas_compatible");
        let kind = error.kind;
        let hint = format!("{:?}", error.fallback_hint);
        let retryable = error.retryable;
        let mut error = append_gateway_error_summary(error, "context", summary);
        assert_safe(&error.message);
        for _ in 0..4 {
            error = append_gateway_error_summary(error, "stage", Some(&oversized_secret()));
            assert_safe(&error.message);
        }
        assert_eq!(error.kind, kind);
        assert_eq!(error.http_status, Some(503));
        assert_eq!(error.code.as_deref(), Some("ctx_busy"));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(error.retryable, retryable);
        assert_eq!(format!("{:?}", error.fallback_hint), hint);
    }
}

async fn assert_safe_wire(error: GatewayError) {
    let diagnostic_message = error.message.clone();
    let status = error.http_status.unwrap();
    let code = error.code.clone().unwrap();
    let kind = format!("{:?}", error.kind);
    let response = error.into_response();
    assert_eq!(response.status().as_u16(), status);
    let bytes = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_safe(body["error"]["message"].as_str().unwrap());
    assert_safe(&diagnostic_message);
    assert_eq!(body["error"]["code"], code);
    assert_eq!(body["error"]["type"], kind);
}

#[tokio::test]
async fn diagnostic_fields_preserve_order_and_metadata_but_bound_the_complete_wire_message() {
    let mut plain = GatewayError::service_unavailable("base").with_code("ctx_busy");
    append_gateway_error_fields(&mut plain, &[("first", "one".into()), ("empty", "".into())]);
    assert_eq!(plain.message, "base; first=one; empty=");
    for count in [0, 1, 4] {
        let mut error = GatewayError::service_unavailable(SECRET)
            .with_code("ctx_busy")
            .with_provider("gemini_canvas_compatible");
        let kind = error.kind;
        let retryable = error.retryable;
        let fallback = format!("{:?}", error.fallback_hint);
        let fields = vec![("context", oversized_secret()); count];
        for _ in 0..4 {
            append_gateway_error_fields(&mut error, &fields);
            assert_safe(&error.message);
        }
        assert_eq!(error.kind, kind);
        assert_eq!(error.retryable, retryable);
        assert_eq!(format!("{:?}", error.fallback_hint), fallback);
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_safe_wire(error).await;
    }
}

#[tokio::test]
async fn diagnostic_media_errors_sanitize_all_composed_inputs_before_wire() {
    let target = GeminiCanvasFollowupTarget::from_bootstrap(
        false,
        Some("/app/example"),
        None,
        GeminiCanvasPageTargetMode::Resolved,
    );
    let detail = oversized_secret();
    let url = "https://example.test/app?token=fixture-token";
    let provider = "gemini_canvas_compatible";
    for error in [
        build_gemini_canvas_media_followup_missing_asset_error(
            provider,
            gemini_canvas::GeminiCanvasMediaOperation::Image,
            &target,
            url,
            1,
            &detail,
        ),
        gemini_canvas_media_followup_failed_error(provider, &target, url, &detail),
        gemini_canvas_media_followup_bootstrap_failed_error(
            GeminiCanvasPageTargetMode::Resolved,
            "/app/example",
            url,
            &detail,
        ),
        gemini_canvas_media_followup_missing_result_error(provider, &target, url),
        finalize_gemini_canvas_media_followup_attempt_state(
            provider,
            &target,
            url,
            GeminiCanvasMediaFollowupAttemptState {
                last_body: None,
                last_error: Some(GatewayError::service_unavailable(SECRET).with_code("ctx_busy")),
            },
        )
        .unwrap_err(),
    ] {
        assert_safe_wire(error).await;
    }
}

#[tokio::test]
async fn diagnostic_page_errors_and_entries_sanitize_urls_details_and_previews() {
    let url = "https://example.test/app?token=fixture-token";
    let detail = oversized_secret();
    assert_safe(&gemini_canvas_conversation_page_poll_stage_entry(
        1, "fetch", &detail,
    ));
    assert_safe(&gemini_canvas_conversation_page_poll_url_entry(
        1, url, "fetch", &detail,
    ));
    assert_safe_wire(gemini_canvas_conversation_page_missing_asset_error(
        gemini_canvas::GeminiCanvasMediaOperation::Video,
        GeminiCanvasPageTargetMode::Resolved,
        "/app/example",
        1,
        30,
        &detail,
        SECRET,
    ))
    .await;
}

#[tokio::test]
async fn diagnostic_video_errors_sanitize_body_and_locator_context_before_wire() {
    let provider = "gemini_canvas_compatible";
    let detail = oversized_secret();
    for error in [
        build_gemini_canvas_video_completion_missing_asset_error(
            "/app/example",
            "c_example",
            "r_example",
            1,
            Some("job"),
            Some(&detail),
            Some(&detail),
        ),
        gemini_canvas_program_video_no_key_invalid_json_error(provider, &detail),
        gemini_canvas_program_video_no_key_poll_invalid_json_error(provider, &detail),
        gemini_canvas_video_followup_missing_locator_error(
            provider,
            "/app/example",
            "https://example.test/app?token=fixture-token",
        ),
    ] {
        assert_safe_wire(error).await;
    }
}
