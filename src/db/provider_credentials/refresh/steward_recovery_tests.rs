//! Real storage failures retain the original result/consumption facts and never start another HTTP.
use super::{fixture::Fixture, steward_fixture::*, steward_wire::Mock};
use crate::{console::refresh_grants::GrantPhase, state::GatewayShutdownHandle};

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL; serialize tests"]
async fn accio_db_steward_result_write_failure_retries_only_the_retained_material() {
    let f = Fixture::new().await;
    expire(&f).await;
    reject(&f, "result").await;
    let mock = Mock::new(reply()).await;
    let mut w = worker(&f, GatewayShutdownHandle::default());
    assert_eq!(tick(&mock, &mut w).await.failed_count, 1);
    assert!(w.has_received());
    assert_eq!(record(&f).await.phase(), GrantPhase::Sending);
    assert_eq!(mock.count(), 1);
    assert!(w.tick().await.is_err());
    assert!(w.has_received());
    assert_eq!(mock.count(), 1);
    allow(&f).await;
    assert_eq!(tick(&mock, &mut w).await.refreshed_count, 1);
    assert!(!w.has_received());
    assert_eq!(record(&f).await.phase(), GrantPhase::Published);
    assert_eq!(mock.count(), 1);
    mock.stop().await;
    f.finish().await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL; serialize tests"]
async fn accio_db_steward_publication_failure_restarts_from_durable_result_without_http() {
    let f = Fixture::new().await;
    expire(&f).await;
    reject(&f, "published").await;
    let mock = Mock::new(reply()).await;
    let mut w = worker(&f, GatewayShutdownHandle::default());
    assert_eq!(tick(&mock, &mut w).await.failed_count, 1);
    assert!(!w.has_received());
    assert_eq!(record(&f).await.phase(), GrantPhase::Result);
    assert_eq!(f.snapshot().await.payload["apiKey"], "synthetic-access");
    drop(w);
    allow(&f).await;
    let mut restarted = worker(&f, GatewayShutdownHandle::default());
    assert_eq!(tick(&mock, &mut restarted).await.refreshed_count, 1);
    assert_eq!(record(&f).await.phase(), GrantPhase::Published);
    assert_eq!(mock.count(), 1);
    mock.stop().await;
    f.finish().await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL; serialize tests"]
async fn accio_db_steward_unconfirmed_send_survives_restart_without_retry() {
    let f = Fixture::new().await;
    expire(&f).await;
    let mut answer = reply();
    answer.status = 500;
    answer.body = "private-token-private-response".into();
    let mock = Mock::new(answer).await;
    let mut w = worker(&f, GatewayShutdownHandle::default());
    let result = tick(&mock, &mut w).await;
    assert_eq!((result.due_count, result.failed_count), (1, 1));
    assert_eq!(record(&f).await.phase(), GrantPhase::Sending);
    assert!(!w.has_received());
    drop(w);
    let mut restarted = worker(&f, GatewayShutdownHandle::default());
    assert_eq!(tick(&mock, &mut restarted).await.locked_count, 1);
    assert_eq!(mock.count(), 1);
    mock.stop().await;
    f.finish().await;
}
