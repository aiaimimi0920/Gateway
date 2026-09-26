use super::*;

struct Entry(&'static str);
impl Drop for Entry {
    fn drop(&mut self) {
        run_buckets().remove(self.0);
    }
}

#[test]
fn removes_idle_bucket_without_external_owners() {
    let key = Entry("bucket-test-idle");
    let bucket = run_bucket(key.0).unwrap();
    assert!(remove_idle_bucket(key.0, &bucket));
    assert!(!run_buckets().contains_key(key.0));
}

#[test]
fn preserves_bucket_already_borrowed_by_an_acquirer() {
    let key = Entry("bucket-test-borrowed");
    let bucket = run_bucket(key.0).unwrap();
    let acquiring = run_bucket(key.0).unwrap();
    assert!(!remove_idle_bucket(key.0, &bucket));
    acquiring.try_lock().unwrap().active = Some(ManagedRun::new("new".into()));
    assert!(Arc::ptr_eq(&run_bucket(key.0).unwrap(), &acquiring));
}

#[test]
fn stale_cleanup_cannot_remove_a_replacement_at_the_same_key() {
    let key = Entry("bucket-test-replaced");
    let stale = run_bucket(key.0).unwrap();
    run_buckets().remove(key.0);
    let replacement = run_bucket(key.0).unwrap();
    assert!(!remove_idle_bucket(key.0, &stale));
    assert!(Arc::ptr_eq(&run_bucket(key.0).unwrap(), &replacement));
}

#[test]
fn rechecks_state_and_never_waits_for_a_held_state_lock() {
    let key = Entry("bucket-test-state");
    let bucket = run_bucket(key.0).unwrap();
    let mut state = bucket.try_lock().unwrap();
    assert!(!remove_idle_bucket(key.0, &bucket));
    state.active = Some(ManagedRun::new("new".into()));
    drop(state);
    assert!(!remove_idle_bucket(key.0, &bucket));
    assert!(run_buckets().contains_key(key.0));
}

#[tokio::test]
async fn stale_lease_cleanup_preserves_a_borrowed_empty_replacement() {
    for invalidate in [false, true] {
        let key = Entry(if invalidate {
            "stale-invalidate"
        } else {
            "stale-release"
        });
        let replacement = run_bucket(key.0).unwrap();
        let lifetime = Arc::new(RunLifetime::untracked());
        lifetime.acquire().unwrap();
        let lease = FreeBuffRunLease {
            runtime: test_runtime().clone(),
            bucket_key: key.0.into(),
            run_id: "old-run".into(),
            lifetime,
            released: false,
        };
        if invalidate {
            let payload = crate::protocol::freebuff::tests::make_payload();
            let mut config = FreeBuffRuntimeConfig::from_payload(&payload, "z-ai/glm-5.1").unwrap();
            config.bucket_key = key.0.into();
            invalidate_run_lease(&config, lease, "test").await;
        } else {
            release_run_lease(lease).await;
        }
        assert!(Arc::ptr_eq(&replacement, &run_bucket(key.0).unwrap()));
    }
}
