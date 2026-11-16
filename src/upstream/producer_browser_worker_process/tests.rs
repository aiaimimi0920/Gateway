//! Existing supervisor regression tests, moved without behavior changes.
use std::io::Write;
use std::time::Instant;

use super::*;

const FIXTURE_MODE_ENV: &str = "NEURO_GATEWAY_PRODUCER_WORKER_PROCESS_FIXTURE";

fn fixture_command(mode: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().expect("current test executable"));
    command
        .arg("producer_browser_worker_process_fixture")
        .arg("--nocapture")
        .arg("--test-threads=1")
        .env(FIXTURE_MODE_ENV, mode);
    command
}

async fn execute_fixture(
    mode: &str,
    worker_timeout: Duration,
    limits: WorkerOutputLimits,
) -> Result<ProducerBrowserWorkerProcessOutput, GatewayError> {
    execute_worker_command(
        fixture_command(mode),
        PathBuf::from("producer-browser-worker-process-fixture"),
        br#"{"request":"fixture"}"#.to_vec(),
        worker_timeout,
        limits,
    )
    .await
}

#[test]
fn producer_browser_worker_process_fixture() {
    let Ok(mode) = std::env::var(FIXTURE_MODE_ENV) else {
        return;
    };
    match mode.as_str() {
        "output" => {
            print!("producer-worker-stdout-marker");
            eprint!("producer-worker-stderr-marker");
            std::io::stdout().flush().expect("flush fixture stdout");
            std::io::stderr().flush().expect("flush fixture stderr");
        }
        "oversized-stdout" => {
            std::io::stdout()
                .write_all(&vec![b'x'; 2 * 1024])
                .expect("write oversized fixture stdout");
        }
        "oversized-stderr" => {
            std::io::stderr()
                .write_all(&vec![b'x'; 2 * 1024])
                .expect("write oversized fixture stderr");
        }
        "nonzero" => {
            eprint!("producer-worker-nonzero-marker");
            std::io::stderr().flush().expect("flush fixture stderr");
            std::process::exit(23);
        }
        "timeout" => std::thread::sleep(Duration::from_secs(10)),
        other => panic!("unknown fixture mode: {other}"),
    }
}

#[tokio::test]
async fn worker_process_collects_stdout_and_stderr_concurrently() {
    let output = execute_fixture(
        "output",
        Duration::from_secs(5),
        WorkerOutputLimits {
            stdout: 64 * 1024,
            stderr: 64 * 1024,
        },
    )
    .await
    .expect("fixture output");

    assert!(String::from_utf8_lossy(&output.stdout).contains("producer-worker-stdout-marker"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("producer-worker-stderr-marker"));
}

#[tokio::test]
async fn worker_process_rejects_oversized_stdout_and_stderr() {
    for (mode, expected_stream) in [
        ("oversized-stdout", "stdout"),
        ("oversized-stderr", "stderr"),
    ] {
        let error = execute_fixture(
            mode,
            Duration::from_secs(5),
            WorkerOutputLimits {
                stdout: if expected_stream == "stdout" {
                    512
                } else {
                    64 * 1024
                },
                stderr: if expected_stream == "stderr" {
                    512
                } else {
                    64 * 1024
                },
            },
        )
        .await
        .expect_err("oversized worker output should fail");

        assert_eq!(
            error.code.as_deref(),
            Some("producer_browser_worker_output_too_large")
        );
        assert!(error.message.contains(expected_stream));
    }
}

#[tokio::test]
async fn worker_process_rejects_nonzero_exit_with_stable_error() {
    let error = execute_fixture(
        "nonzero",
        Duration::from_secs(5),
        WorkerOutputLimits {
            stdout: 64 * 1024,
            stderr: 64 * 1024,
        },
    )
    .await
    .expect_err("nonzero worker exit should fail");

    assert_eq!(
        error.code.as_deref(),
        Some("producer_browser_worker_nonzero_exit")
    );
    assert!(error.message.contains("code 23"));
}

#[tokio::test]
async fn worker_process_timeout_kills_and_reaps_child_promptly() {
    let started = Instant::now();
    let error = execute_fixture(
        "timeout",
        Duration::from_millis(50),
        WorkerOutputLimits {
            stdout: 64 * 1024,
            stderr: 64 * 1024,
        },
    )
    .await
    .expect_err("sleeping worker should time out");

    assert_eq!(
        error.code.as_deref(),
        Some("producer_browser_worker_timeout")
    );
    assert!(started.elapsed() < Duration::from_secs(3));
}

#[tokio::test]
async fn cancelling_worker_future_kills_and_reaps_child() {
    let (cleanup_probe, cleanup_result) = oneshot::channel();
    let worker = tokio::spawn(execute_worker_command_observed(
        fixture_command("timeout"),
        PathBuf::from("producer-browser-worker-process-fixture"),
        br#"{"request":"fixture"}"#.to_vec(),
        Duration::from_secs(5),
        WorkerOutputLimits {
            stdout: 64 * 1024,
            stderr: 64 * 1024,
        },
        WorkerCleanup {
            cancellation_probe: Some(cleanup_probe),
            ..WorkerCleanup::default()
        },
    ));
    tokio::time::sleep(Duration::from_millis(100)).await;
    worker.abort();
    assert!(worker
        .await
        .expect_err("worker task should be cancelled")
        .is_cancelled());

    let reaped = timeout(Duration::from_secs(3), cleanup_result)
        .await
        .expect("cancel cleanup should complete")
        .expect("cancel cleanup result");
    assert!(reaped, "cancelled worker child was not reaped");
}

#[tokio::test]
async fn cancelling_console_worker_reclaims_parent_owned_profile() {
    let workspace = ConsoleBrowserWorkspace::create().await.unwrap();
    let root = workspace.path().to_path_buf();
    tokio::fs::create_dir(root.join("browser-profile"))
        .await
        .unwrap();
    tokio::fs::write(root.join("browser-profile/private-fixture"), b"fixture")
        .await
        .unwrap();
    let mut command = fixture_command("timeout");
    workspace.configure(&mut command);
    let (probe, result) = oneshot::channel();
    let worker = tokio::spawn(execute_worker_command_observed(
        command,
        PathBuf::from("console-browser-workspace-fixture"),
        b"{}".to_vec(),
        Duration::from_secs(10),
        WorkerOutputLimits {
            stdout: 64 * 1024,
            stderr: 64 * 1024,
        },
        WorkerCleanup {
            cancellation_probe: Some(probe),
            workspace: Some(workspace),
        },
    ));
    tokio::time::sleep(Duration::from_millis(100)).await;
    worker.abort();
    assert!(worker.await.unwrap_err().is_cancelled());
    assert!(timeout(Duration::from_secs(3), result)
        .await
        .unwrap()
        .unwrap());
    timeout(Duration::from_secs(6), async {
        while root.exists() {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("profile reclaimed after forced worker cancellation");
}
