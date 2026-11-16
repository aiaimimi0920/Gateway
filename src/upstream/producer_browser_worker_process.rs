use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use tokio::process::{Child, Command};
use tokio::sync::{oneshot, OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinHandle;
use tokio::time::timeout;

use super::console_browser_workspace::ConsoleBrowserWorkspace;
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

#[derive(Default)]
struct WorkerCleanup {
    cancellation_probe: Option<oneshot::Sender<bool>>,
    workspace: Option<ConsoleBrowserWorkspace>,
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
    execute_browser_worker_process_with_limits(
        launch,
        worker_timeout,
        WorkerOutputLimits::production(),
    )
    .await
}

/// Reuse the supervisor for metadata-only workers without allocating the Producer media budget.
pub(super) async fn execute_browser_worker_process_with_limits(
    launch: PreparedProducerBrowserWorkerLaunch,
    worker_timeout: Duration,
    limits: WorkerOutputLimits,
) -> Result<ProducerBrowserWorkerProcessOutput, GatewayError> {
    let mut command = Command::new(&launch.node_bin);
    command.arg(&launch.script_path);
    execute_worker_command(
        command,
        launch.script_path,
        launch.stdin_json,
        worker_timeout,
        limits,
    )
    .await
}

/// Console capture uses the same cancellation/reaping owner, with a much smaller output budget.
pub(crate) async fn execute_console_browser_worker(
    launch: PreparedProducerBrowserWorkerLaunch,
    workspace: ConsoleBrowserWorkspace,
) -> Result<ProducerBrowserWorkerProcessOutput, GatewayError> {
    let mut command = Command::new(&launch.node_bin);
    command.arg(&launch.script_path);
    workspace.configure(&mut command);
    execute_worker_command_observed(
        command,
        launch.script_path,
        launch.stdin_json,
        Duration::from_secs(225),
        WorkerOutputLimits {
            stdout: 128 * 1024,
            stderr: 4096,
        },
        WorkerCleanup {
            workspace: Some(workspace),
            ..WorkerCleanup::default()
        },
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
        WorkerCleanup::default(),
    )
    .await
}

async fn execute_worker_command_observed(
    command: Command,
    script_path: PathBuf,
    stdin_json: Vec<u8>,
    worker_timeout: Duration,
    limits: WorkerOutputLimits,
    cleanup: WorkerCleanup,
) -> Result<ProducerBrowserWorkerProcessOutput, GatewayError> {
    let (cancellation, cancellation_receiver) = oneshot::channel();
    let task = tokio::spawn(run_worker_command(
        command,
        script_path,
        stdin_json,
        worker_timeout,
        limits,
        cancellation_receiver,
        cleanup,
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
    mut cleanup: WorkerCleanup,
) -> Result<ProducerBrowserWorkerProcessOutput, GatewayError> {
    let worker_permit = tokio::select! {
        biased;
        _ = &mut cancellation => {
            if let Some(probe) = cleanup.cancellation_probe.take() {
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
    let result = match stop {
        WorkerExecutionStop::Completed(Ok(output)) => {
            drop(process_tree);
            Ok(output)
        }
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
            if let Some(probe) = cleanup.cancellation_probe.take() {
                let _ = probe.send(reaped);
            }
            Err(worker_cancelled_error())
        }
    };
    if let Some(workspace) = cleanup.workspace.take() {
        if !workspace.cleanup().await {
            tracing::warn!(
                code = "console_browser_scratch_cleanup_pending",
                "Browser scratch cleanup will retry"
            );
        }
    }
    result
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
mod tests;
