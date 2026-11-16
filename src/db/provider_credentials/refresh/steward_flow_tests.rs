//! Normal DB scans and dedicated HTTP publish complete material without touching hosted Redis identity.
use super::{
    fixture::Fixture,
    journal,
    steward_fixture::*,
    steward_wire::{http, Mock},
};
use crate::{
    console::refresh_grants::GrantPhase, provider_credential_refresh::accio::Worker,
    state::GatewayShutdownHandle,
};
use serde_json::{json, Value};
use std::time::Duration;

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL; serialize tests"]
async fn accio_db_steward_expired_scan_sends_once_and_normal_reader_sees_exact_successor() {
    let f = Fixture::new().await;
    expire(&f).await;
    let before = f.row().await;
    let mock = Mock::new(reply()).await;
    let mut w = worker(&f, GatewayShutdownHandle::default());
    let summary = tick(&mock, &mut w).await;
    assert_eq!(
        (
            summary.scanned_count,
            summary.due_count,
            summary.refreshed_count,
            summary.failed_count
        ),
        (1, 1, 1, 0)
    );
    assert_eq!(mock.count(), 1);
    assert_eq!(record(&f).await.phase(), GrantPhase::Published);
    let ordinary = crate::db::get_provider_credential(&f.pool, &f.credential)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ordinary.payload["apiKey"], "rotated-synthetic-access");
    assert_eq!(
        ordinary.payload["refreshToken"],
        "rotated-synthetic-refresh"
    );
    assert_eq!(ordinary.payload["accioSgcookie"], "synthetic opaque sg");
    assert_eq!(
        ordinary.payload["expiresAt"],
        "2027-01-01T00:00:00.123456789Z"
    );
    assert_eq!(ordinary.payload["headers"]["utdid"], "synthetic-device");
    let after = f.row().await;
    for key in [
        "label",
        "status",
        "source_kind",
        "source_hash",
        "sync_error",
        "last_error",
        "failure_count",
    ] {
        assert_eq!(before[key], after[key], "metadata {key}");
    }
    let calls = mock.calls.lock().unwrap().clone();
    assert_eq!(
        serde_json::from_str::<Value>(&calls[0].1).unwrap(),
        json!({"accessToken":"synthetic-access","refreshToken":"synthetic-refresh"})
    );
    assert_eq!(calls[0].0["content-type"], "application/json");
    for name in ["authorization", "cookie", "x-api-key"] {
        assert!(!calls[0].0.contains_key(name));
    }
    drop(w);
    let mut restarted = worker(&f, GatewayShutdownHandle::default());
    assert_eq!(tick(&mock, &mut restarted).await.refreshed_count, 0);
    assert_eq!(mock.count(), 1);
    mock.stop().await;
    f.finish().await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL; serialize tests"]
async fn accio_db_steward_two_workers_share_one_permanent_sender() {
    let f = Fixture::new().await;
    expire(&f).await;
    let mock = Mock::new(reply()).await;
    let mut left = worker(&f, GatewayShutdownHandle::default());
    let mut right = worker(&f, GatewayShutdownHandle::default());
    let (l, r) = tokio::join!(tick(&mock, &mut left), tick(&mock, &mut right));
    // Each sweep may acknowledge the same idempotent publication; the wire count
    // is the unique sender evidence, not a sum of per-worker acknowledgements.
    assert!(l.refreshed_count + r.refreshed_count >= 1);
    assert_eq!(mock.count(), 1);
    assert_eq!(record(&f).await.phase(), GrantPhase::Published);
    mock.stop().await;
    f.finish().await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL; serialize tests"]
async fn accio_db_steward_keyset_batch_does_not_starve_due_rows_behind_fresh_rows() {
    let mut f = Fixture::new().await;
    let old = f.credential.clone();
    f.credential = format!("a-{}", uuid::Uuid::new_v4().simple());
    sqlx::query("update gateway_provider_credentials set id=$2,payload_inline=jsonb_set(payload_inline,'{expiresAt}','\"2099-01-01T00:00:00Z\"'::jsonb) where id=$1")
        .bind(old).bind(&f.credential).execute(&f.pool).await.unwrap();
    let due = format!("z-{}", uuid::Uuid::new_v4().simple());
    sqlx::query("insert into gateway_provider_credentials(id,provider_account_id,label,payload_inline,source_kind,sync_mode) select $2,provider_account_id,'later due',jsonb_set(payload_inline,'{expiresAt}','\"2000-01-01T00:00:00Z\"'::jsonb),'manual','manual' from gateway_provider_credentials where id=$1")
        .bind(&f.credential).bind(&due).execute(&f.pool).await.unwrap();
    let mock = Mock::new(reply()).await;
    let mut w = Worker::new(
        f.pool.clone(),
        http(),
        GatewayShutdownHandle::default(),
        1,
        Duration::from_secs(300),
    );
    assert_eq!(tick(&mock, &mut w).await.due_count, 0);
    assert_eq!(mock.count(), 0);
    assert_eq!(tick(&mock, &mut w).await.refreshed_count, 1);
    assert_eq!(mock.count(), 1);
    assert_eq!(
        crate::db::get_provider_credential(&f.pool, &due)
            .await
            .unwrap()
            .unwrap()
            .payload["apiKey"],
        "rotated-synthetic-access"
    );
    mock.stop().await;
    f.finish().await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL; serialize tests"]
async fn accio_db_steward_saved_result_is_recovered_before_fresh_due_filtering() {
    let f = Fixture::new().await;
    sqlx::query("update gateway_provider_credentials set payload_inline=jsonb_set(payload_inline,'{expiresAt}','\"2099-01-01T00:00:00Z\"'::jsonb) where id=$1")
        .bind(&f.credential).execute(&f.pool).await.unwrap();
    let (snapshot, sending) = f.sending().await;
    journal::store_result(&f.pool, &sending, &Fixture::result(&snapshot))
        .await
        .unwrap();
    let mock = Mock::new(reply()).await;
    let mut w = worker(&f, GatewayShutdownHandle::default());
    let result = tick(&mock, &mut w).await;
    assert_eq!((result.due_count, result.refreshed_count), (0, 1));
    assert_eq!(mock.count(), 0);
    assert_eq!(record(&f).await.phase(), GrantPhase::Published);
    mock.stop().await;
    f.finish().await;
}
