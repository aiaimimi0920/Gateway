use std::future::pending;
use std::pin::Pin;
use std::time::Duration;

use tokio::time::Sleep;

#[derive(Default)]
pub(super) struct DebounceTimer {
    sleep: Option<Pin<Box<Sleep>>>,
}

impl DebounceTimer {
    pub(super) fn schedule(&mut self, delay: Duration) {
        // Coalesce from the first event; later events must not postpone the run.
        if self.sleep.is_none() {
            self.sleep = Some(Box::pin(tokio::time::sleep(delay)));
        }
    }

    pub(super) fn cancel(&mut self) {
        // Drop even elapsed readiness; no queued timer signal can cross disable.
        self.sleep = None;
    }

    pub(super) async fn ready(&mut self) {
        // Cancelling this borrow in select! preserves the owned sleep/deadline.
        match self.sleep.as_mut() {
            Some(sleep) => sleep.as_mut().await,
            None => pending::<()>().await,
        }
        self.sleep = None;
    }
}

#[cfg(test)]
mod tests;
