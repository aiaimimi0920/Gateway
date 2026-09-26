use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use tokio::process::{Child, Command};
use tokio::sync::{oneshot, OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinHandle;
use tokio::time::timeout;

use crate::error::GatewayError;

use super::producer_browser_worker_io::{
    collect_worker_process_output, ProducerBrowserWorkerProcessOutput, WorkerOutputLimits,
};
use super::producer_browser_worker_tree::ProcessTreeGuard;
#[derive(Debug)]
pub(crate) struct PreparedProducerBrowserWorkerLaunch {
    pub(crate) node_bin: String,
    pub(crate) script_path: PathBuf,
    pub(crate) stdin_json: Vec<u8>,
}

const WORKER_REAP_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_CONCURRENT_WORKER_PROCESSES: usize = 16;

static WORKER_PROCESS_SLOTS: LazyLock<Arc<Semaphore>> =
    LazyLock::new(|| Arc::new(Semaphore::new(MAX_CONCURRENT_WORKER_PROCESSES)));

struct WorkerTaskGuard {
    cancellation: Option<oneshot::Sender<()>>,
    task: JoinHandle<Result<ProducerBrowserWorkerProcessOutput, GatewayError>>,
}

enum WorkerExecutionStop {
    Completed(Result<ProducerBrowserWorkerProcessOutput, GatewayError>),
    TimedOut,
    Cancelled,
}

impl WorkerTaskGuard {
    async fn wait(mut self) -> Result<ProducerBrowserWorkerProcessOutput, GatewayError> {
        let result = (&mut self.task).await.map_err(|_| {
            crate::protocol::producer::browser_worker_wait_failed_error(
                "worker supervisor task failed",
            )
        })?;
        self.cancellation.take();
        result
    }
}

impl Drop for WorkerTaskGuard {
    fn drop(&mut self) {
        if let Some(cancellation) = self.cancellation.take() {
            let _ = cancellation.send(());
        }
    }
}

pub(crate) async fn execute_producer_browser_worker_process(
    launch: PreparedProducerBrowserWorkerLaunch,
    worker_timeout: Duration,
) -> Result<ProducerBrowserWorkerProcessOutput, GatewayError> {
    let mut command = Command::new(&launch.node_bin);
    command.arg(&launch.script_path);
    execute_worker_command(
        command,
        launch.script_path,
        launch.stdin_json,
        worker_timeout,
        WorkerOutputLimits::production(),
    )
    .await
}

async fn execute_worker_command(
    command: Command,
    script_path: PathBuf,
    stdin_json: Vec<u8>,
    worker_timeout: Duration,
    limits: WorkerOutputLimits,
) -> Result<ProducerBrowserWorkerProcessOutput, GatewayError> {
    execute_worker_command_observed(
        command,
        script_path,
        stdin_json,
        worker_timeout,
        limits,
        None,
    )
    .await
}

async fn execute_worker_command_observed(
    command: Command,
    script_path: PathBuf,
    stdin_json: Vec<u8>,
    worker_timeout: Duration,
    limits: WorkerOutputLimits,
    cancellation_cleanup_probe: Option<oneshot::Sender<bool>>,
) -> Result<ProducerBrowserWorkerProcessOutput, GatewayError> {
    let (cancellation, cancellation_receiver) = oneshot::channel();
    let task = tokio::spawn(run_worker_command(
        command,
        script_path,
        stdin_json,
        worker_timeout,
        limits,
        cancellation_receiver,
        cancellation_cleanup_probe,
    ));
    WorkerTaskGuard {
        cancellation: Some(cancellation),
        task,
    }
    .wait()
    .await
}

async fn run_worker_command(
    mut command: Command,
    script_path: PathBuf,
    stdin_json: Vec<u8>,
    worker_timeout: Duration,
    limits: WorkerOutputLimits,
    mut cancellation: oneshot::Receiver<()>,
    mut cancellation_cleanup_probe: Option<oneshot::Sender<bool>>,
) -> Result<ProducerBrowserWorkerProcessOutput, GatewayError> {
    let worker_permit = tokio::select! {
        biased;
        _ = &mut cancellation => {
            if let Some(probe) = cancellation_cleanup_probe.take() {
                let _ = probe.send(true);
            }
            return Err(worker_cancelled_error());
        }
        permit = WORKER_PROCESS_SLOTS.clone().acquire_owned() => {
            permit.expect("producer browser worker semaphore should never close")
        }
    };
    command
        .kill_on_drop(true)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut process_tree = ProcessTreeGuard::prepare(&mut command).map_err(|error| {
        crate::protocol::producer::browser_worker_spawn_failed_error(
            script_path.as_path(),
            error.to_string().as_str(),
        )
    })?;
    let mut child = command.spawn().map_err(|error| {
        crate::protocol::producer::browser_worker_spawn_failed_error(
            script_path.as_path(),
            error.to_string().as_str(),
        )
    })?;
    if let Err(error) = process_tree.attach(&child) {
        let gateway_error = crate::protocol::producer::browser_worker_spawn_failed_error(
            script_path.as_path(),
            format!("failed to isolate worker process tree: {error}").as_str(),
        );
        terminate_and_reap(child, None, worker_permit).await;
        return Err(gateway_error);
    }

    let Some(stdin) = child.stdin.take() else {
        terminate_and_reap(child, Some(process_tree), worker_permit).await;
        return Err(crate::protocol::producer::browser_worker_wait_failed_error(
            "stdin pipe was unavailable",
        ));
    };
    let Some(stdout) = child.stdout.take() else {
        terminate_and_reap(child, Some(process_tree), worker_permit).await;
        return Err(crate::protocol::producer::browser_worker_wait_failed_error(
            "stdout pipe was unavailable",
        ));
    };
    let Some(stderr) = child.stderr.take() else {
        terminate_and_reap(child, Some(process_tree), worker_permit).await;
        return Err(crate::protocol::producer::browser_worker_wait_failed_error(
            "stderr pipe was unavailable",
        ));
    };

    let stop = {
        let operation =
            collect_worker_process_output(&mut child, stdin, stdout, stderr, &stdin_json, limits);
        tokio::select! {
            outcome = timeout(worker_timeout, operation) => match outcome {
                Ok(outcome) => WorkerExecutionStop::Completed(outcome),
                Err(_) => WorkerExecutionStop::TimedOut,
            },
            _ = &mut cancellation => WorkerExecutionStop::Cancelled,
        }
    };
    match stop {
        WorkerExecutionStop::Completed(Ok(output)) => Ok(output),
        WorkerExecutionStop::Completed(Err(error)) => {
            terminate_and_reap(child, Some(process_tree), worker_permit).await;
            Err(error)
        }
        WorkerExecutionStop::TimedOut => {
            terminate_and_reap(child, Some(process_tree), worker_permit).await;
            Err(crate::protocol::producer::browser_worker_timeout_error())
        }
        WorkerExecutionStop::Cancelled => {
            let reaped = terminate_and_reap(child, Some(process_tree), worker_permit).await;
            if let Some(probe) = cancellation_cleanup_probe.take() {
                let _ = probe.send(reaped);
            }
            Err(worker_cancelled_error())
        }
    }
}

fn worker_cancelled_error() -> GatewayError {
    crate::protocol::producer::browser_worker_wait_failed_error("worker execution was cancelled")
}

async fn terminate_and_reap(
    mut child: Child,
    process_tree: Option<ProcessTreeGuard>,
    worker_permit: OwnedSemaphorePermit,
) -> bool {
    if child.id().is_none() {
        return true;
    }
    if let Some(process_tree) = process_tree.as_ref() {
        process_tree.terminate();
    }
    let _ = child.start_kill();
    match timeout(WORKER_REAP_TIMEOUT, child.wait()).await {
        Ok(Ok(_)) => true,
        Ok(Err(_)) => false,
        Err(_) => {
            tokio::spawn(async move {
                let _worker_permit = worker_permit;
                reap_worker_until_exit(child, process_tree).await;
            });
            false
        }
    }
}

async fn reap_worker_until_exit(mut child: Child, process_tree: Option<ProcessTreeGuard>) -> bool {
    if let Some(process_tree) = process_tree.as_ref() {
        process_tree.terminate();
    }
    let _ = child.start_kill();
    child.wait().await.is_ok()
}

#[cfg(test)]
mod tests {
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
            Some(cleanup_probe),
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
}
