use std::task::Poll;
use std::time::Duration;

use neuro_gateway::error::GatewayError;
use neuro_gateway::provider_credential_folder_sync::{run_folder_sync_once, FolderSyncDirection};
use neuro_gateway::redis::keys;
use redis::AsyncCommands;

use crate::fixture::Fixture;

fn assert_busy(error: GatewayError) {
    assert_eq!(error.http_status, Some(409));
    assert_eq!(
        error.code.as_deref(),
        Some("provider_credential_folder_sync_run_busy")
    );
}

async fn wait_for_finished_run(fixture: &Fixture) -> GatewayError {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let error = run_folder_sync_once(&fixture.state, FolderSyncDirection::Both)
                .await
                .unwrap_err();
            if error.code.as_deref() != Some("provider_credential_folder_sync_run_busy") {
                return error;
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("admitted run finishes before fixture cleanup")
}

#[tokio::test]
#[ignore = "requires a dedicated guarded Redis instance; run with --test-threads=1"]
async fn folder_sync_overlapping_run_fails_before_waiting_for_redis() {
    let fixture = Fixture::new(false, false, true).await;
    fixture.state.redis_pool.resize(1);
    let connection = fixture.state.redis_pool.get().await.unwrap();
    let mut first = Box::pin(run_folder_sync_once(
        &fixture.state,
        FolderSyncDirection::Import,
    ));
    assert!(matches!(futures::poll!(first.as_mut()), Poll::Pending));
    let second = tokio::time::timeout(
        Duration::from_millis(200),
        run_folder_sync_once(&fixture.state, FolderSyncDirection::Export),
    )
    .await;
    assert!(!fixture.root.exists());
    drop(connection);
    assert_eq!(first.await.unwrap_err().http_status, Some(503));
    assert_busy(second.expect("busy must not wait for Redis").unwrap_err());
}

#[tokio::test]
#[ignore = "requires a dedicated guarded Redis instance; run with --test-threads=1"]
async fn folder_sync_cancelled_run_keeps_admission_until_its_work_finishes() {
    let fixture = Fixture::new(true, false, true).await;
    fixture.state.redis_pool.resize(1);
    let connection = fixture.state.redis_pool.get().await.unwrap();
    let mut first = Box::pin(run_folder_sync_once(
        &fixture.state,
        FolderSyncDirection::Import,
    ));
    assert!(matches!(futures::poll!(first.as_mut()), Poll::Pending));
    drop(first);
    fixture
        .state
        .provider_credential_folder_sync
        .set_enabled(false);
    let second = tokio::time::timeout(
        Duration::from_millis(200),
        run_folder_sync_once(&fixture.state, FolderSyncDirection::Both),
    )
    .await;
    drop(connection);
    fixture.wait_root().await;
    assert_busy(
        second
            .expect("caller cancellation must retain admission")
            .unwrap_err(),
    );
    assert_eq!(wait_for_finished_run(&fixture).await.http_status, Some(503));
}

#[tokio::test]
#[ignore = "requires a dedicated guarded Redis instance; run with --test-threads=1"]
async fn folder_sync_cancelled_caller_does_not_cancel_admitted_filesystem_effect() {
    let fixture = Fixture::new(false, false, true).await;
    fixture.state.redis_pool.resize(1);
    let connection = fixture.state.redis_pool.get().await.unwrap();
    let mut request = Box::pin(run_folder_sync_once(
        &fixture.state,
        FolderSyncDirection::Export,
    ));
    assert!(matches!(futures::poll!(request.as_mut()), Poll::Pending));
    drop(request);
    assert!(!fixture.root.exists());
    drop(connection);
    fixture.wait_root().await;
    assert_eq!(wait_for_finished_run(&fixture).await.http_status, Some(503));
    assert_eq!(std::sync::Arc::strong_count(&fixture.state), 1);
}

#[tokio::test]
#[ignore = "requires a dedicated guarded Redis instance; run with --test-threads=1"]
async fn folder_sync_status_error_releases_run_and_preserves_error_order() {
    let fixture = Fixture::new(false, false, true).await;
    let key = keys::provider_credential_folder_sync_status_key();
    let mut connection = fixture.state.redis_pool.get().await.unwrap();
    let _: () = connection.set(&key, "{broken").await.unwrap();
    drop(connection);
    for direction in [FolderSyncDirection::Import, FolderSyncDirection::Export] {
        let error = run_folder_sync_once(&fixture.state, direction)
            .await
            .unwrap_err();
        assert!(error.message.starts_with("decode folder sync status:"));
        assert!(
            !fixture.root.exists(),
            "status errors precede filesystem effects"
        );
    }
    let mut connection = fixture.state.redis_pool.get().await.unwrap();
    let _: () = connection.set(&key, "{}").await.unwrap();
    drop(connection);
    let error = run_folder_sync_once(&fixture.state, FolderSyncDirection::Both)
        .await
        .unwrap_err();
    assert_eq!(error.http_status, Some(503));
    assert!(error.message.starts_with("PostgreSQL"));
    assert!(fixture.root.is_dir());
}

#[tokio::test]
#[ignore = "requires a dedicated guarded Redis instance; run with --test-threads=1"]
async fn folder_sync_manual_run_remains_available_while_auto_mode_disabled() {
    let fixture = Fixture::new(false, false, true).await;
    for direction in [
        FolderSyncDirection::Import,
        FolderSyncDirection::Export,
        FolderSyncDirection::Both,
    ] {
        let error = run_folder_sync_once(&fixture.state, direction)
            .await
            .unwrap_err();
        assert_eq!(error.http_status, Some(503));
        assert!(error.message.starts_with("PostgreSQL"));
        assert!(
            fixture.root.is_dir(),
            "manual run passes the auto-mode toggle"
        );
    }
}
