use super::super::{FreeBuffRuntimeConfig, ManagedRun, RunLifetime};
use super::*;
use crate::protocol::freebuff::tests::make_payload;
use rquest::Client;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;

fn reserve(slots: &Arc<Semaphore>) -> Result<OwnedSemaphorePermit, GatewayError> {
    slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| GatewayError::service_unavailable("test capacity"))
}

fn insert(
    buckets: &DashMap<String, Arc<Mutex<FreeBuffRunBucket>>>,
    slots: &Arc<Semaphore>,
    key: &str,
    inflight: bool,
) -> Arc<Mutex<FreeBuffRunBucket>> {
    let config = FreeBuffRuntimeConfig::from_payload(&make_payload(), "z-ai/glm-5.1").unwrap();
    let lifetime = Arc::new(RunLifetime::tracked(
        Client::builder().build().unwrap(),
        config,
        key.into(),
        reserve(slots).unwrap(),
    ));
    if inflight {
        lifetime.acquire().unwrap();
    }
    // Admit existing leases before invalidation; skip remote FINISH in this registry-only fixture.
    lifetime.invalidate();
    let bucket = Arc::new(Mutex::new(FreeBuffRunBucket {
        active: Some(ManagedRun {
            run_id: key.into(),
            started_at: Instant::now(),
            lifetime,
        }),
        ..Default::default()
    }));
    buckets.insert(key.into(), bucket.clone());
    bucket
}

#[test]
fn full_capacity_reclaims_idle_owners_and_allows_new_admission() {
    let buckets = DashMap::new();
    let slots = Arc::new(Semaphore::new(2));
    let first = insert(&buckets, &slots, "first", false);
    let second = insert(&buckets, &slots, "second", false);
    let permit = reserve_with_reclamation(&buckets, || reserve(&slots))
        .expect("idle owners must not permanently exhaust admission");
    assert!(first.try_lock().unwrap().active.is_none());
    assert!(second.try_lock().unwrap().active.is_none());
    drop(permit);
    assert_eq!(slots.available_permits(), 2);
}

#[test]
fn available_capacity_does_not_evict_reusable_active_runs() {
    let buckets = DashMap::new();
    let slots = Arc::new(Semaphore::new(2));
    let active = insert(&buckets, &slots, "reusable", false);
    let _permit = reserve_with_reclamation(&buckets, || reserve(&slots)).unwrap();
    assert!(active.try_lock().unwrap().active.is_some());
}

#[test]
fn pressure_preserves_live_leases_and_never_waits_on_state_lock() {
    let buckets = DashMap::new();
    let slots = Arc::new(Semaphore::new(3));
    let live = insert(&buckets, &slots, "live", true);
    let busy = insert(&buckets, &slots, "busy", false);
    let idle = insert(&buckets, &slots, "idle", false);
    let guard = busy.try_lock().unwrap();
    let _permit = reserve_with_reclamation(&buckets, || reserve(&slots))
        .expect("an unlocked idle owner can be retired without waiting for busy owners");
    assert!(live.try_lock().unwrap().active.is_some());
    assert!(guard.active.is_some());
    assert!(idle.try_lock().unwrap().active.is_none());
    live.try_lock()
        .unwrap()
        .active
        .as_ref()
        .unwrap()
        .lifetime
        .release();
}

#[test]
fn rotation_retires_its_locked_stale_owner_before_reserving() {
    let buckets = DashMap::new();
    let slots = Arc::new(Semaphore::new(1));
    let bucket = insert(&buckets, &slots, "stale", false);
    let mut state = bucket.try_lock().unwrap();
    let _permit = reserve_for_rotation(&mut state, &buckets, || reserve(&slots)).unwrap();
    assert!(state.active.is_none());
}

#[test]
fn rotation_keeps_old_live_lease_capacity_until_last_owner_drops() {
    let buckets = DashMap::new();
    let slots = Arc::new(Semaphore::new(1));
    let bucket = insert(&buckets, &slots, "stale-live", true);
    let mut state = bucket.try_lock().unwrap();
    let old_lease_owner = state.active.as_ref().unwrap().lifetime.clone();
    assert!(reserve_for_rotation(&mut state, &buckets, || reserve(&slots)).is_err());
    assert!(state.active.is_none());
    assert_eq!(old_lease_owner.inflight(), 1);
    assert_eq!(slots.available_permits(), 0);
    old_lease_owner.release();
    drop(old_lease_owner);
    assert_eq!(slots.available_permits(), 1);
}

#[tokio::test]
async fn pressure_recovers_capacity_after_remote_finish_response() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;
    use tokio::time::timeout;

    struct Server(tokio::task::JoinHandle<()>);
    impl Drop for Server {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut payload = make_payload();
    payload.base_url = format!("http://{}", listener.local_addr().unwrap());
    let (sent, arrived) = oneshot::channel();
    let (respond, response) = oneshot::channel();
    let _server = Server(tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut bytes = [0; 4096];
        assert!(socket.read(&mut bytes).await.unwrap() > 0);
        sent.send(()).unwrap();
        response.await.unwrap();
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")
            .await
            .unwrap();
    }));
    let slots = Arc::new(Semaphore::new(1));
    let lifetime = Arc::new(RunLifetime::tracked(
        Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap(),
        FreeBuffRuntimeConfig::from_payload(&payload, "z-ai/glm-5.1").unwrap(),
        "pressure-run".into(),
        reserve(&slots).unwrap(),
    ));
    let buckets = DashMap::new();
    buckets.insert(
        "pressure".into(),
        Arc::new(Mutex::new(FreeBuffRunBucket {
            active: Some(ManagedRun {
                run_id: "pressure-run".into(),
                started_at: Instant::now(),
                lifetime,
            }),
            ..Default::default()
        })),
    );
    assert!(reserve_with_reclamation(&buckets, || reserve(&slots)).is_err());
    timeout(Duration::from_secs(1), arrived)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(slots.available_permits(), 0);
    respond.send(()).unwrap();
    let returned = timeout(Duration::from_secs(1), slots.clone().acquire_owned())
        .await
        .unwrap()
        .unwrap();
    drop(returned);
    assert!(reserve_with_reclamation(&buckets, || reserve(&slots)).is_ok());
}
