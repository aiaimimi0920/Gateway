use crate::provider_credential_folder_sync::FolderSyncExplicitDeleteEventView;

use super::{
    update_folder_sync_enabled, update_folder_sync_status,
    ProviderCredentialFolderSyncStatusView as Status,
};

mod fixture;
use fixture::Fixture;

#[tokio::test]
#[ignore = "requires isolated guarded Redis and --test-threads=1"]
async fn folder_sync_status_retry_preserves_concurrent_run_and_delete_audit() {
    let mut fixture = Fixture::new().await;
    fixture.replace(&Status::default());
    let latest = Status {
        imported_count: 7,
        last_run_at: Some("concurrent run".into()),
        recent_explicit_delete_events: vec![FolderSyncExplicitDeleteEventView {
            event_id: "concurrent deletion".into(),
            deleted_paths: vec!["provider/deleted.json".into()],
            ..Default::default()
        }],
        ..Default::default()
    };
    let pool = fixture.pool.clone();
    let mut attempts = 0;
    let result = update_folder_sync_status(&pool, |status| {
        if attempts == 0 {
            fixture.replace(&latest);
        }
        attempts += 1;
        status.watch_running = true;
    })
    .await
    .unwrap();
    assert_eq!(
        result.imported_count, 7,
        "watch update erased the concurrent run"
    );
    assert_eq!(result.last_run_at, latest.last_run_at);
    assert_eq!(
        result.recent_explicit_delete_events,
        latest.recent_explicit_delete_events
    );
    assert!(result.watch_running);
    assert_eq!(
        attempts, 2,
        "conflict must reload before reapplying owned fields"
    );
    assert_eq!(fixture.status().await.imported_count, 7);
}

#[tokio::test]
#[ignore = "requires isolated guarded Redis and --test-threads=1"]
async fn folder_sync_status_missing_snapshot_preserves_concurrent_creation() {
    let mut fixture = Fixture::new().await;
    let pool = fixture.pool.clone();
    let mut first = true;
    let result = update_folder_sync_status(&pool, |status| {
        if first {
            fixture.replace(&Status {
                last_watch_event_at: Some("concurrent event".into()),
                interval_seconds: Some(u64::MAX),
                ..Default::default()
            });
            first = false;
        }
        status.last_error = Some("run failed".into());
    })
    .await
    .unwrap();
    assert_eq!(
        result.last_watch_event_at.as_deref(),
        Some("concurrent event")
    );
    assert_eq!(result.interval_seconds, Some(u64::MAX));
    assert_eq!(result.last_error.as_deref(), Some("run failed"));
}

#[tokio::test]
#[ignore = "requires isolated guarded Redis and --test-threads=1"]
async fn folder_sync_status_contention_is_bounded_without_blind_overwrite() {
    let mut fixture = Fixture::new().await;
    let pool = fixture.pool.clone();
    let mut attempts = 0;
    let error = update_folder_sync_status(&pool, |status| {
        attempts += 1;
        fixture.replace(&Status {
            imported_count: attempts,
            ..Default::default()
        });
        status.watch_running = true;
    })
    .await
    .expect_err("every stale commit must be rejected");
    assert_eq!(error.http_status, Some(409));
    assert!((2..=8).contains(&attempts), "retry work must be bounded");
    let stored = fixture.status().await;
    assert_eq!(stored.imported_count, attempts);
    assert!(
        !stored.watch_running,
        "failed update overwrote the winning writer"
    );
}

#[tokio::test]
#[ignore = "requires isolated guarded Redis and --test-threads=1"]
async fn folder_sync_status_preserves_exact_large_integer_fields() {
    let mut fixture = Fixture::new().await;
    fixture.replace(&Status {
        interval_seconds: Some(u64::MAX),
        imported_count: usize::MAX,
        ..Default::default()
    });
    update_folder_sync_status(&fixture.pool, |status| status.watch_running = true)
        .await
        .unwrap();
    let stored = fixture.status().await;
    assert_eq!(stored.interval_seconds, Some(u64::MAX));
    assert_eq!(stored.imported_count, usize::MAX);
    assert!(stored.watch_running);
}

#[tokio::test]
#[ignore = "requires isolated guarded Redis and --test-threads=1"]
async fn folder_sync_status_malformed_json_is_not_overwritten() {
    let mut fixture = Fixture::new().await;
    fixture.replace_raw("{broken");
    let mut invoked = false;
    let error = update_folder_sync_status(&fixture.pool, |_| invoked = true)
        .await
        .unwrap_err();
    assert!(error.message.starts_with("decode folder sync status:"));
    assert!(!invoked);
    assert_eq!(fixture.raw().await, "{broken");
}

#[tokio::test]
#[ignore = "requires isolated guarded Redis and --test-threads=1"]
async fn folder_sync_status_missing_key_initializes_and_returns_committed_value() {
    let fixture = Fixture::new().await;
    let result = update_folder_sync_status(&fixture.pool, |status| {
        status.watch_running = true;
        status.last_watch_error = Some("watch diagnostic".into());
    })
    .await
    .unwrap();
    assert_eq!(serde_json::to_string(&result).unwrap(), fixture.raw().await);
    assert!(result.watch_running);
    assert_eq!(result.last_watch_error.as_deref(), Some("watch diagnostic"));
}

#[tokio::test]
#[ignore = "requires isolated guarded Redis and --test-threads=1"]
async fn folder_sync_enabled_update_commits_status_and_override_as_one_snapshot() {
    let mut fixture = Fixture::new().await;
    fixture.replace(&Status {
        last_watch_event_at: Some("before race".into()),
        ..Default::default()
    });
    fixture.replace_enabled(false);
    let pool = fixture.pool.clone();
    let mut first = true;
    let result = update_folder_sync_enabled(&pool, true, |status| {
        if first {
            fixture.replace(&Status {
                last_watch_event_at: Some("concurrent observer".into()),
                ..Default::default()
            });
            fixture.replace_enabled(false);
            first = false;
        }
        status.enabled = true;
    })
    .await
    .unwrap();

    assert_eq!(
        result.last_watch_event_at.as_deref(),
        Some("concurrent observer")
    );
    assert!(result.enabled);
    assert_eq!(fixture.enabled_raw().await.as_deref(), Some("true"));
    assert_eq!(
        fixture.status().await.last_watch_event_at.as_deref(),
        Some("concurrent observer")
    );
}

#[tokio::test]
#[ignore = "requires isolated guarded Redis and --test-threads=1"]
async fn folder_sync_legacy_snapshot_is_read_and_promoted_to_tagged_keys() {
    let mut fixture = Fixture::new().await;
    fixture.replace_legacy(&Status {
        imported_count: 11,
        last_run_at: Some("legacy run".into()),
        ..Default::default()
    });
    fixture.replace_legacy_enabled(true);

    let result = update_folder_sync_enabled(&fixture.pool, false, |status| {
        status.watch_running = true;
    })
    .await
    .unwrap();

    assert_eq!(result.imported_count, 11);
    assert_eq!(result.last_run_at.as_deref(), Some("legacy run"));
    assert!(result.watch_running);
    assert!(!result.enabled);
    assert_eq!(fixture.status().await.imported_count, 11);
    assert_eq!(fixture.enabled_raw().await.as_deref(), Some("false"));
}
