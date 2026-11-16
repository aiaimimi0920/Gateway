//! Reuse the guarded DB schema and exact synthetic Accio HTTP peer, not a mock storage backend.
use super::{
    fixture::Fixture,
    steward_wire::{http, Mock, Reply},
};
use crate::{
    console::refresh_grants::GrantRecord,
    protocol::accio_oauth::TEST_REFRESH_ENDPOINT,
    provider_credential_refresh::{accio::Worker, ProviderCredentialRefreshSweepSummary},
    state::GatewayShutdownHandle,
};
use serde_json::json;
use std::time::Duration;

pub(super) fn reply() -> Reply {
    Reply {
        body: json!({"success":true,"data":{"accessToken":"rotated-synthetic-access",
        "refreshToken":"rotated-synthetic-refresh", "expiresAt":"2027-01-01T00:00:00.123456789Z"}})
        .to_string(),
        ..Default::default()
    }
}
pub(super) fn worker(f: &Fixture, shutdown: GatewayShutdownHandle) -> Worker {
    Worker::new(
        f.pool.clone(),
        http(),
        shutdown,
        100,
        Duration::from_secs(300),
    )
}
pub(super) async fn tick(
    mock: &Mock,
    worker: &mut Worker,
) -> ProviderCredentialRefreshSweepSummary {
    TEST_REFRESH_ENDPOINT
        .scope(mock.endpoint.clone(), worker.tick())
        .await
        .unwrap()
}
pub(super) async fn expire(f: &Fixture) {
    sqlx::query("update gateway_provider_credentials set payload_inline=jsonb_set(payload_inline,'{expiresAt}','\"2000-01-01T00:00:00Z\"'::jsonb) where id=$1")
        .bind(&f.credential).execute(&f.pool).await.unwrap();
}
pub(super) async fn record(f: &Fixture) -> GrantRecord {
    let raw: String = sqlx::query_scalar("select record from gateway_provider_refresh_grants")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    GrantRecord::from_json(&raw).unwrap()
}
pub(super) async fn reject(f: &Fixture, phase: &str) {
    assert!(matches!(phase, "result" | "published"));
    sqlx::raw_sql(&format!("create function reject_steward_phase() returns trigger language plpgsql as $$ begin raise exception 'synthetic_storage_failure'; end $$; create trigger reject_steward_phase before update on gateway_provider_refresh_grants for each row when ((new.record::jsonb->>'phase')='{phase}') execute function reject_steward_phase();"))
        .execute(&f.pool).await.unwrap();
}
pub(super) async fn allow(f: &Fixture) {
    sqlx::query("drop trigger reject_steward_phase on gateway_provider_refresh_grants")
        .execute(&f.pool)
        .await
        .unwrap();
}
