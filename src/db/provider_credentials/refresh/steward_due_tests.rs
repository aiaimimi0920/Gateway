//! The DB steward's restart deadline comes from actual persisted material, not token lifetime metadata.
use super::{snapshot::from_rows, snapshot_tests::rows};
use crate::provider_credential_refresh::accio::is_due;
use serde_json::json;
use std::time::Duration;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

fn check(expiry: &str, now: &str, margin: u64) -> bool {
    let (a, c, ap, mut cp) = rows();
    cp["expiresAt"] = json!(expiry);
    cp["tokenExpiresInSecs"] = json!(86_400);
    let snapshot = from_rows(&a, &c, ap, cp).unwrap();
    is_due(
        &snapshot,
        OffsetDateTime::parse(now, &Rfc3339).unwrap(),
        Duration::from_secs(margin),
    )
}

#[test]
fn accio_db_steward_expired_material_is_due_even_with_a_long_retained_lifetime() {
    assert!(check("2026-11-09T00:00:00Z", "2026-11-09T00:00:01Z", 0));
}

#[test]
fn accio_db_steward_margin_includes_exact_deadline_but_not_later_fractional_expiry() {
    assert!(check("2026-11-09T00:05:00Z", "2026-11-09T00:00:00Z", 300));
    assert!(!check(
        "2026-11-09T00:05:00.000000001Z",
        "2026-11-09T00:00:00Z",
        300
    ));
}

#[test]
fn accio_db_steward_timezone_and_restart_keep_the_same_absolute_deadline() {
    assert!(!check(
        "2026-11-09T08:10:00+08:00",
        "2026-11-09T00:00:00Z",
        300
    ));
    assert!(check(
        "2026-11-09T08:10:00+08:00",
        "2026-11-09T00:06:00Z",
        300
    ));
}

#[test]
fn accio_db_steward_fresh_material_is_not_consumed_early() {
    assert!(!check("2027-01-01T00:00:00Z", "2026-11-09T00:00:00Z", 300));
}

#[tokio::test]
async fn accio_db_steward_tick_future_is_bounded_for_default_windows_thread_stacks() {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgresql://synthetic:synthetic@127.0.0.1:1/synthetic")
        .unwrap();
    pool.close().await; // The size-only future is never polled or connected.
    let mut worker = crate::provider_credential_refresh::accio::Worker::new(
        pool,
        super::steward_wire::http(),
        crate::state::GatewayShutdownHandle::default(),
        1,
        Duration::from_secs(300),
    );
    let future = worker.tick();
    let bytes = std::mem::size_of_val(&future);
    assert!(bytes <= 65_536, "DB worker future is {bytes} bytes");
}
