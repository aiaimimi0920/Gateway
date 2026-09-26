use super::*;
use crate::protocol::freebuff::session_response::parse_free_session_snapshot;
use crate::protocol::freebuff::tests::make_payload;
use std::time::Instant;

fn config() -> FreeBuffRuntimeConfig {
    FreeBuffRuntimeConfig::from_payload(&make_payload(), "z-ai/glm-5.1").unwrap()
}

fn insert_snapshot(registry: &SessionRegistry, state: FreeBuffSessionState) {
    let mut snapshot =
        parse_free_session_snapshot(&serde_json::json!({"status":"disabled"}), &config()).unwrap();
    snapshot.state = state;
    snapshot.instance_id = Some("preserved-claim".into());
    snapshot.refresh_after = Instant::now();
    registry
        .get_or_insert("old")
        .unwrap()
        .try_lock()
        .unwrap()
        .set_snapshot(Some(snapshot));
}

#[test]
fn pressure_reclaims_empty_bucket() {
    let registry = SessionRegistry::new(1);
    drop(registry.get_or_insert("old").unwrap());
    let _new = registry.get_or_insert("new").unwrap();
    assert!(!registry.buckets.contains_key("old"));
    assert_eq!(registry.buckets.len(), 1);
}

#[test]
fn pressure_reclaims_only_terminal_or_disabled_snapshots() {
    for state in [
        FreeBuffSessionState::None,
        FreeBuffSessionState::Disabled,
        FreeBuffSessionState::Expired,
        FreeBuffSessionState::Superseded,
    ] {
        let registry = SessionRegistry::new(1);
        insert_snapshot(&registry, state);
        let _new = registry.get_or_insert("new").unwrap();
        assert!(!registry.buckets.contains_key("old"));
        assert_eq!(registry.buckets.len(), 1);
    }
}

#[test]
fn stale_refresh_does_not_authorize_discarding_live_claims() {
    for state in [
        FreeBuffSessionState::Active,
        FreeBuffSessionState::Queued,
        FreeBuffSessionState::Draining,
    ] {
        let registry = SessionRegistry::new(1);
        insert_snapshot(&registry, state);
        assert_eq!(
            registry.get_or_insert("new").unwrap_err().code.as_deref(),
            Some("freebuff_session_registry_capacity_exhausted")
        );
        let old = registry.get_or_insert("old").unwrap();
        assert_eq!(
            old.try_lock()
                .unwrap()
                .snapshot
                .as_ref()
                .unwrap()
                .instance_id
                .as_deref(),
            Some("preserved-claim")
        );
    }
}

#[test]
fn borrowed_bucket_preserves_identity_and_existing_lookup_at_capacity() {
    let registry = SessionRegistry::new(1);
    let borrowed = registry.get_or_insert("old").unwrap();
    assert!(registry.get_or_insert("new").is_err());
    assert!(Arc::ptr_eq(
        &borrowed,
        &registry.get_or_insert("old").unwrap()
    ));
}

#[test]
fn removed_bucket_holds_capacity_until_last_owner_drops() {
    let registry = SessionRegistry::new(1);
    let owner = registry.get_or_insert("old").unwrap();
    registry.buckets.remove("old");
    assert!(registry.get_or_insert("new").is_err());
    drop(owner);
    assert!(registry.get_or_insert("new").is_ok());
}

#[tokio::test]
async fn rejection_cache_pressure_preserves_existing_claim_without_insertion() {
    let registry = SessionRegistry::new(1);
    insert_snapshot(&registry, FreeBuffSessionState::Active);
    let mut config = config();
    config.session_bucket_key = "other".into();
    registry
        .record_rejection(
            &config,
            &SessionObservation::default(),
            FreeBuffWaitingRoomRejection::WaitingRoomRequired,
            "{}",
        )
        .await;
    assert_eq!(registry.buckets.len(), 1);
    assert!(!registry.buckets.contains_key("other"));
    assert!(registry.buckets.contains_key("old"));
}

#[tokio::test]
async fn existing_rejection_updates_at_capacity_and_allows_terminal_eviction() {
    let registry = SessionRegistry::new(1);
    insert_snapshot(&registry, FreeBuffSessionState::Active);
    let mut config = config();
    config.session_bucket_key = "old".into();
    let bucket = registry.get_or_insert("old").unwrap();
    let observation = SessionObservation::capture(
        &bucket,
        &bucket.try_lock().unwrap(),
        Some("preserved-claim".into()),
    );
    registry
        .record_rejection(
            &config,
            &observation,
            FreeBuffWaitingRoomRejection::SessionExpired,
            "{}",
        )
        .await;
    let old = registry.get_or_insert("old").unwrap();
    assert_eq!(
        old.try_lock().unwrap().snapshot.as_ref().unwrap().state,
        FreeBuffSessionState::Expired
    );
    drop(old);
    drop(observation);
    drop(bucket);
    assert!(registry.get_or_insert("new").is_ok());
    assert!(!registry.buckets.contains_key("old"));
}

#[test]
fn concurrent_creators_obey_capacity_and_return_duplicate_reservations() {
    for same_key in [false, true] {
        let capacity = if same_key { 32 } else { 4 };
        let registry = Arc::new(SessionRegistry::new(capacity));
        let barrier = Arc::new(std::sync::Barrier::new(16));
        let handles: Vec<_> = (0..16)
            .map(|index| {
                let registry = registry.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    registry.get_or_insert(&format!("key-{}", if same_key { 0 } else { index }))
                })
            })
            .collect();
        let owners: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert!(owners.iter().any(Result::is_ok));
        assert!(registry.buckets.len() <= capacity);
        if same_key {
            assert!(owners.iter().all(Result::is_ok));
            assert_eq!(registry.buckets.len(), 1);
            assert_eq!(registry.slots.available_permits(), capacity - 1);
        }
    }
}
