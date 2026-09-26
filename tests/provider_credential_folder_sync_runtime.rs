#[path = "pipeline_send_runtime/config.rs"]
mod config;
#[path = "provider_credential_folder_sync_runtime/enable.rs"]
mod enable;
#[path = "provider_credential_folder_sync_runtime/fixture.rs"]
mod fixture;
#[path = "provider_credential_folder_sync_runtime/run.rs"]
mod run;
#[path = "provider_credential_folder_sync_runtime/shutdown.rs"]
mod shutdown;
mod support;

use std::task::Poll;
use std::time::Duration;

use fixture::Fixture;
use neuro_gateway::provider_credential_folder_sync::{set_runtime_enabled, start_folder_sync_task};

#[tokio::test]
#[ignore = "requires a dedicated guarded Redis instance; run with --test-threads=1"]
async fn folder_sync_disabled_start_enables_filesystem_watch_without_restart() {
    let mut fixture = Fixture::new(false, true, true).await;
    fixture.start();
    fixture.wait_disabled_status().await;
    set_runtime_enabled(&fixture.state, true).await.unwrap();
    fixture.wait_root().await;
    tokio::time::timeout(Duration::from_secs(10), async {
        let file = fixture.root.join("startup.json");
        let mut revision = 0;
        loop {
            std::fs::write(&file, format!("{{\"revision\":{revision}}}")).unwrap();
            revision += 1;
            if let Some(status) = fixture.status().await {
                if status.last_watch_event_at.is_some() {
                    assert!(status.enabled && status.watch_running);
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
    })
    .await
    .expect("real filesystem events reach the re-enabled runtime");
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires a dedicated guarded Redis instance; run with --test-threads=1"]
async fn folder_sync_disabled_start_enables_runtime_without_watchers() {
    let mut fixture = Fixture::new(false, false, true).await;
    fixture.start();
    fixture.wait_disabled_status().await;
    set_runtime_enabled(&fixture.state, true).await.unwrap();
    fixture.wait_root().await;
    fixture.wait_enabled_status_without_watchers().await;
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires a dedicated guarded Redis instance; run with --test-threads=1"]
async fn folder_sync_enable_during_disabled_status_write_is_retained() {
    let mut fixture = Fixture::new(false, false, true).await;
    fixture.state.redis_pool.resize(1);
    let connection = fixture.state.redis_pool.get().await.unwrap();
    let mut startup = Box::pin(start_folder_sync_task(fixture.state.clone()));
    // Hold the only connection so the first poll pauses inside the disabled write.
    assert!(matches!(futures::poll!(startup.as_mut()), Poll::Pending));
    fixture
        .state
        .provider_credential_folder_sync
        .set_enabled(true);
    drop(connection);
    fixture.start_future(startup);
    fixture.wait_root().await;
    fixture.wait_enabled_status_without_watchers().await;
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires a dedicated guarded Redis instance; run with --test-threads=1"]
async fn folder_sync_disabled_wait_cancellation_releases_task_state() {
    let mut fixture = Fixture::new(false, false, true).await;
    fixture.start();
    fixture.wait_disabled_status().await;
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires a dedicated guarded Redis instance; run with --test-threads=1"]
async fn folder_sync_enabled_start_keeps_immediate_initialization() {
    let mut fixture = Fixture::new(true, false, true).await;
    fixture.start();
    fixture.wait_root().await;
    fixture.wait_enabled_status_without_watchers().await;
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires a dedicated guarded Redis instance; run with --test-threads=1"]
async fn folder_sync_unconfigured_root_still_exits_without_initialization() {
    let fixture = Fixture::new(false, true, false).await;
    tokio::time::timeout(
        Duration::from_secs(3),
        start_folder_sync_task(fixture.state.clone()),
    )
    .await
    .unwrap();
    assert!(!fixture.root.exists());
    assert!(fixture.status().await.is_none());
    assert!(set_runtime_enabled(&fixture.state, true).await.is_err());
}
