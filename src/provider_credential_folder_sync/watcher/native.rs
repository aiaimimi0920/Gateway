//! Native watcher setup and destruction stay on their owning OS thread.
use std::sync::mpsc;
use std::time::Duration;

use tokio::sync::oneshot;

use crate::error::GatewayError;

pub(super) struct NativeWatchOwner {
    stop: Option<mpsc::Sender<()>>,
    ready: Option<oneshot::Receiver<Option<bool>>>,
    done: oneshot::Receiver<()>,
    running: bool,
}

impl NativeWatchOwner {
    pub(super) fn spawn<T, F>(prepare: F) -> Result<Self, GatewayError>
    where
        T: Send + 'static,
        F: FnOnce() -> Option<(T, bool)> + Send + 'static,
    {
        let (stop_tx, stop) = mpsc::channel();
        let (ready_tx, ready) = oneshot::channel();
        let (done_tx, done) = oneshot::channel();
        let thread = std::thread::Builder::new()
            .name("gw-folder-watch-owner".to_string())
            .spawn(move || {
                if matches!(stop.try_recv(), Err(mpsc::TryRecvError::Empty)) {
                    match prepare() {
                        Some((resources, running)) => {
                            let _ = ready_tx.send(Some(running));
                            if running {
                                // No Tokio pool slot is occupied while watchers are idle.
                                let _ = stop.recv();
                            }
                            drop(resources);
                        }
                        None => {
                            let _ = ready_tx.send(None);
                        }
                    }
                } else {
                    let _ = ready_tx.send(None);
                }
                // Readiness alone cannot prove destruction; acknowledge only afterward.
                let _ = done_tx.send(());
            })
            .map_err(|_| task_error())?;
        drop(thread);
        Ok(Self {
            stop: Some(stop_tx),
            ready: Some(ready),
            done,
            running: false,
        })
    }

    pub(super) async fn ready(&mut self) -> Result<Option<bool>, GatewayError> {
        let result = self
            .ready
            .take()
            .ok_or_else(task_error)?
            .await
            .map_err(|_| task_error())?;
        self.running = result.unwrap_or(false);
        Ok(result)
    }

    pub(super) async fn ready_with_deadline(
        &mut self,
        deadline: Duration,
    ) -> Result<Option<bool>, GatewayError> {
        tokio::time::timeout(deadline, self.ready())
            .await
            .map_err(|_| deadline_error("folder watcher native initialization timed out"))?
    }

    pub(super) fn is_running(&self) -> bool {
        self.running
    }

    pub(super) async fn shutdown(mut self) -> Result<(), GatewayError> {
        drop(self.stop.take());
        (&mut self.done).await.map_err(|_| task_error())
    }

    pub(super) async fn shutdown_with_deadline(
        self,
        deadline: Duration,
    ) -> Result<(), GatewayError> {
        match tokio::time::timeout(deadline, self.shutdown()).await {
            Ok(result) => result,
            Err(_) => {
                // The native call is not preemptible. Its detached worker still owns
                // the resources and will drop them after observing the stop signal.
                Err(deadline_error("folder watcher native shutdown timed out"))
            }
        }
    }
}

impl Drop for NativeWatchOwner {
    fn drop(&mut self) {
        // Abort cannot preempt native setup; the worker drops any late resources.
        drop(self.stop.take());
    }
}

fn task_error() -> GatewayError {
    let mut error = GatewayError::server_error("folder watcher native owner failed");
    error.code = Some("provider_credential_folder_watch_owner_failed".to_string());
    error
}

fn deadline_error(message: &str) -> GatewayError {
    let mut error = GatewayError::service_unavailable(message);
    error.code = Some("provider_credential_folder_watch_owner_deadline_exceeded".to_string());
    error
}

#[cfg(test)]
mod tests;
