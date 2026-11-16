//! The original permission lease and runtime-owned task cover shutdown, Drop, and operator revocation.
use super::{authorize_send, fixture::Fixture, steward_fixture::*, steward_wire::Mock};
use crate::{
    console::refresh_grants::GrantPhase, protocol::accio_oauth::TEST_REFRESH_ENDPOINT,
    state::GatewayShutdownHandle,
};
use std::time::Duration;

#[test]
fn accio_db_steward_normal_runtime_drain_stops_maintenance_before_global_shutdown() {
    let lifecycle = crate::state::GatewayLifecycleState::default();
    assert!(!lifecycle.accio_db_refresh_stop.is_requested());
    assert!(lifecycle.begin_drain("synthetic-drain"));
    assert!(lifecycle.accio_db_refresh_stop.is_requested());
    assert_eq!(
        lifecycle.accio_db_refresh_stop.reason().as_deref(),
        Some("synthetic-drain")
    );
    assert!(!lifecycle.begin_drain("later-drain"));
    assert_eq!(
        lifecycle.accio_db_refresh_stop.reason().as_deref(),
        Some("synthetic-drain")
    );
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL; serialize tests"]
async fn accio_db_steward_requested_shutdown_admits_no_grant_or_http() {
    let f = Fixture::new().await;
    expire(&f).await;
    let shutdown = GatewayShutdownHandle::default();
    shutdown.request("synthetic-before-scan");
    let mock = Mock::new(reply()).await;
    let mut w = worker(&f, shutdown);
    assert_eq!(tick(&mock, &mut w).await.scanned_count, 0);
    let records: i64 = sqlx::query_scalar("select count(*) from gateway_provider_refresh_grants")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(records, 0);
    assert_eq!(mock.count(), 0);
    mock.stop().await;
    f.finish().await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL; serialize tests"]
async fn accio_db_steward_shutdown_joins_the_original_http_and_persists_its_result() {
    let f = Fixture::new().await;
    expire(&f).await;
    let shutdown = GatewayShutdownHandle::default();
    let mut answer = reply();
    answer.body_delay = Duration::from_millis(250);
    let mock = Mock::new(answer).await;
    let w = worker(&f, shutdown.clone());
    let endpoint = mock.endpoint.clone();
    let task = tokio::spawn(TEST_REFRESH_ENDPOINT.scope(endpoint, w.run(Duration::from_secs(60))));
    mock.wait_called().await;
    shutdown.request("synthetic-during-http");
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record(&f).await.phase(), GrantPhase::Published);
    assert_eq!(mock.count(), 1);
    let mut restarted = worker(&f, GatewayShutdownHandle::default());
    tick(&mock, &mut restarted).await;
    assert_eq!(mock.count(), 1);
    mock.stop().await;
    f.finish().await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL; serialize tests"]
async fn accio_db_steward_dropped_inflight_task_does_not_create_a_detached_retrier() {
    let f = Fixture::new().await;
    expire(&f).await;
    let mut answer = reply();
    answer.body_delay = Duration::from_secs(60);
    let mock = Mock::new(answer).await;
    let w = worker(&f, GatewayShutdownHandle::default());
    let endpoint = mock.endpoint.clone();
    let task = tokio::spawn(TEST_REFRESH_ENDPOINT.scope(endpoint, w.run(Duration::from_secs(60))));
    mock.wait_called().await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(record(&f).await.phase(), GrantPhase::Sending);
    let mut restarted = worker(&f, GatewayShutdownHandle::default());
    assert_eq!(tick(&mock, &mut restarted).await.locked_count, 1);
    assert_eq!(mock.count(), 1);
    mock.stop().await;
    f.finish().await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL; serialize tests"]
async fn accio_db_steward_revocation_after_reserve_cannot_obtain_a_send_lease() {
    let f = Fixture::new().await;
    let (snapshot, sending) = f.sending().await;
    sqlx::query("update gateway_provider_credentials set status='disabled' where id=$1")
        .bind(&f.credential)
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(authorize_send(&f.pool, &snapshot, &sending)
        .await
        .unwrap()
        .is_none());
    assert_eq!(f.record(&sending).await.phase(), GrantPhase::Sending);
    f.finish().await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL; serialize tests"]
async fn accio_db_steward_send_lease_serializes_revocation_and_retains_the_late_result() {
    let f = Fixture::new().await;
    let (snapshot, sending) = f.sending().await;
    let lease = authorize_send(&f.pool, &snapshot, &sending)
        .await
        .unwrap()
        .unwrap();
    let pool = f.pool.clone();
    let credential = f.credential.clone();
    let revoke = tokio::spawn(async move {
        sqlx::query("/*accio_send_lease_test*/ update gateway_provider_credentials set status='disabled' where id=$1")
            .bind(credential).execute(&pool).await.unwrap()
    });
    // Observe a real PostgreSQL lock wait, not merely an unpolled update future.
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let waiting: i64 = sqlx::query_scalar("select count(*) from pg_stat_activity where wait_event_type='Lock' and query like '/*accio_send_lease_test*/%'")
                .fetch_one(&f.pool).await.unwrap();
            if waiting > 0 { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.unwrap();
    assert!(!revoke.is_finished());
    drop(lease);
    let changed = tokio::time::timeout(Duration::from_secs(3), revoke)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(changed.rows_affected(), 1);
    let result = super::store_result(&f.pool, &sending, &Fixture::result(&snapshot))
        .await
        .unwrap();
    assert!(super::publish_result(&f.pool, &result)
        .await
        .unwrap()
        .is_none());
    assert_eq!(f.record(&sending).await.phase(), GrantPhase::Result);
    f.finish().await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL; serialize tests"]
async fn accio_db_steward_drain_during_snapshot_read_does_not_burn_a_new_grant() {
    let f = Fixture::new().await;
    expire(&f).await;
    let mut held = f.pool.begin().await.unwrap();
    sqlx::query("select id from gateway_provider_credentials where id=$1 for update")
        .bind(&f.credential)
        .fetch_one(&mut *held)
        .await
        .unwrap();
    let shutdown = GatewayShutdownHandle::default();
    let mock = Mock::new(reply()).await;
    let w = worker(&f, shutdown.clone());
    let task = tokio::spawn(
        TEST_REFRESH_ENDPOINT.scope(mock.endpoint.clone(), w.run(Duration::from_secs(60))),
    );
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let waiting: i64 = sqlx::query_scalar("select count(*) from pg_stat_activity where wait_event_type='Lock' and query like '%from gateway_provider_credentials c where id=$1 for update%'")
                .fetch_one(&f.pool).await.unwrap();
            if waiting > 0 { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.unwrap();
    shutdown.request("synthetic-drain-during-read");
    held.rollback().await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap();
    let records: i64 = sqlx::query_scalar("select count(*) from gateway_provider_refresh_grants")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(records, 0);
    assert_eq!(mock.count(), 0);
    mock.stop().await;
    f.finish().await;
}
