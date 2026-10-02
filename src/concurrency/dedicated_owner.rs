//! Run admitted blocking-owner work without Tokio's shared blocking pool.
use std::future::Future;

use tokio::sync::{oneshot, OwnedMutexGuard};

use crate::error::GatewayError;

pub(crate) async fn run<T, Fut>(
    permit: OwnedMutexGuard<()>,
    operation: Fut,
    runtime_stopped: fn() -> GatewayError,
    task_failed: fn() -> GatewayError,
) -> Result<T, GatewayError>
where
    T: Send + 'static,
    Fut: Future<Output = Result<T, GatewayError>> + Send + 'static,
{
    let (cancel, cancelled) = oneshot::channel::<()>();
    let (result_tx, result_rx) = oneshot::channel();
    let (done_tx, done) = oneshot::channel::<()>();
    let worker = std::thread::Builder::new()
        .name("gw-dedicated-owner".to_string())
        .spawn(move || {
            let _permit = permit;
            let worker_runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(_) => {
                    let _ = result_tx.send(Err(task_failed()));
                    let _ = done_tx.send(());
                    return;
                }
            };
            let result = worker_runtime.block_on(async move {
                tokio::select! {
                    biased;
                    _ = cancelled => Err(runtime_stopped()),
                    result = operation => result,
                }
            });
            if let Err(error) = &result {
                tracing::warn!(kind = ?error.kind, "dedicated owner operation failed");
            }
            let _ = result_tx.send(result);
            let _ = done_tx.send(());
        })
        .map_err(|_| task_failed())?;
    // Teardown may drop the monitor before its first poll; capture an armed guard.
    let guard = WorkerGuard {
        cancel: Some(cancel),
        worker: Some(worker),
    };
    let monitor = tokio::spawn(async move {
        let _guard = guard;
        let _ = done.await;
    });
    let result = result_rx.await;
    let _ = monitor.await;
    result.map_err(|_| task_failed())?
}

struct WorkerGuard {
    cancel: Option<oneshot::Sender<()>>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl Drop for WorkerGuard {
    fn drop(&mut self) {
        drop(self.cancel.take());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
