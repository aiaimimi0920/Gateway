//! Fixed runtime worker ownership. Dropping a join waiter never aborts admitted persistence.
use crate::state::GatewayShutdownHandle;
use futures::FutureExt;
use parking_lot::Mutex;
use std::{collections::BTreeMap, future::Future, pin::Pin, task::Poll};
use tokio::task::{JoinError, JoinHandle};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Worker {
    NetworkProbe,
    LocalMaintenance,
    CredentialRefill,
    CredentialProbe,
    CredentialCache,
    FolderSync,
    CredentialRefresh,
    CredentialStock,
    PoolAutomation,
    SiteHealth,
    Discovery,
    #[cfg(feature = "copilot-sdk-runtime")]
    CopilotCatalog,
}

#[derive(Debug, Default)]
struct TaskState {
    closed: bool,
    // Static keys bound both live handles and retained failure summaries to the worker variants.
    tasks: BTreeMap<Worker, JoinHandle<()>>,
    failures: BTreeMap<Worker, &'static str>,
}

#[derive(Debug, Default)]
pub(crate) struct BackgroundTasks {
    state: Mutex<TaskState>,
    join_gate: tokio::sync::Mutex<()>,
}

fn record(
    kind: Worker,
    result: Result<(), JoinError>,
    failures: &mut BTreeMap<Worker, &'static str>,
) {
    if let Err(error) = result {
        // JoinError's Display may contain a panic payload, including secrets.
        failures.insert(
            kind,
            if error.is_panic() {
                "panicked"
            } else {
                "cancelled"
            },
        );
    }
}

impl BackgroundTasks {
    pub(crate) fn spawn(
        &self,
        kind: Worker,
        stop: &GatewayShutdownHandle,
        future: impl Future<Output = ()> + Send + 'static,
    ) -> bool {
        let mut state = self.state.lock();
        if state.closed || stop.is_requested() {
            return false;
        }
        if let Some(handle) = state.tasks.get_mut(&kind) {
            // Even is_finished() handles may poll Pending when Tokio's coop budget is exhausted.
            let Some(result) = handle.now_or_never() else {
                return false;
            };
            record(kind, result, &mut state.failures);
        }
        state.tasks.insert(kind, tokio::spawn(future));
        true
    }

    pub(crate) async fn join(&self) -> anyhow::Result<()> {
        let _joiner = self.join_gate.lock().await;
        self.state.lock().closed = true;
        std::future::poll_fn(|cx| {
            let mut state = self.state.lock();
            let TaskState {
                tasks, failures, ..
            } = &mut *state;
            // Poll borrowed handles under a short lock. Cancellation leaves them in this owner.
            tasks.retain(|kind, handle| match Pin::new(handle).poll(cx) {
                Poll::Ready(result) => {
                    record(*kind, result, failures);
                    false
                }
                Poll::Pending => true,
            });
            if !tasks.is_empty() {
                return Poll::Pending;
            }
            if failures.is_empty() {
                Poll::Ready(Ok(()))
            } else {
                let summary = failures
                    .iter()
                    .map(|(kind, reason)| format!("{kind:?}: {reason}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                Poll::Ready(Err(anyhow::anyhow!(
                    "Gateway background worker shutdown failed: {summary}"
                )))
            }
        })
        .await
    }

    #[cfg(test)]
    pub(super) fn workers(&self) -> Vec<Worker> {
        self.state.lock().tasks.keys().copied().collect()
    }

    #[cfg(test)]
    pub(super) fn state_for_test_finished(&self, kind: Worker) -> bool {
        self.state
            .lock()
            .tasks
            .get(&kind)
            .is_none_or(JoinHandle::is_finished)
    }
}

#[cfg(test)]
#[path = "runtime_background_tasks_tests.rs"]
mod tests;
