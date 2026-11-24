//! Bounded ownership for SAP requests with durable effects. Dropping the HTTP
//! waiter signals pre-send cancellation, not permission to abort settlement.
use crate::error::GatewayError;
use std::{future::Future, sync::Arc};
use tokio::sync::{oneshot, OwnedSemaphorePermit, Semaphore};

const CAPACITY: u32 = 32;

#[derive(Debug)]
pub(crate) struct StoredResponseTasks {
    closed: parking_lot::Mutex<bool>,
    permits: Arc<Semaphore>,
}

impl Default for StoredResponseTasks {
    fn default() -> Self {
        Self {
            closed: parking_lot::Mutex::new(false),
            permits: Arc::new(Semaphore::new(CAPACITY as usize)),
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct StoredResponseWaiter(Arc<parking_lot::Mutex<bool>>);

struct CancelOnDrop(Arc<parking_lot::Mutex<bool>>);

/// Acquire before transferring an existing audit owner. This permit also makes
/// drain wait for an admitted task that has not been spawned yet.
pub(crate) struct StoredResponseAdmission(OwnedSemaphorePermit);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        *self.0.lock() = true;
    }
}

impl StoredResponseWaiter {
    pub(crate) fn check(&self) -> Result<(), GatewayError> {
        if *self.0.lock() {
            return Err(GatewayError::service_unavailable("Caller disconnected.")
                .with_code("sap_aicore_response_caller_disconnected"));
        }
        Ok(())
    }
}

impl StoredResponseTasks {
    /// Linearize the send decision with drain and caller Drop. Once this returns
    /// success, cancellation cannot retract that one send or abort its writeback.
    pub(crate) fn begin_send(
        &self,
        waiter: &StoredResponseWaiter,
        begin: impl FnOnce() -> Result<(), GatewayError>,
    ) -> Result<(), GatewayError> {
        let closed = self.closed.lock();
        let cancelled = waiter.0.lock();
        if *closed || *cancelled {
            return Err(
                GatewayError::service_unavailable("Stored response send cancelled.")
                    .with_code("sap_aicore_response_not_sent"),
            );
        }
        begin()
    }

    pub(crate) fn close(&self) {
        *self.closed.lock() = true;
    }

    pub(crate) async fn join(&self) {
        self.close();
        // No future admissions can race with this barrier. Each task holds its
        // permit through durable writeback, even if its HTTP receiver is gone.
        let _all = self.permits.acquire_many(CAPACITY).await;
    }

    pub(crate) fn try_admit(&self) -> Result<StoredResponseAdmission, GatewayError> {
        let closed = self.closed.lock();
        if *closed {
            return Err(GatewayError::service_unavailable("Gateway is draining.")
                .with_code("sap_aicore_response_draining"));
        }
        let permit = self.permits.clone().try_acquire_owned().map_err(|_| {
            GatewayError::service_unavailable("Stored SAP Responses capacity exhausted.")
                .with_code("sap_aicore_response_busy")
        })?;
        Ok(StoredResponseAdmission(permit))
    }

    #[cfg(test)]
    async fn run<T, F, Fut>(&self, work: F) -> Result<T, GatewayError>
    where
        T: Send + 'static,
        F: FnOnce(StoredResponseWaiter) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T, GatewayError>> + Send + 'static,
    {
        self.try_admit()?.run(work).await
    }
}

impl StoredResponseAdmission {
    pub(crate) async fn run<T, F, Fut>(self, work: F) -> Result<T, GatewayError>
    where
        T: Send + 'static,
        F: FnOnce(StoredResponseWaiter) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T, GatewayError>> + Send + 'static,
    {
        let (tx, rx) = oneshot::channel();
        let waiter = StoredResponseWaiter::default();
        let _cancel = CancelOnDrop(waiter.0.clone());
        tokio::spawn(async move {
            let _permit = self.0;
            let _ = tx.send(work(waiter).await);
        });
        rx.await.map_err(|_| {
            GatewayError::service_unavailable("Stored SAP Responses task stopped.")
                .with_code("sap_aicore_response_task_stopped")
        })?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sap_stored_response_send_permission_orders_drain_and_caller_drop() {
        let tasks = StoredResponseTasks::default();
        let waiter = StoredResponseWaiter::default();
        tasks.begin_send(&waiter, || Ok(())).unwrap();
        drop(CancelOnDrop(waiter.0.clone()));
        assert!(tasks
            .begin_send(&waiter, || panic!("cancelled send"))
            .is_err());
        let waiter = StoredResponseWaiter::default();
        tasks.close();
        assert!(tasks
            .begin_send(&waiter, || panic!("draining send"))
            .is_err());
    }

    #[tokio::test]
    async fn sap_stored_response_tasks_disconnect_does_not_abort_writeback() {
        let owner = Arc::new(StoredResponseTasks::default());
        let (started, started_rx) = oneshot::channel();
        let (release, release_rx) = oneshot::channel();
        let (recorded, recorded_rx) = oneshot::channel();
        let caller = tokio::spawn({
            let owner = owner.clone();
            async move {
                owner
                    .run(move |waiter| async move {
                        waiter.check()?;
                        started.send(()).unwrap(); // Simulated point of no return.
                        release_rx.await.unwrap();
                        assert!(waiter.check().is_err());
                        recorded.send(()).unwrap();
                        Ok(())
                    })
                    .await
            }
        });
        started_rx.await.unwrap();
        caller.abort();
        assert!(caller.await.unwrap_err().is_cancelled());
        owner.close();
        assert!(owner.run(|_| async { Ok(()) }).await.is_err());
        let join = owner.join();
        tokio::pin!(join);
        assert!(futures::poll!(&mut join).is_pending());
        release.send(()).unwrap();
        join.await;
        recorded_rx.await.unwrap();
    }

    #[tokio::test]
    async fn sap_stored_response_tasks_rejects_capacity_without_running_work() {
        let owner = StoredResponseTasks::default();
        let all = owner.permits.acquire_many(CAPACITY).await.unwrap();
        let result = owner
            .run(|_| async { panic!("rejected task must not run") })
            .await;
        let Err::<(), _>(error) = result else {
            panic!("saturated owner accepted a task");
        };
        assert_eq!(error.code.as_deref(), Some("sap_aicore_response_busy"));
        drop(all);
        owner.run(|_| async { Ok(()) }).await.unwrap();
        owner.join().await;
    }
}
