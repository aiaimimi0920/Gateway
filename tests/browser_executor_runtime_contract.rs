mod console_contract_support;
#[path = "browser_executor_runtime_contract/fixture.rs"]
mod fixture;
mod support;

use fixture::LeaseFixture;
use neuro_gateway::browser_executor_runtime::{
    list_browser_capability_leases, BrowserCapabilityLeaseFilters, BrowserCapabilitySlotStatus,
};
use neuro_gateway::error::ErrorKind;
use neuro_gateway::redis::keys;
use redis::AsyncCommands;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

async fn assert_owned_release(reason: &str, status: BrowserCapabilitySlotStatus) {
    let fixture = LeaseFixture::new().await;
    let lease = fixture.acquire(None).await.unwrap();
    let before = fixture.slot().await;
    assert_eq!(before.status, BrowserCapabilitySlotStatus::Busy);
    let released = fixture.release(&lease, Some(reason)).await.unwrap();
    assert!(fixture.lock().await.is_none());
    assert_eq!(released.lease_id, lease.lease_id);
    assert_eq!(released.release_reason.as_deref(), Some(reason));
    let mut expected_slot = before;
    expected_slot.status = status;
    expected_slot.updated_at = released.released_at.clone().unwrap();
    if status == BrowserCapabilitySlotStatus::Cooling {
        expected_slot.last_failure_at = released.released_at.clone();
    }
    assert_eq!(
        serde_json::to_value(fixture.slot().await).unwrap(),
        serde_json::to_value(expected_slot).unwrap()
    );
    let mut conn = fixture.state.redis_pool.get().await.unwrap();
    let stored: String = conn
        .get(keys::browser_executor_lease_key(&lease.lease_id))
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&stored).unwrap(),
        serde_json::to_value(released).unwrap()
    );
}

#[tokio::test]
#[ignore = "requires an isolated, guarded Redis fixture"]
async fn current_owner_clean_release_preserves_metadata() {
    assert_owned_release("completed", BrowserCapabilitySlotStatus::Warm).await;
}

#[tokio::test]
#[ignore = "requires an isolated, guarded Redis fixture"]
async fn current_owner_failed_release_records_cooling() {
    assert_owned_release("browser timeout", BrowserCapabilitySlotStatus::Cooling).await;
}

#[tokio::test]
#[ignore = "requires an isolated, guarded Redis fixture"]
async fn superseded_release_preserves_the_replacement_owner() {
    let fixture = LeaseFixture::new().await;
    let old = fixture.acquire(None).await.unwrap();
    fixture.expire_lock().await;
    fixture.warm_slot().await;
    let replacement = fixture.acquire(None).await.unwrap();
    let slot = fixture.slot_raw().await;
    let released = fixture.release(&old, Some("old timeout")).await.unwrap();
    assert!(released.released_at.is_some());
    assert_eq!(
        fixture.lock().await.as_deref(),
        Some(replacement.lease_id.as_str())
    );
    assert_eq!(fixture.slot_raw().await, slot);
}

#[tokio::test]
#[ignore = "requires an isolated, guarded Redis fixture"]
async fn expired_release_does_not_change_slot_availability() {
    let fixture = LeaseFixture::new().await;
    let lease = fixture.acquire(None).await.unwrap();
    fixture.expire_lock().await;
    let slot = fixture.slot_raw().await;
    let released = fixture.release(&lease, Some("completed")).await.unwrap();
    assert!(released.released_at.is_some());
    assert!(fixture.lock().await.is_none());
    assert_eq!(fixture.slot_raw().await, slot);
}

#[tokio::test]
#[ignore = "requires an isolated, guarded Redis fixture"]
async fn malformed_slot_release_retains_the_owned_lock() {
    let fixture = LeaseFixture::new().await;
    let lease = fixture.acquire(None).await.unwrap();
    let mut conn = fixture.state.redis_pool.get().await.unwrap();
    let _: () = conn
        .set(
            keys::browser_executor_slot_key(&fixture.slot_id),
            "invalid slot JSON",
        )
        .await
        .unwrap();
    drop(conn);
    let error = fixture.release(&lease, None).await.unwrap_err();
    assert_eq!(error.kind, ErrorKind::ServerError);
    assert_eq!(
        fixture.lock().await.as_deref(),
        Some(lease.lease_id.as_str())
    );
}

