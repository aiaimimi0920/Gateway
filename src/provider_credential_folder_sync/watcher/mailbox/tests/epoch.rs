use std::collections::HashSet;
use std::sync::{Arc, Barrier};

use crate::state::ProviderCredentialFolderSyncRuntime;

use super::super::channel;
use super::{change, FolderWatchSignal};

#[tokio::test]
async fn folder_sync_queued_signals_do_not_cross_coalesced_disable() {
    let runtime = ProviderCredentialFolderSyncRuntime::new(true);
    let (tx, mut rx) = channel(runtime.clone());
    tx.send(change(&["old/a.json"])).unwrap();
    tx.send(FolderWatchSignal::WatcherError("old error".into()))
        .unwrap();
    runtime.set_enabled(false);
    runtime.set_enabled(true);
    drop(tx);
    assert!(
        rx.recv().await.is_none(),
        "both obsolete signal kinds are discarded"
    );
}

#[tokio::test]
async fn folder_sync_new_epoch_keeps_exact_paths_and_latest_kind_order() {
    let runtime = ProviderCredentialFolderSyncRuntime::new(true);
    let (tx, mut rx) = channel(runtime.clone());
    tx.send(change(&["old/a.json"])).unwrap();
    runtime.set_enabled(false);
    runtime.set_enabled(true);
    tx.send(change(&["new/a.json"])).unwrap();
    tx.send(FolderWatchSignal::WatcherError("new error".into()))
        .unwrap();
    tx.send(change(&["new/b.json"])).unwrap();
    assert_eq!(rx.len(), 2);
    let first = rx.recv().await.unwrap();
    assert!(first.is_current(&runtime.snapshot()));
    assert!(
        matches!(first.signal, FolderWatchSignal::WatcherError(message) if message == "new error")
    );
    let received = rx.recv().await.unwrap();
    assert!(received.is_current(&runtime.snapshot()));
    let FolderWatchSignal::FilesystemEvent(event) = received.signal else {
        panic!("latest filesystem event clears the current epoch error");
    };
    assert_eq!(
        event.deleted_paths,
        HashSet::from(["new/a.json".into(), "new/b.json".into()])
    );
}

#[tokio::test]
async fn folder_sync_disabled_storm_is_discarded_and_closed_sends_still_fail() {
    let runtime = ProviderCredentialFolderSyncRuntime::new(true);
    let (tx, mut rx) = channel(runtime.clone());
    tx.send(change(&["old/a.json"])).unwrap();
    runtime.set_enabled(false);
    for _ in 0..10_000 {
        tx.send(change(&["disabled/a.json"])).unwrap();
        tx.send(FolderWatchSignal::WatcherError("disabled".into()))
            .unwrap();
    }
    assert_eq!(rx.len(), 0);
    runtime.set_enabled(true);
    tx.send(change(&["new/a.json"])).unwrap();
    let received = rx.recv().await.unwrap();
    let FolderWatchSignal::FilesystemEvent(event) = received.signal else {
        panic!("expected current epoch filesystem event");
    };
    assert_eq!(event.deleted_paths, HashSet::from(["new/a.json".into()]));
    drop(rx);
    runtime.set_enabled(false);
    assert!(tx.send(change(&[])).is_err(), "closed outranks disabled");
}

#[tokio::test]
async fn folder_sync_dequeued_signal_is_rejected_after_coalesced_disable() {
    let runtime = ProviderCredentialFolderSyncRuntime::new(true);
    let (tx, mut rx) = channel(runtime.clone());
    tx.send(change(&["old/a.json"])).unwrap();
    let received = rx.recv().await.unwrap();
    assert!(received.is_current(&runtime.snapshot()));
    runtime.set_enabled(false);
    runtime.set_enabled(true);
    assert!(
        !received.is_current(&runtime.snapshot()),
        "recheck after dequeue"
    );
    tx.send(change(&["new/a.json"])).unwrap();
    assert!(rx.recv().await.unwrap().is_current(&runtime.snapshot()));
}

#[tokio::test]
async fn folder_sync_parallel_epoch_boundary_preserves_all_new_deletions() {
    let runtime = ProviderCredentialFolderSyncRuntime::new(true);
    let (tx, mut rx) = channel(runtime.clone());
    let boundary = Arc::new(Barrier::new(5));
    let threads: Vec<_> = (0..4)
        .map(|sender| {
            let tx = tx.clone();
            let boundary = boundary.clone();
            std::thread::spawn(move || {
                tx.send(change(&[&format!("old/{sender}.json")])).unwrap();
                boundary.wait();
                boundary.wait();
                for index in 0..64 {
                    tx.send(change(&[&format!("new-{sender}/{index}.json")]))
                        .unwrap();
                }
            })
        })
        .collect();
    boundary.wait();
    runtime.set_enabled(false);
    runtime.set_enabled(true);
    boundary.wait();
    for thread in threads {
        thread.join().unwrap();
    }
    drop(tx);
    let received = rx.recv().await.unwrap();
    let FolderWatchSignal::FilesystemEvent(event) = received.signal else {
        panic!("expected current epoch deletions");
    };
    let expected: HashSet<_> = (0..4)
        .flat_map(|sender| (0..64).map(move |index| format!("new-{sender}/{index}.json")))
        .collect();
    assert_eq!(
        event.deleted_paths, expected,
        "no old paths or invented ancestors"
    );
    assert!(rx.recv().await.is_none());
}
