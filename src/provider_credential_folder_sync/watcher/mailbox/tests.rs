use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::super::super::path_budget::WATCHER_DELETED_PATH_BYTES;
use super::super::{FolderWatchEvent, FolderWatchSignal};
use fixture::channel;

mod epoch;
mod fixture;

fn change(paths: &[&str]) -> FolderWatchSignal {
    FolderWatchSignal::FilesystemEvent(FolderWatchEvent {
        deleted_paths: paths.iter().map(|path| (*path).to_owned()).collect(),
    })
}

#[tokio::test]
async fn folder_sync_duplicate_change_storm_has_one_pending_signal() {
    let (tx, mut rx) = channel();
    for _ in 0..10_000 {
        tx.send(change(&["codex/removed.json"])).unwrap();
    }
    assert_eq!(rx.len(), 1, "duplicate changes must not grow queue nodes");
    let Some(FolderWatchSignal::FilesystemEvent(event)) = rx.recv().await else {
        panic!("expected filesystem event");
    };
    assert_eq!(
        event.deleted_paths,
        HashSet::from(["codex/removed.json".to_owned()])
    );
}

#[tokio::test]
async fn folder_sync_error_storm_retains_latest_diagnostic() {
    let (tx, mut rx) = channel();
    for index in 0..1000 {
        tx.send(FolderWatchSignal::WatcherError(format!("error {index}")))
            .unwrap();
    }
    assert_eq!(rx.len(), 1, "error diagnostics must be coalesced");
    let Some(FolderWatchSignal::WatcherError(message)) = rx.recv().await else {
        panic!("expected latest error");
    };
    assert_eq!(message, "error 999");
}

#[tokio::test]
async fn folder_sync_latest_signal_order_preserves_final_watch_status() {
    let (tx, mut rx) = channel();
    tx.send(change(&["codex/a.json"])).unwrap();
    tx.send(FolderWatchSignal::WatcherError("backend".to_owned()))
        .unwrap();
    tx.send(change(&["codex/b.json"])).unwrap();
    assert!(matches!(
        rx.recv().await,
        Some(FolderWatchSignal::WatcherError(_))
    ));
    let Some(FolderWatchSignal::FilesystemEvent(event)) = rx.recv().await else {
        panic!("last change must clear the earlier backend error");
    };
    assert_eq!(
        event.deleted_paths,
        HashSet::from(["codex/a.json".to_owned(), "codex/b.json".to_owned()])
    );
}

#[tokio::test]
async fn folder_sync_mixed_storm_has_at_most_two_signal_nodes() {
    let (tx, rx) = channel();
    for _ in 0..1000 {
        tx.send(change(&[])).unwrap();
        tx.send(FolderWatchSignal::WatcherError("backend".to_owned()))
            .unwrap();
    }
    assert_eq!(rx.len(), 2, "one pending node per signal kind");
}

#[tokio::test]
async fn folder_sync_parallel_senders_preserve_every_distinct_delete() {
    let (tx, mut rx) = channel();
    let threads: Vec<_> = (0..4)
        .map(|sender| {
            let tx = tx.clone();
            std::thread::spawn(move || {
                for index in 0..100 {
                    tx.send(change(&[&format!("provider-{sender}/{index}.json")]))
                        .unwrap();
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    drop(tx);
    let mut deleted = HashSet::new();
    while let Some(signal) = rx.recv().await {
        let FolderWatchSignal::FilesystemEvent(event) = signal else {
            panic!("unexpected signal");
        };
        deleted.extend(event.deleted_paths);
    }
    let expected: HashSet<_> = (0..4)
        .flat_map(|sender| (0..100).map(move |index| format!("provider-{sender}/{index}.json")))
        .collect();
    assert_eq!(
        deleted, expected,
        "no lost paths or invented ancestor deletion"
    );
}

#[tokio::test]
async fn folder_sync_receiver_drop_rejects_callback_sends() {
    let (tx, rx) = channel();
    tx.send(change(&["provider/a.json"])).unwrap();
    drop(rx);
    assert!(tx.send(change(&["provider/b.json"])).is_err());
}

#[tokio::test]
async fn folder_sync_lazy_callback_skips_allocation_when_disabled_or_closed() {
    let runtime = crate::state::ProviderCredentialFolderSyncRuntime::new(true);
    let (tx, rx) = super::channel(runtime.clone());
    let built = Arc::new(AtomicBool::new(false));
    let callback_built = built.clone();
    runtime.set_enabled(false);
    tx.send_lazy(|| {
        callback_built.store(true, Ordering::SeqCst);
        change(&["disabled/a.json"])
    });
    assert!(!built.load(Ordering::SeqCst));

    drop(rx);
    let callback_built = built.clone();
    tx.send_lazy(|| {
        callback_built.store(true, Ordering::SeqCst);
        change(&["closed/a.json"])
    });
    assert!(!built.load(Ordering::SeqCst));
}

#[tokio::test]
async fn folder_sync_last_sender_drop_wakes_receiver() {
    let (tx, mut rx) = channel();
    let other = tx.clone();
    drop(tx);
    let waiter = tokio::spawn(async move { rx.recv().await });
    tokio::task::yield_now().await;
    drop(other);
    assert!(tokio::time::timeout(Duration::from_secs(1), waiter)
        .await
        .unwrap()
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn folder_sync_cancelled_receive_keeps_next_notification() {
    let (tx, mut rx) = channel();
    assert!(tokio::time::timeout(Duration::from_millis(5), rx.recv())
        .await
        .is_err());
    tx.send(change(&[])).unwrap();
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(1), rx.recv())
            .await
            .unwrap(),
        Some(FolderWatchSignal::FilesystemEvent(_))
    ));
}

#[tokio::test]
async fn folder_sync_mailbox_overflow_retains_no_partial_deletion_intent() {
    let (tx, mut rx) = channel();
    tx.send(change(&["provider/kept.json"])).unwrap();
    let oversized = "x".repeat(WATCHER_DELETED_PATH_BYTES);
    tx.send(change(&[oversized.as_str()])).unwrap();

    let Some(FolderWatchSignal::FilesystemEvent(event)) = rx.recv().await else {
        panic!("expected bounded filesystem event");
    };
    assert!(event.deleted_paths.overflowed());
    assert!(event.deleted_paths.is_empty());
}
