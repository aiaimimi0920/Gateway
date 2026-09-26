use std::collections::HashSet;
use std::task::Poll;
use std::time::Duration;

use crate::state::ProviderCredentialFolderSyncRuntime;

use super::super::super::path_budget::{BoundedPathSet, WATCHER_DELETED_PATH_BYTES};
use super::PendingWatchWork;

#[tokio::test]
async fn folder_sync_elapsed_timer_rechecks_coalesced_disable_before_control() {
    let runtime = ProviderCredentialFolderSyncRuntime::new(true);
    let mut control = runtime.subscribe();
    let mut work = PendingWatchWork::new(runtime.snapshot());
    work.deleted_paths.insert("old/a.json".into());
    work.timer.schedule(Duration::ZERO);
    tokio::time::timeout(Duration::from_secs(1), work.timer.ready())
        .await
        .expect("actual timer readiness selected before control");
    runtime.set_enabled(false);
    runtime.set_enabled(true);
    let current = runtime.snapshot();
    assert!(
        current.enabled(),
        "boolean notifications have coalesced to enabled"
    );
    assert!(
        !work.reconcile(&current),
        "obsolete selected timer cannot admit a run"
    );
    assert!(work.deleted_paths.is_empty());
    assert!(matches!(
        futures::poll!(Box::pin(work.timer.ready())),
        Poll::Pending
    ));
    control.changed().await.unwrap();
    assert!(work.reconcile(&control.borrow_and_update().clone()));
    assert!(work.deleted_paths.is_empty());
}

#[tokio::test]
async fn folder_sync_new_paths_survive_late_control_reconciliation() {
    let runtime = ProviderCredentialFolderSyncRuntime::new(true);
    let mut control = runtime.subscribe();
    let mut work = PendingWatchWork::new(runtime.snapshot());
    work.deleted_paths.insert("old/a.json".into());
    work.timer.schedule(Duration::ZERO);
    runtime.set_enabled(false);
    runtime.set_enabled(true);
    // A new-epoch event wins select before the control notification.
    work.reconcile(&runtime.snapshot());
    let expected: HashSet<_> = (0..100).map(|index| format!("new/{index}.json")).collect();
    work.deleted_paths.extend(expected.iter().cloned());
    work.timer.schedule(Duration::from_secs(3600));
    control.changed().await.unwrap();
    assert!(work.reconcile(&control.borrow_and_update().clone()));
    assert_eq!(work.deleted_paths, expected);
    assert!(matches!(
        futures::poll!(Box::pin(work.timer.ready())),
        Poll::Pending
    ));
}

#[tokio::test]
async fn folder_sync_same_enabled_value_retains_paths_and_first_deadline() {
    let runtime = ProviderCredentialFolderSyncRuntime::new(true);
    let control = runtime.subscribe();
    let before = runtime.snapshot();
    let mut work = PendingWatchWork::new(before.clone());
    work.deleted_paths.insert("same/a.json".into());
    work.timer.schedule(Duration::ZERO);
    assert!(!runtime.set_enabled(true));
    assert!(!control.has_changed().unwrap());
    assert!(before.same_epoch(&runtime.snapshot()));
    assert!(work.reconcile(&runtime.snapshot()));
    work.timer.schedule(Duration::from_secs(3600));
    tokio::time::timeout(Duration::from_secs(1), work.timer.ready())
        .await
        .expect("same-value reconciliation retains the first deadline");
    assert_eq!(work.deleted_paths, HashSet::from(["same/a.json".into()]));
}

#[tokio::test]
async fn folder_sync_observed_disable_clears_work_and_repeated_disable_is_noop() {
    let runtime = ProviderCredentialFolderSyncRuntime::new(true);
    let mut work = PendingWatchWork::new(runtime.snapshot());
    work.deleted_paths.insert("old/a.json".into());
    work.timer.schedule(Duration::from_millis(10));
    assert!(runtime.set_enabled(false));
    let disabled = runtime.snapshot();
    assert!(!work.reconcile(&disabled));
    assert!(work.deleted_paths.is_empty());
    assert!(!runtime.set_enabled(false));
    assert!(disabled.same_epoch(&runtime.snapshot()));
    assert!(runtime.set_enabled(true));
    assert!(work.reconcile(&runtime.snapshot()));
    assert!(matches!(
        futures::poll!(Box::pin(work.timer.ready())),
        Poll::Pending
    ));
}

#[test]
fn folder_sync_overflow_retains_no_partial_intent_and_epoch_reset_recovers() {
    let runtime = ProviderCredentialFolderSyncRuntime::new(true);
    let mut work = PendingWatchWork::new(runtime.snapshot());
    let mut incoming = BoundedPathSet::default();
    incoming.insert("provider/kept.json".into());
    incoming.insert("x".repeat(WATCHER_DELETED_PATH_BYTES));

    work.merge_deleted_paths(incoming);
    assert!(work.deletion_intent_overflowed());
    assert!(work.deleted_paths.is_empty());
    work.clear_deletion_intent();
    assert!(!work.deletion_intent_overflowed());
    work.deleted_paths.insert("provider/recovered.json".into());
    assert_eq!(
        work.deleted_paths,
        HashSet::from(["provider/recovered.json".into()])
    );

    runtime.set_enabled(false);
    assert!(!work.reconcile(&runtime.snapshot()));
    assert!(!work.deletion_intent_overflowed());
    assert!(work.deleted_paths.is_empty());
}