#[tokio::test]
#[ignore = "requires an isolated, guarded Redis fixture"]
async fn missing_slot_does_not_prevent_owned_lease_cleanup() {
    let fixture = LeaseFixture::new().await;
    let lease = fixture.acquire(None).await.unwrap();
    let mut conn = fixture.state.redis_pool.get().await.unwrap();
    let _: usize = conn
        .del(keys::browser_executor_slot_key(&fixture.slot_id))
        .await
        .unwrap();
    drop(conn);
    let released = fixture.release(&lease, None).await.unwrap();
    assert!(released.released_at.is_some());
    assert!(fixture.lock().await.is_none());
    let mut conn = fixture.state.redis_pool.get().await.unwrap();
    let slot: Option<String> = conn
        .get(keys::browser_executor_slot_key(&fixture.slot_id))
        .await
        .unwrap();
    assert!(slot.is_none());
}

#[tokio::test]
#[ignore = "requires an isolated, guarded Redis fixture"]
async fn representable_ttls_keep_defaults_minimum_and_retention() {
    let fixture = LeaseFixture::new().await;
    for (input, expected) in [(None, 300u64), (Some(0), 30), (Some(86_400), 86_400)] {
        let started = std::time::Instant::now();
        let lease = fixture.acquire(input).await.unwrap();
        let issued = OffsetDateTime::parse(&lease.issued_at, &Rfc3339).unwrap();
        let expires =
            OffsetDateTime::parse(lease.expires_at.as_deref().unwrap(), &Rfc3339).unwrap();
        assert!((expires - issued).whole_seconds() >= expected as i64);
        let mut conn = fixture.state.redis_pool.get().await.unwrap();
        let lock_ttl: i64 = conn
            .ttl(keys::browser_executor_slot_lease_lock_key(&fixture.slot_id))
            .await
            .unwrap();
        let record_ttl: i64 = conn
            .ttl(keys::browser_executor_lease_key(&lease.lease_id))
            .await
            .unwrap();
        let elapsed = started.elapsed().as_secs() + 1;
        assert!((expected.saturating_sub(elapsed) as i64..=expected as i64).contains(&lock_ttl));
        assert!(
            ((expected + 3600).saturating_sub(elapsed) as i64..=(expected + 3600) as i64)
                .contains(&record_ttl)
        );
        drop(conn);
        fixture.release(&lease, Some("completed")).await.unwrap();
    }
}

async fn assert_invalid_ttl(ttl: u64) {
    let fixture = LeaseFixture::new().await;
    let slot = fixture.slot_raw().await;
    let error = fixture.acquire(Some(ttl)).await.unwrap_err();
    assert_eq!(error.kind, ErrorKind::BadRequest);
    assert_eq!(error.http_status, Some(400));
    assert!(fixture.lock().await.is_none());
    assert_eq!(fixture.slot_raw().await, slot);
    let leases = list_browser_capability_leases(
        &fixture.state,
        BrowserCapabilityLeaseFilters {
            provider_account_id: Some(fixture.account_id.clone()),
            ..BrowserCapabilityLeaseFilters::default()
        },
    )
    .await
    .unwrap();
    assert!(leases.is_empty());
}

#[tokio::test]
#[ignore = "requires an isolated, guarded Redis fixture"]
async fn overflowing_retention_ttl_is_rejected_before_locking() {
    assert_invalid_ttl(u64::MAX).await;
}

#[tokio::test]
#[ignore = "requires an isolated, guarded Redis fixture"]
async fn signed_timestamp_overflow_is_rejected_before_locking() {
    assert_invalid_ttl(i64::MAX as u64).await;
}

#[tokio::test]
#[ignore = "requires an isolated, guarded Redis fixture"]
async fn unsigned_timestamp_overflow_is_rejected_before_locking() {
    assert_invalid_ttl(i64::MAX as u64 + 1).await;
}
