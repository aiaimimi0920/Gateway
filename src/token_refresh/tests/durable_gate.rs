//! Real scheduler admission: synthetic grants, separate runtime owners and actual Local evidence.
use super::*;
use crate::console::{refresh_grants::GrantRequest, RouteConfigRedisRevision};

#[tokio::test]
async fn unknown_send_survives_restart_without_retrying_the_same_grant() {
    let mock = MockOAuth::new(503, "outcome unknown", 0).await;
    let mut fixture = LocalFixture::new(&mock.url).await;
    fixture.refresh(&mut Pending::new()).await;
    assert_eq!(mock.count(), 1);
    fixture.runtime = RouteConfigRuntime::new_local(&fixture.config).unwrap();
    fixture.runtime.recover_startup().await.unwrap();
    fixture.refresh(&mut Pending::new()).await;
    assert_eq!(
        mock.count(),
        1,
        "restarting must not erase Sending evidence"
    );
}

#[tokio::test]
async fn local_writer_lock_blocks_reservation_before_any_http_request() {
    let mock = MockOAuth::new(200, ROTATED, 0).await;
    let fixture = LocalFixture::new(&mock.url).await;
    let persistence =
        RouteConfigPersistence::new(&fixture.config.state_dir, &fixture.config.routes_file)
            .unwrap();
    let guard = persistence.try_writer_lock().unwrap();
    fixture.refresh(&mut Pending::new()).await;
    assert_eq!(mock.count(), 0, "no authority reservation, no send");
    drop(guard);
    fixture.refresh(&mut Pending::new()).await;
    assert_eq!(mock.count(), 1);
}

#[tokio::test]
async fn absent_authority_runtime_never_consumes_a_refresh_grant() {
    let mock = MockOAuth::new(200, ROTATED, 0).await;
    let fixture = LocalFixture::new(&mock.url).await;
    refresh_once(
        fixture.runtime.route_config(),
        &client(),
        None,
        None,
        &mut Pending::new(),
        Duration::from_secs(300),
        None,
    )
    .await;
    assert_eq!(mock.count(), 0);
}

#[tokio::test]
async fn separate_local_runtime_owners_do_not_send_the_same_material_twice() {
    let mock = MockOAuth::new(200, ROTATED, 80).await;
    let fixture = LocalFixture::new(&mock.url).await;
    let other = RouteConfigRuntime::new_local(&fixture.config).unwrap();
    other.recover_startup().await.unwrap();
    let (mut a, mut b) = (Pending::new(), Pending::new());
    let http = client();
    tokio::join!(
        fixture.refresh(&mut a),
        refresh_once(
            other.route_config(),
            &http,
            Some(&other),
            None,
            &mut b,
            Duration::from_secs(300),
            None
        )
    );
    assert_eq!(
        mock.count(),
        1,
        "Arc single-flight alone cannot coordinate separate runtimes"
    );
}

#[tokio::test]
async fn durable_result_is_recovered_without_sending_a_second_http_grant() {
    let mock = MockOAuth::new(200, ROTATED, 0).await;
    let fixture = LocalFixture::new(&mock.url).await;
    let snapshot = fixture.runtime.route_config().snapshot();
    let revision =
        RouteConfigRedisRevision::new(snapshot.revision().clone(), snapshot.document().clone())
            .unwrap();
    let request = GrantRequest::from_revision(&revision, "oauth", "account").unwrap();
    let persistence =
        RouteConfigPersistence::new(&fixture.config.state_dir, &fixture.config.routes_file)
            .unwrap();
    let guard = persistence.try_writer_lock().unwrap();
    let crate::console::refresh_grants::GrantReservation::Reserved(record) = persistence
        .reserve_refresh_grant_locked(&guard, &revision, &request)
        .unwrap()
    else {
        panic!("expected a new reservation")
    };
    let expiry = future_rfc3339_after_secs(3600);
    let result = request
        .result(
            "recovered-access",
            Some("recovered-refresh"),
            3600,
            &expiry,
            None,
        )
        .unwrap();
    persistence
        .store_refresh_grant_result_locked(&guard, &record, &result)
        .unwrap();
    drop(guard);
    fixture.refresh(&mut Pending::new()).await;
    assert_eq!(
        mock.count(),
        0,
        "Result is replayed locally, never re-exchanged"
    );
    assert_eq!(fixture.saved().api_key.as_deref(), Some("recovered-access"));
    assert_eq!(fixture.saved().expires_at.as_deref(), Some(expiry.as_str()));
}
