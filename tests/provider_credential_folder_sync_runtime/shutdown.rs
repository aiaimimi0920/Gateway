use std::task::Poll;

use crate::fixture::Fixture;
use neuro_gateway::provider_credential_folder_sync::start_folder_sync_task;

#[tokio::test]
#[ignore = "requires a dedicated guarded Redis instance; run with --test-threads=1"]
async fn shutdown_stops_disabled_folder_task() {
    let mut fixture = Fixture::new(false, false, true).await;
    fixture.start();
    fixture.wait_disabled_status().await;
    fixture.finish_gracefully().await;
}

#[tokio::test]
#[ignore = "requires a dedicated guarded Redis instance; run with --test-threads=1"]
async fn shutdown_stops_initialized_folder_watchers() {
    let mut fixture = Fixture::new(true, true, true).await;
    fixture.start();
    fixture.wait_root().await;
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while !fixture
            .status()
            .await
            .is_some_and(|status| status.watch_running)
        {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    fixture.finish_gracefully().await;
}

#[tokio::test]
#[ignore = "requires a dedicated guarded Redis instance; run with --test-threads=1"]
async fn shutdown_does_not_wait_for_disabled_status_redis_io() {
    let mut fixture = Fixture::new(false, false, true).await;
    fixture.state.redis_pool.resize(1);
    let connection = fixture.state.redis_pool.get().await.unwrap();
    let mut startup = Box::pin(start_folder_sync_task(fixture.state.clone()));
    assert!(matches!(futures::poll!(startup.as_mut()), Poll::Pending));
    fixture.start_future(startup);
    assert!(!fixture.root.exists());
    fixture.finish_gracefully().await;
    drop(connection);
}

#[tokio::test]
#[ignore = "requires a dedicated guarded Redis instance; run with --test-threads=1"]
async fn shutdown_drains_in_flight_status_write_before_stopping_task() {
    let mut fixture = Fixture::new(true, false, true).await;
    fixture.state.redis_pool.resize(1);
    let connection = fixture.state.redis_pool.get().await.unwrap();
    // Prevent a second connection from letting the initial run finish before
    // the shutdown assertion observes the in-flight status operation.
    fixture.state.redis_pool.resize(0);
    fixture.start();
    fixture.wait_root().await;
    let mut task = fixture.take_task();

    fixture
        .state
        .shutdown
        .request("folder watcher shutdown drain regression");
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut task)
            .await
            .is_err()
    );

    drop(connection);
    fixture.state.redis_pool.resize(1);
    tokio::time::timeout(std::time::Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap();
}
