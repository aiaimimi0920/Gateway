use super::{execute_driver, DriverResponse};
use std::time::Duration;

mod fixture;
use fixture::{request, Fixture};

const LIMIT_ERROR: &str = "credential automation driver response exceeded the size limit";

async fn invoke(
    mode: &str,
    large: bool,
    timeout_secs: u64,
    captured: bool,
) -> anyhow::Result<DriverResponse> {
    let fixture = Fixture::new(mode);
    let mut request = request(large);
    if large && mode != "closed-input" {
        // Exceed the Windows ChildStdin buffer to exercise actual pipe backpressure.
        request.provider.label = "a".repeat(8 * 1024 * 1024);
    }
    let result = tokio::time::timeout(
        Duration::from_secs(3),
        execute_driver(&fixture.config(), &fixture.driver(timeout_secs), &request),
    )
    .await;
    let observed = fixture.finish().await;
    assert!(observed.started, "script fixture must start a real child");
    assert!(
        observed.exited_promptly,
        "script child must exit before the contract returns"
    );
    let result = result.expect("script driver exceeded the I/O decision budget");
    if captured {
        assert_eq!(
            observed.request,
            Some(serde_json::to_value(request).unwrap())
        );
    }
    result
}

fn assert_ready(response: DriverResponse) {
    assert_eq!(response.credentials.len(), 1);
    assert_eq!(response.credentials[0].id.as_deref(), Some("draft-a"));
    assert!(response.prune.is_empty());
    assert_eq!(response.message.as_deref(), Some("ready"));
}

fn assert_exact_limit(response: DriverResponse) {
    let overhead = br#"{"credentials":[],"prune":[],"message":""}"#.len();
    assert!(response.credentials.is_empty());
    assert!(response.prune.is_empty());
    let message = response.message.unwrap();
    assert_eq!(message.len(), 2 * 1024 * 1024 - overhead);
    assert!(message.bytes().all(|byte| byte == b'a'));
}

#[tokio::test]
async fn script_request_and_response_contract_are_preserved() {
    assert_ready(invoke("normal", false, 10, true).await.unwrap());
}

#[tokio::test]
async fn script_accepts_exact_limit_stdout_without_truncation() {
    assert_exact_limit(invoke("exact-limit", false, 10, true).await.unwrap());
}

#[tokio::test]
async fn script_rejects_finite_oversize_with_the_existing_error() {
    assert_eq!(
        invoke("finite-oversize", false, 10, true)
            .await
            .unwrap_err()
            .to_string(),
        LIMIT_ERROR
    );
}

#[tokio::test]
async fn script_rejects_open_oversize_without_waiting_for_exit() {
    assert_eq!(
        invoke("open-oversize", false, 10, true)
            .await
            .unwrap_err()
            .to_string(),
        LIMIT_ERROR
    );
}

#[tokio::test]
async fn script_timeout_includes_blocked_stdin() {
    assert_eq!(
        invoke("ignore-input", true, 1, false)
            .await
            .unwrap_err()
            .to_string(),
        "credential automation script timed out"
    );
}

#[tokio::test]
async fn script_drains_stdout_while_writing_large_input() {
    assert_exact_limit(invoke("stdout-duplex", true, 10, true).await.unwrap());
}

#[tokio::test]
async fn script_discards_stderr_while_writing_large_input() {
    assert_ready(invoke("stderr-duplex", true, 10, true).await.unwrap());
}

#[tokio::test]
async fn script_malformed_json_preserves_the_existing_error() {
    assert_eq!(
        invoke("malformed", false, 10, true)
            .await
            .unwrap_err()
            .to_string(),
        "credential automation driver returned invalid JSON"
    );
}

#[tokio::test]
async fn script_nonzero_exit_preserves_the_existing_error() {
    assert_eq!(
        invoke("nonzero", false, 10, true)
            .await
            .unwrap_err()
            .to_string(),
        "credential automation script returned a non-zero exit status"
    );
}

#[tokio::test]
async fn script_wait_timeout_reaps_the_child() {
    assert_eq!(
        invoke("wait-timeout", false, 1, true)
            .await
            .unwrap_err()
            .to_string(),
        "credential automation script timed out"
    );
}

#[tokio::test]
async fn script_closed_stdin_preserves_the_existing_input_error() {
    assert_eq!(
        invoke("closed-input", true, 10, false)
            .await
            .unwrap_err()
            .to_string(),
        "failed to write credential automation script input"
    );
}

#[tokio::test]
async fn script_cancellation_reaps_a_blocked_child() {
    let fixture = Fixture::new("ignore-input");
    let config = fixture.config();
    let driver = fixture.driver(10);
    let request = request(true);
    let task = tokio::spawn(async move { execute_driver(&config, &driver, &request).await });
    let started = fixture.wait_for_start().await;
    task.abort();
    let outcome = task.await;
    let observed = fixture.finish().await;
    assert!(started && observed.started);
    assert!(observed.exited_promptly);
    assert!(outcome.unwrap_err().is_cancelled());
}
