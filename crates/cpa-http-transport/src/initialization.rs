//! One owned blocking initialization job; cancelled callers never launch duplicate jobs.
use crate::{Error, Result};
use futures::future::{BoxFuture, FutureExt, Shared};
use std::sync::{Arc, OnceLock};
use tokio::time::Instant;

type Job<T> = Shared<BoxFuture<'static, Result<Arc<T>>>>;

/// Instance-startup configuration is cached, including failures, until process restart.
pub struct Initialization<T> {
    job: OnceLock<Job<T>>,
}

impl<T: Send + Sync + 'static> Initialization<T> {
    pub const fn new() -> Self {
        Self {
            job: OnceLock::new(),
        }
    }

    /// Wait for the single bounded job within the caller's end-to-end deadline.
    pub async fn get(
        &self,
        deadline: Instant,
        initialize: impl FnOnce() -> Result<T> + Send + 'static,
    ) -> Result<Arc<T>> {
        let job = self.job.get_or_init(|| {
            let worker = tokio::task::spawn_blocking(move || initialize().map(Arc::new));
            async move { worker.await.map_err(|_| Error::Trust)? }
                .boxed()
                .shared()
        });
        tokio::time::timeout_at(deadline, job.clone())
            .await
            .map_err(|_| Error::Timeout)?
    }
}

impl<T: Send + Sync + 'static> Default for Initialization<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        time::Duration,
    };

    #[tokio::test]
    async fn timed_out_waiter_does_not_duplicate_or_cancel_initialization() {
        let init = Initialization::new();
        let count = Arc::new(AtomicUsize::new(0));
        let worker_count = count.clone();
        let first = init
            .get(Instant::now() + Duration::from_millis(10), move || {
                worker_count.fetch_add(1, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(80));
                Ok(42)
            })
            .await;
        assert_eq!(first.unwrap_err(), Error::Timeout);
        let value = init
            .get(Instant::now() + Duration::from_secs(2), || {
                panic!("duplicate job")
            })
            .await
            .unwrap();
        assert_eq!(*value, 42);
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn configuration_failure_is_cached_without_retry_storm() {
        let init = Initialization::<u8>::new();
        assert_eq!(
            init.get(Instant::now() + Duration::from_secs(1), || Err(
                Error::Trust
            ))
            .await
            .unwrap_err(),
            Error::Trust
        );
        assert_eq!(
            init.get(Instant::now() + Duration::from_secs(1), || panic!(
                "duplicate job"
            ))
            .await
            .unwrap_err(),
            Error::Trust
        );
    }
}
