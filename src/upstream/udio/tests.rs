//! Explicit launch fixtures avoid process-wide environment mutation and live accounts.
use super::browser_worker;
use crate::{
    error::{FallbackHint, GatewayError},
    upstream::PreparedProducerBrowserWorkerLaunch,
};
use serde_json::json;
use std::{path::PathBuf, time::Duration};

fn launch(mode: &str) -> PreparedProducerBrowserWorkerLaunch {
    PreparedProducerBrowserWorkerLaunch {
        node_bin: "node".into(),
        script_path: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("scripts/tests/fixtures/udio-worker-process.mjs"),
        stdin_json: serde_json::to_vec(&json!({"mode":mode})).unwrap(),
    }
}
fn assert_error(error: GatewayError, code: &str) {
    assert_eq!(error.code.as_deref(), Some(code));
    assert_eq!(error.provider_name.as_deref(), Some("udio_compatible"));
    assert!(!error.retryable);
    assert!(!crate::retry::should_retry(
        &error,
        &crate::retry::RetryPolicy::default()
    ));
    assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
    for private in [
        "synthetic-private-token",
        "private invalid",
        "private empty",
        "producer_",
    ] {
        assert!(!error.message.contains(private));
    }
}
#[tokio::test]
async fn udio_no_replay_red_nonzero_is_not_success() {
    assert_error(
        browser_worker::run(launch("nonzero"), Duration::from_secs(5))
            .await
            .unwrap_err(),
        "udio_browser_worker_nonzero_exit",
    );
}
#[tokio::test]
async fn udio_no_replay_red_output_has_metadata_budget() {
    assert_error(
        browser_worker::run(launch("stdout"), Duration::from_secs(5))
            .await
            .unwrap_err(),
        "udio_browser_worker_output_too_large",
    );
}
#[tokio::test]
async fn udio_no_replay_red_business_error_stops_retry() {
    assert_error(
        browser_worker::run(launch("error"), Duration::from_secs(5))
            .await
            .unwrap_err(),
        "udio_fixture_error",
    );
}

#[tokio::test]
async fn udio_worker_checks_stderr_budget() {
    assert_error(
        browser_worker::run(launch("stderr"), Duration::from_secs(5))
            .await
            .unwrap_err(),
        "udio_browser_worker_output_too_large",
    );
}
#[tokio::test]
async fn udio_worker_success_keeps_pending_identity() {
    let result = browser_worker::run(launch("success"), Duration::from_secs(5))
        .await
        .unwrap();
    assert_eq!(result.track_ids, ["fixture"]);
    assert!(!result.completed);
}
#[tokio::test]
async fn udio_worker_invalid_output_is_secret_safe() {
    for (mode, code) in [
        ("invalid", "udio_browser_worker_output_parse_failed"),
        ("empty", "udio_browser_worker_empty_output"),
        ("utf8", "udio_browser_worker_output_parse_failed"),
    ] {
        assert_error(
            browser_worker::run(launch(mode), Duration::from_secs(5))
                .await
                .unwrap_err(),
            code,
        );
    }
}
#[tokio::test]
async fn udio_worker_rejects_large_input_before_spawn() {
    let mut input = launch("success");
    input.node_bin = "must-not-launch-fixture".into();
    input.stdin_json = vec![b'x'; 1024 * 1024 + 1];
    assert_error(
        browser_worker::run(input, Duration::from_secs(5))
            .await
            .unwrap_err(),
        "udio_browser_worker_input_too_large",
    );
}
#[tokio::test]
async fn udio_worker_spawn_failure_preserves_provider_identity() {
    let mut input = launch("success");
    input.node_bin = "must-not-launch-fixture".into();
    assert_error(
        browser_worker::run(input, Duration::from_secs(5))
            .await
            .unwrap_err(),
        "udio_browser_worker_spawn_failed",
    );
}

mod lifecycle;
