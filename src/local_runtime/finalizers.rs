//! Graceful shutdown waits for detached stream audit callbacks before closing SQLite.
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tokio::sync::Notify;

#[derive(Default)]
pub(super) struct Finalizers {
    pending: AtomicUsize,
    drained: Notify,
}

pub struct FinalizerGuard(Arc<Finalizers>);
impl Drop for FinalizerGuard {
    fn drop(&mut self) {
        if self.0.pending.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.0.drained.notify_one();
        }
    }
}
impl Finalizers {
    pub(super) fn track(self: &Arc<Self>) -> FinalizerGuard {
        self.pending.fetch_add(1, Ordering::AcqRel);
        FinalizerGuard(Arc::clone(self))
    }
    pub(super) async fn wait(&self) {
        while self.pending.load(Ordering::Acquire) != 0 {
            self.drained.notified().await;
        }
    }
}
