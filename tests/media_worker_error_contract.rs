#![cfg(all(
    feature = "line-lumalabs-web-reverse-api",
    feature = "line-suno-web-reverse-api",
    feature = "line-udio-web-reverse-api"
))]

use axum::response::IntoResponse;
use neuro_gateway::error::{ErrorKind, GatewayError};
use neuro_gateway::protocol::{lumalabs, suno, udio};
use std::path::Path;

struct WorkerErrors {
    label: &'static str,
    prefix: &'static str,
    provider: &'static str,
    empty: fn(&str) -> GatewayError,
    parse: fn(&str, &str) -> GatewayError,
    wait: fn(&str) -> GatewayError,
    stdin: fn(&str) -> GatewayError,
    serialize: fn(&str) -> GatewayError,
    spawn: fn(&Path, &str) -> GatewayError,
    remote: Option<fn(&str) -> GatewayError>,
}

const WORKERS: [WorkerErrors; 3] = [
    WorkerErrors {
        label: "LumaLabs",
        prefix: "lumalabs",
        provider: "lumalabs_compatible",
        empty: lumalabs::empty_browser_worker_output_error,
        parse: lumalabs::browser_worker_output_parse_error,
        wait: lumalabs::browser_worker_wait_failed_error,
        stdin: lumalabs::browser_worker_stdin_error,
        serialize: lumalabs::browser_worker_input_serialize_error,
        spawn: lumalabs::browser_worker_spawn_failed_error,
        remote: None,
    },
    WorkerErrors {
        label: "Suno",
        prefix: "suno",
        provider: "suno_compatible",
        empty: suno::empty_browser_worker_output_error,
        parse: suno::browser_worker_output_parse_error,
        wait: suno::browser_worker_wait_failed_error,
        stdin: suno::browser_worker_stdin_error,
        serialize: suno::browser_worker_input_serialize_error,
        spawn: suno::browser_worker_spawn_failed_error,
        remote: Some(suno::remote_browser_worker_result_parse_error),
    },
    WorkerErrors {
        label: "Udio",
        prefix: "udio",
        provider: "udio_compatible",
        empty: udio::empty_browser_worker_output_error,
        parse: udio::browser_worker_output_parse_error,
        wait: udio::browser_worker_wait_failed_error,
        stdin: udio::browser_worker_stdin_error,
        serialize: udio::browser_worker_input_serialize_error,
        spawn: udio::browser_worker_spawn_failed_error,
        remote: Some(udio::remote_browser_worker_result_parse_error),
    },
];

fn dynamic_errors(worker: &WorkerErrors, detail: &str) -> Vec<(GatewayError, &'static str)> {
    let mut errors = vec![
        ((worker.empty)(detail), "browser_worker_empty_output"),
        (
            (worker.parse)(detail, "safe output"),
            "browser_worker_output_parse_failed",
        ),
        (
            (worker.parse)("expected value", detail),
            "browser_worker_output_parse_failed",
        ),
        ((worker.wait)(detail), "browser_worker_wait_failed"),
        ((worker.stdin)(detail), "browser_worker_stdin_failed"),
        (
            (worker.serialize)(detail),
            "browser_worker_input_serialize_failed",
        ),
        (
            (worker.spawn)(Path::new("worker.mjs"), detail),
            "browser_worker_spawn_failed",
        ),
    ];
    if let Some(remote) = worker.remote {
        errors.push((remote(detail), "remote_result_parse_failed"));
    }
    errors
}

fn assert_identity(worker: &WorkerErrors, error: &GatewayError, suffix: &str) {
    assert_eq!(error.kind, ErrorKind::ServerError);
    assert_eq!(error.http_status, Some(500));
    assert_eq!(error.provider_name.as_deref(), Some(worker.provider));
    assert_eq!(
        error.code.as_deref(),
        Some(format!("{}_{suffix}", worker.prefix).as_str())
    );
    let original = GatewayError::server_error("safe detail");
    assert_eq!(error.retryable, original.retryable);
    assert_eq!(
        serde_json::to_value(&error.fallback_hint).unwrap(),
        serde_json::to_value(original.fallback_hint).unwrap()
    );
}

fn auth_detail() -> &'static str {
    "Authorization: Bearer fake-bearer-secret\r\nCookie: sid=fake-cookie-secret\r\n\
     token=fake-assignment-secret\nsk-test-fake-prefix-secret eyJfake.fakepayload.fakesignature"
}

