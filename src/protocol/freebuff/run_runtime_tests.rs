use super::*;
use futures::FutureExt;
use std::sync::Arc;

#[path = "run_runtime_fixture.rs"]
mod fixture;
use fixture::Fixture;

fn budget() -> Duration {
    Duration::from_secs(2)
}

#[tokio::test]
async fn shutdown_retires_cached_run_and_waits_for_finish_response() {
    let mut f = Fixture::new().await;
    drop(f.ready_lease().await);
    let owner = f.owner.clone();
    let mut shutdown = Box::pin(owner.shutdown(budget()));
    assert!(shutdown.as_mut().now_or_never().is_none());
    f.received("FINISH").await;
    assert!(shutdown.as_mut().now_or_never().is_none());
    let error = owner
        .acquire_run_lease(&f.client, &f.config)
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("freebuff_run_admission_closed"));
    f.finish_reply.take().unwrap().send(()).unwrap();
    shutdown.await.unwrap();
    assert!(owner.registry.buckets().is_empty());
}

#[tokio::test]
async fn shutdown_retains_live_lease_until_its_final_drop() {
    let mut f = Fixture::new().await;
    let lease = f.ready_lease().await;
    let owner = f.owner.clone();
    let mut shutdown = Box::pin(owner.shutdown(budget()));
    assert!(shutdown.as_mut().now_or_never().is_none());
    assert!(f.requests.try_recv().is_err());
    drop(lease);
    f.received("FINISH").await;
    assert!(shutdown.as_mut().now_or_never().is_none());
    f.finish_reply.take().unwrap().send(()).unwrap();
    shutdown.await.unwrap();
}

#[tokio::test]
async fn shutdown_timeout_can_be_retried_without_reopening_or_aborting_lease() {
    let mut f = Fixture::new().await;
    let lease = f.ready_lease().await;
    let error = f
        .owner
        .shutdown(Duration::from_millis(10))
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("freebuff_run_shutdown_timeout"));
    assert!(f.owner.ensure_open().is_err());
    assert!(f.requests.try_recv().is_err());
    drop(lease);
    f.received("FINISH").await;
    f.finish_reply.take().unwrap().send(()).unwrap();
    f.owner.shutdown(budget()).await.unwrap();
}

async fn shutdown_during_start(cancel_caller: bool) {
    let mut f = Fixture::new().await;
    let caller = f.acquire();
    f.received("START").await;
    if cancel_caller {
        caller.abort();
    }
    let owner = f.owner.clone();
    let mut shutdown = Box::pin(owner.shutdown(budget()));
    assert!(shutdown.as_mut().now_or_never().is_none());
    f.start_reply.take().unwrap().send(()).unwrap();
    if cancel_caller {
        assert!(caller.await.unwrap_err().is_cancelled());
    } else {
        assert_eq!(
            caller.await.unwrap().unwrap_err().code.as_deref(),
            Some("freebuff_run_admission_closed")
        );
    }
    // The shutdown future may still be waiting for the bucket lock released by acquisition.
    assert!(shutdown.as_mut().now_or_never().is_none());
    f.received("FINISH").await;
    f.finish_reply.take().unwrap().send(()).unwrap();
    shutdown.await.unwrap();
    assert!(owner.registry.buckets().is_empty());
}

#[tokio::test]
async fn shutdown_catches_detached_start_after_caller_cancellation() {
    shutdown_during_start(true).await;
}

#[tokio::test]
async fn shutdown_prevents_late_start_from_repopulating_cache() {
    shutdown_during_start(false).await;
}

#[tokio::test]
async fn same_key_in_other_runtime_remains_usable_after_shutdown() {
    let mut a = Fixture::new().await;
    let mut b = Fixture::new().await;
    drop(a.ready_lease().await);
    drop(b.ready_lease().await);
    let owner = a.owner.clone();
    let shutdown = tokio::spawn(async move { owner.shutdown(budget()).await });
    a.received("FINISH").await;
    a.finish_reply.take().unwrap().send(()).unwrap();
    shutdown.await.unwrap().unwrap();
    let lease = b
        .owner
        .acquire_run_lease(&b.client, &b.config)
        .await
        .unwrap();
    assert_eq!(lease.run_id, "runtime-run");
    assert!(b.requests.try_recv().is_err());
    drop(lease);
    let owner = b.owner.clone();
    let shutdown = tokio::spawn(async move { owner.shutdown(budget()).await });
    b.received("FINISH").await;
    b.finish_reply.take().unwrap().send(()).unwrap();
    shutdown.await.unwrap().unwrap();
}

#[tokio::test]
async fn upstream_client_clones_share_owner_but_new_clients_are_independent() {
    use crate::upstream::client::UpstreamClient;
    let first = UpstreamClient::new(5);
    let clone = first.clone();
    let second = UpstreamClient::new(5);
    assert!(Arc::ptr_eq(&first.freebuff, &clone.freebuff));
    assert!(!Arc::ptr_eq(&first.freebuff, &second.freebuff));
    first.freebuff.shutdown(budget()).await.unwrap();
    assert!(clone.freebuff.ensure_open().is_err());
    assert!(second.freebuff.ensure_open().is_ok());
}

#[tokio::test]
async fn stale_initial_admission_cannot_insert_after_shutdown_clears_registry() {
    let owner = RunRuntime::default();
    owner.ensure_open().unwrap();
    // Model descheduling between acquisition's initial check and its registry insertion.
    owner.shutdown(budget()).await.unwrap();
    let error = owner
        .registry
        .get_or_insert_checked("late", || owner.ensure_open())
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("freebuff_run_admission_closed"));
    assert!(owner.registry.buckets().is_empty());
}

#[tokio::test]
async fn cached_lifetime_does_not_keep_runtime_owner_in_a_reference_cycle() {
    let mut f = Fixture::new().await;
    drop(f.ready_lease().await);
    let old = std::mem::replace(&mut f.owner, Arc::new(RunRuntime::default()));
    let weak = Arc::downgrade(&old);
    drop(old);
    assert!(weak.upgrade().is_none());
    f.received("FINISH").await;
    f.finish_reply.take().unwrap().send(()).unwrap();
    f.complete().await;
}
