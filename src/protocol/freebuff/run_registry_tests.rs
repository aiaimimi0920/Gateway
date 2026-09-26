use super::super::ManagedRun;
use super::*;

#[test]
fn full_registry_rejects_new_key_without_splitting_borrowed_bucket() {
    let registry = RunRegistry::new(1);
    let borrowed = registry.get_or_insert("same").unwrap();
    let error = registry.get_or_insert("other").unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("freebuff_run_registry_capacity_exhausted")
    );
    assert!(Arc::ptr_eq(
        &borrowed,
        &registry.get_or_insert("same").unwrap()
    ));
    assert_eq!(registry.buckets.len(), 1);
}

#[test]
fn capacity_pressure_evicts_unborrowed_empty_bucket() {
    let registry = RunRegistry::new(1);
    drop(registry.get_or_insert("old").unwrap());
    let _new = registry.get_or_insert("new").unwrap();
    assert!(!registry.buckets.contains_key("old"));
    assert_eq!(registry.buckets.len(), 1);
}

#[test]
fn pressure_preserves_active_run_with_live_lease_then_reclaims_idle_owner() {
    let registry = RunRegistry::new(1);
    let bucket = registry.get_or_insert("live").unwrap();
    let run = ManagedRun::new("run".into());
    run.lifetime.acquire().unwrap();
    let lease_owner = run.lifetime.clone();
    bucket.try_lock().unwrap().active = Some(run);
    drop(bucket);
    assert!(registry.get_or_insert("other").is_err());
    lease_owner.release();
    drop(lease_owner);
    let _new = registry.get_or_insert("other").unwrap();
    assert!(!registry.buckets.contains_key("live"));
    assert_eq!(registry.buckets.len(), 1);
}

#[test]
fn reserved_capacity_does_not_block_existing_lookup() {
    let registry = RunRegistry::new(2);
    let existing = registry.get_or_insert("existing").unwrap();
    let _held = registry.slots.clone().try_acquire_owned().unwrap();
    assert!(registry.get_or_insert("new").is_err());
    assert!(Arc::ptr_eq(
        &existing,
        &registry.get_or_insert("existing").unwrap()
    ));
}

#[test]
fn concurrent_distinct_keys_cannot_overshoot_capacity() {
    let registry = Arc::new(RunRegistry::new(4));
    let barrier = Arc::new(std::sync::Barrier::new(16));
    let handles: Vec<_> = (0..16)
        .map(|index| {
            let registry = registry.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                registry.get_or_insert(&format!("key-{index}"))
            })
        })
        .collect();
    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    assert!(results.iter().any(Result::is_ok));
    assert!(registry.buckets.len() <= 4);
    assert!(results.iter().filter(|result| result.is_ok()).count() <= 4);
}

#[test]
fn removed_bucket_keeps_capacity_until_its_last_external_owner_drops() {
    let registry = RunRegistry::new(1);
    let owner = registry.get_or_insert("old").unwrap();
    registry.buckets.remove("old");
    assert!(registry.get_or_insert("new").is_err());
    drop(owner);
    assert!(registry.get_or_insert("new").is_ok());
}

#[test]
fn concurrent_same_key_creators_return_unused_reservations() {
    let registry = Arc::new(RunRegistry::new(32));
    let barrier = Arc::new(std::sync::Barrier::new(16));
    let handles: Vec<_> = (0..16)
        .map(|_| {
            let registry = registry.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                registry.get_or_insert("same").unwrap()
            })
        })
        .collect();
    let owners: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    assert!(owners.iter().all(|owner| Arc::ptr_eq(owner, &owners[0])));
    assert_eq!(registry.buckets.len(), 1);
    assert_eq!(registry.slots.available_permits(), 31);
    drop(owners);
    registry.buckets.remove("same");
    assert_eq!(registry.slots.available_permits(), 32);
}

#[test]
fn failed_entry_admission_does_not_insert_or_keep_registry_permit() {
    let registry = RunRegistry::new(1);
    let error = registry
        .get_or_insert_checked("late", || {
            Err(GatewayError::service_unavailable("test closed").with_code("closed"))
        })
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("closed"));
    assert!(registry.buckets.is_empty());
    assert_eq!(registry.slots.available_permits(), 1);
    assert!(registry.get_or_insert("allowed").is_ok());
}