fn assert_safe(message: &str) {
    assert!(!message.chars().any(char::is_control));
    assert!(message.chars().count() <= 512);
    for secret in [
        "fake-bearer-secret",
        "fake-cookie-secret",
        "fake-assignment-secret",
        "sk-test-fake-prefix-secret",
        "eyJfake.fakepayload.fakesignature",
    ] {
        assert!(!message.contains(secret), "worker detail leaked: {secret}");
    }
}

#[test]
fn every_dynamic_error_redacts_auth_material_and_preserves_classification() {
    for worker in &WORKERS {
        for (error, suffix) in dynamic_errors(worker, auth_detail()) {
            assert_identity(worker, &error, suffix);
            assert_safe(&error.message);
            assert!(error.message.contains("[REDACTED]"));
        }
    }
}

#[test]
fn controls_are_replaced_without_discarding_safe_words() {
    for worker in &WORKERS {
        for (error, _) in dynamic_errors(worker, "phase\0one\r\nfailed\tplease") {
            assert_safe(&error.message);
            assert!(error.message.contains("phase one  failed please"));
        }
    }
}

#[test]
fn unicode_details_and_composed_parse_messages_obey_the_final_character_budget() {
    let detail = "\u{754c}".repeat(700);
    for worker in &WORKERS {
        for (error, _) in dynamic_errors(worker, &detail) {
            assert_safe(&error.message);
            assert!(error.message.ends_with("..."));
        }
        let error = (worker.parse)(&"e".repeat(400), &detail);
        assert_safe(&error.message);
        assert!(error.message.contains("stdout:"));
    }
}

#[test]
fn oversized_details_are_omitted_before_regex_processing() {
    let detail = format!("{}token=fake-oversized-tail", "\u{754c}".repeat(6000));
    for worker in &WORKERS {
        for (error, _) in dynamic_errors(worker, &detail) {
            assert_safe(&error.message);
            assert!(error.message.contains("safe processing limit exceeded"));
            assert!(!error.message.contains("fake-oversized-tail"));
        }
        let error = (worker.parse)(&detail, &detail);
        assert_safe(&error.message);
        assert_eq!(
            error
                .message
                .matches("safe processing limit exceeded")
                .count(),
            2
        );
    }
}

#[test]
fn detail_admission_includes_exactly_sixteen_kibibytes() {
    let accepted = "z".repeat(16 * 1024);
    let omitted = format!("{accepted}z");
    for worker in &WORKERS {
        for (error, _) in dynamic_errors(worker, &accepted) {
            assert_safe(&error.message);
            assert!(!error.message.contains("safe processing limit exceeded"));
            assert!(error.message.contains("zzzz"));
        }
        for (error, _) in dynamic_errors(worker, &omitted) {
            assert_safe(&error.message);
            assert!(error.message.contains("safe processing limit exceeded"));
            assert!(!error.message.contains("zzzz"));
        }
    }
}

#[test]
fn spawn_errors_hide_local_paths_and_keep_the_safe_cause() {
    for worker in &WORKERS {
        for path in [
            "C:/Users/private-user/runtime/browser-worker.mjs",
            "/srv/private-user/runtime/browser-worker.mjs",
        ] {
            let error = (worker.spawn)(Path::new(path), "permission denied");
            assert_identity(worker, &error, "browser_worker_spawn_failed");
            assert_eq!(
                error.message,
                format!(
                    "Failed to launch {} browser worker: permission denied",
                    worker.label
                )
            );
            assert!(!error.message.contains("private-user"));
            assert!(!error.message.contains("browser-worker.mjs"));
        }
    }
}

#[tokio::test]
async fn http_error_publication_contains_only_sanitized_worker_details() {
    for worker in &WORKERS {
        for (error, suffix) in dynamic_errors(worker, auth_detail()) {
            let response = error.into_response();
            assert_eq!(response.status(), 500);
            let bytes = axum::body::to_bytes(response.into_body(), 4096)
                .await
                .unwrap();
            let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(body["error"]["code"], format!("{}_{suffix}", worker.prefix));
            let message = body["error"]["message"].as_str().unwrap();
            assert_safe(message);
            assert!(message.contains("[REDACTED]"));
        }
    }
}
