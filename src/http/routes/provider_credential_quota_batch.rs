use std::future::Future;

use futures::{stream, StreamExt};

pub(super) const MAX_CONCURRENT_QUOTA_READS: usize = 8;

pub(super) async fn collect_bounded_ordered<F>(jobs: impl IntoIterator<Item = F>) -> Vec<F::Output>
where
    F: Future,
{
    stream::iter(jobs)
        .buffered(MAX_CONCURRENT_QUOTA_READS)
        .collect()
        .await
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use super::{collect_bounded_ordered, MAX_CONCURRENT_QUOTA_READS};

    #[tokio::test]
    async fn quota_reads_are_concurrent_bounded_and_ordered() {
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let jobs = (0..MAX_CONCURRENT_QUOTA_READS * 3).map(|index| {
            let active = Arc::clone(&active);
            let peak = Arc::clone(&peak);
            async move {
                let running = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(running, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(5)).await;
                active.fetch_sub(1, Ordering::SeqCst);
                index
            }
        });

        let values = collect_bounded_ordered(jobs).await;

        expect_sequence(&values);
        assert_eq!(peak.load(Ordering::SeqCst), MAX_CONCURRENT_QUOTA_READS);
    }

    fn expect_sequence(values: &[usize]) {
        assert_eq!(values, (0..values.len()).collect::<Vec<_>>());
    }
}
