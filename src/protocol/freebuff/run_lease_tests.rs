use super::tests::make_payload;
use super::*;

struct Fixture {
    client: Client,
    config: FreeBuffRuntimeConfig,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        run_buckets().remove(&self.config.bucket_key);
    }
}

async fn setup(name: &str) -> Fixture {
    let mut config = FreeBuffRuntimeConfig::from_payload(&make_payload(), "z-ai/glm-5.1").unwrap();
    config.bucket_key = format!("lease-test-{name}");
    run_bucket(&config.bucket_key).unwrap().lock().await.active =
        Some(ManagedRun::new("run".into()));
    Fixture {
        client: Client::builder().build().unwrap(),
        config,
    }
}

async fn inflight(f: &Fixture) -> u64 {
    run_bucket(&f.config.bucket_key)
        .unwrap()
        .lock()
        .await
        .active
        .as_ref()
        .unwrap()
        .inflight()
}

fn handle(f: &Fixture, lease: FreeBuffRunLease) -> FreeBuffLeaseHandle {
    FreeBuffLeaseHandle {
        config: f.config.clone(),
        lease: Some(lease),
    }
}

#[tokio::test]
async fn dropped_raw_leases_release_exactly_one_inflight_each() {
    let f = setup("raw").await;
    let first = acquire_run_lease(&f.client, &f.config).await.unwrap();
    let second = acquire_run_lease(&f.client, &f.config).await.unwrap();
    assert_eq!(inflight(&f).await, 2);
    drop(first);
    assert_eq!(inflight(&f).await, 1);
    drop(second);
    assert_eq!(inflight(&f).await, 0);
}

#[tokio::test]
async fn dropped_public_handle_releases_inflight_without_async_cleanup() {
    let f = setup("handle").await;
    let lease = acquire_run_lease(&f.client, &f.config).await.unwrap();
    drop(handle(&f, lease));
    assert_eq!(inflight(&f).await, 0);
}

#[tokio::test]
async fn cancelled_release_and_invalidate_waits_do_not_leak_inflight() {
    for invalidate in [false, true] {
        let f = setup(if invalidate { "invalidate" } else { "release" }).await;
        let lease = acquire_run_lease(&f.client, &f.config).await.unwrap();
        let handle = handle(&f, lease);
        let bucket = run_bucket(&f.config.bucket_key).unwrap();
        let guard = bucket.lock().await;
        let task = tokio::spawn(async move {
            if invalidate {
                handle.invalidate("test").await;
            } else {
                handle.release().await;
            }
        });
        tokio::task::yield_now().await;
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        drop(guard);
        assert_eq!(inflight(&f).await, 0);
    }
}

#[tokio::test]
async fn explicit_release_does_not_double_decrement_other_lease() {
    let f = setup("explicit").await;
    let first = acquire_run_lease(&f.client, &f.config).await.unwrap();
    let second = acquire_run_lease(&f.client, &f.config).await.unwrap();
    handle(&f, first).release().await;
    assert_eq!(inflight(&f).await, 1);
    handle(&f, second).release().await;
    assert_eq!(inflight(&f).await, 0);
}
