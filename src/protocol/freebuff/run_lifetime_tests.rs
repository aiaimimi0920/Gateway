use super::*;
use crate::protocol::freebuff::tests::make_payload;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

fn tracked(slots: &Arc<Semaphore>, base_url: String) -> RunLifetime {
    let mut payload = make_payload();
    payload.base_url = base_url;
    let config = FreeBuffRuntimeConfig::from_payload(&payload, "z-ai/glm-5.1").unwrap();
    RunLifetime::tracked(
        Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap(),
        config,
        "synthetic-run".into(),
        reserve_from(slots).unwrap(),
    )
}

#[test]
fn capacity_fails_closed_without_waiting_and_recovers_after_release() {
    let slots = Arc::new(Semaphore::new(MAX_TRACKED_RUNS as usize));
    let permits: Vec<_> = (0..MAX_TRACKED_RUNS)
        .map(|_| reserve_from(&slots).unwrap())
        .collect();
    assert_eq!(
        reserve_from(&slots).unwrap_err().code.as_deref(),
        Some("freebuff_run_capacity_exhausted")
    );
    drop(permits);
    assert_eq!(slots.available_permits(), MAX_TRACKED_RUNS as usize);
    assert!(reserve_from(&slots).is_ok());
}

#[test]
fn invalidated_lifetime_holds_slot_until_final_owner_without_spawning() {
    let slots = Arc::new(Semaphore::new(1));
    let lifetime = Arc::new(tracked(&slots, "http://127.0.0.1:1".into()));
    let lease_owner = lifetime.clone();
    lifetime.invalidate();
    drop(lifetime);
    assert!(lease_owner.is_invalidated());
    assert_eq!(slots.available_permits(), 0);
    drop(lease_owner);
    assert_eq!(slots.available_permits(), 1);
}

struct Server(tokio::task::JoinHandle<()>);

impl Drop for Server {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn finish_holds_slot_until_response(cancel_probe: bool) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (arrived, accepted) = oneshot::channel();
    let (respond, response) = oneshot::channel();
    let _server = Server(tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buffer = [0; 4096];
        assert!(socket.read(&mut buffer).await.unwrap() > 0);
        arrived.send(()).unwrap();
        response.await.unwrap();
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")
            .await
            .unwrap();
    }));
    let slots = Arc::new(Semaphore::new(1));
    let lifetime = tracked(&slots, format!("http://{address}"));
    let probe = if cancel_probe {
        Some(tokio::spawn(lifetime.finish_now()))
    } else {
        let owner = Arc::new(lifetime);
        let last_owner = owner.clone();
        drop(owner);
        assert_eq!(slots.available_permits(), 0);
        drop(last_owner);
        None
    };
    tokio::time::timeout(Duration::from_secs(1), accepted)
        .await
        .unwrap()
        .unwrap();
    if let Some(probe) = probe {
        probe.abort();
        assert!(probe.await.unwrap_err().is_cancelled());
    }
    assert_eq!(slots.available_permits(), 0);
    assert!(reserve_from(&slots).is_err());
    respond.send(()).unwrap();
    let permit = tokio::time::timeout(Duration::from_secs(1), slots.clone().acquire_owned())
        .await
        .unwrap()
        .unwrap();
    drop(permit);
    assert_eq!(slots.available_permits(), 1);
}

#[tokio::test]
async fn last_owner_drop_keeps_permit_in_finish_task_until_completion() {
    finish_holds_slot_until_response(false).await;
}

#[tokio::test]
async fn cancelled_probe_does_not_cancel_finish_or_release_capacity_early() {
    finish_holds_slot_until_response(true).await;
}

#[tokio::test]
async fn finish_http_failure_returns_error_and_releases_capacity() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let _server = Server(tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut bytes = [0; 4096];
        assert!(socket.read(&mut bytes).await.unwrap() > 0);
        socket
            .write_all(
                b"HTTP/1.1 503 Unavailable\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
            )
            .await
            .unwrap();
    }));
    let slots = Arc::new(Semaphore::new(1));
    let error = tracked(&slots, format!("http://{address}"))
        .finish_now()
        .await
        .unwrap_err();
    assert_eq!(error.provider_name.as_deref(), Some("freebuff_compatible"));
    assert_eq!(slots.available_permits(), 1);
}

#[test]
fn unavailable_runtime_reports_failure_and_returns_reserved_slot() {
    let slots = Arc::new(Semaphore::new(1));
    let mut lifetime = tracked(&slots, "http://127.0.0.1:1".into());
    let job = lifetime.finish.take().unwrap();
    let error = spawn_finish(job, 0).unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("freebuff_finish_runtime_unavailable")
    );
    assert_eq!(slots.available_permits(), 1);
}

#[tokio::test]
async fn finish_deadline_releases_capacity_and_closes_stalled_connection() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (sent, arrived) = oneshot::channel();
    let (disconnected, closed) = oneshot::channel();
    let _server = Server(tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut bytes = [0; 4096];
        let mut received = socket.read(&mut bytes).await.unwrap();
        assert!(received > 0);
        sent.send(()).unwrap();
        loop {
            match socket.read(&mut bytes).await {
                Ok(0) | Err(_) => break,
                Ok(count) => {
                    received += count;
                    assert!(received <= 16 * 1024);
                }
            }
        }
        disconnected.send(()).unwrap();
    }));
    let slots = Arc::new(Semaphore::new(1));
    let mut lifetime = tracked(&slots, format!("http://{address}"));
    // Exercise the actual production FINISH deadline, not the shorter fixture client timeout.
    lifetime.finish.as_mut().unwrap().client = Client::builder()
        .timeout(FINISH_TIMEOUT + Duration::from_secs(30))
        .build()
        .unwrap();
    let task = tokio::spawn(lifetime.finish_now());
    tokio::time::timeout(Duration::from_secs(2), arrived)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(slots.available_permits(), 0);
    let error = tokio::time::timeout(FINISH_TIMEOUT + Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("freebuff_finish_timeout"));
    assert_eq!(slots.available_permits(), 1);
    tokio::time::timeout(Duration::from_secs(2), closed)
        .await
        .unwrap()
        .unwrap();
}
