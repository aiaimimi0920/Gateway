//! A cancelled store future may have committed; no later operation is allowed to reuse its preimage.
use super::*;
use futures::future::BoxFuture;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tokio::sync::Notify;

struct PendingStore {
    reached: Arc<Notify>,
    calls: Arc<AtomicUsize>,
}
impl RunRecorder for PendingStore {
    fn save<'a>(&'a mut self, _: &'a RunRecord) -> BoxFuture<'a, Result<(), BusinessError>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.reached.notify_one();
            std::future::pending().await
        })
    }
}
#[tokio::test]
async fn freebuff_run_journal_cancelled_store_wait_latches_unknown_and_blocks_finish() {
    let reached = Arc::new(Notify::new());
    let calls = Arc::new(AtomicUsize::new(0));
    let mut journal = RunJournal::new(
        "original-client".into(),
        Some(Box::new(PendingStore {
            reached: reached.clone(),
            calls: calls.clone(),
        })),
    );
    {
        let save = journal.save();
        tokio::pin!(save);
        tokio::select! { _ = &mut save => panic!("store remains unconfirmed"), _ = reached.notified() => {} }
    }
    assert_eq!(
        journal.save().await,
        Err(BusinessError::RunJournalUnavailable)
    );
    assert_eq!(
        journal.finishing(None).await,
        Err(BusinessError::RunJournalUnavailable)
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "an interrupted store is never replayed"
    );
}
