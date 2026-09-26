use super::super::tests::make_payload;
use super::*;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::timeout;

struct Cleanup {
    key: String,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
        session_buckets().remove(&self.key);
    }
}

fn setup(name: &str) -> (FreeBuffRuntimeConfig, Client, Cleanup) {
    let mut config = FreeBuffRuntimeConfig::from_payload(&make_payload(), "z-ai/glm-5.1").unwrap();
    config.session_bucket_key = format!("poll-test-{name}");
    config.session_poll_timeout = Duration::from_millis(50);
    config.session_poll_interval = Duration::from_secs(5);
    let cleanup = Cleanup {
        key: config.session_bucket_key.clone(),
        task: None,
    };
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    (config, client, cleanup)
}

async fn within_budget(client: &Client, config: &FreeBuffRuntimeConfig) -> GatewayError {
    timeout(
        Duration::from_millis(500),
        ensure_free_session(client, config),
    )
    .await
    .expect("session poll exceeded its deadline")
    .unwrap_err()
}

#[tokio::test]
async fn cached_queue_sleep_respects_remaining_poll_budget() {
    let (config, client, _cleanup) = setup("cached");
    let snapshot = build_synthetic_session_snapshot(
        FreeBuffWaitingRoomRejection::WaitingRoomQueued,
        Some("instance"),
        "{}",
        &config,
    );
    session_bucket(&config.session_bucket_key)
        .unwrap()
        .lock()
        .await
        .set_snapshot(Some(snapshot));
    let error = within_budget(&client, &config).await;
    assert_eq!(error.code.as_deref(), Some("waiting_room_queued"));
}

#[tokio::test]
async fn lock_wait_is_included_in_poll_budget() {
    let (config, client, _cleanup) = setup("locked");
    let bucket = session_bucket(&config.session_bucket_key).unwrap();
    let guard = bucket.lock().await;
    let error = within_budget(&client, &config).await;
    assert_eq!(error.code.as_deref(), Some("freebuff_session_unavailable"));
    drop(guard);
}

async fn server(config: &mut FreeBuffRuntimeConfig, cleanup: &mut Cleanup, queued: bool) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    config.base_url = format!("http://{}", listener.local_addr().unwrap());
    cleanup.task = Some(tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0u8; 4096];
        let _ = socket.read(&mut request).await.unwrap();
        if queued {
            let body = r#"{"status":"queued","instanceId":"instance","estimatedWaitMs":5000}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        } else {
            std::future::pending::<()>().await;
        }
    }));
}

#[tokio::test]
async fn new_queue_sleep_respects_remaining_poll_budget() {
    let (mut config, client, mut cleanup) = setup("new");
    server(&mut config, &mut cleanup, true).await;
    let error = within_budget(&client, &config).await;
    assert_eq!(error.code.as_deref(), Some("waiting_room_queued"));
}

#[tokio::test]
async fn stalled_upstream_is_cancelled_at_poll_deadline_and_unlocks_bucket() {
    let (mut config, client, mut cleanup) = setup("stalled");
    server(&mut config, &mut cleanup, false).await;
    let error = within_budget(&client, &config).await;
    assert_eq!(error.code.as_deref(), Some("freebuff_session_unavailable"));
    let bucket = session_bucket(&config.session_bucket_key).unwrap();
    assert!(bucket.try_lock().is_ok());
}

#[tokio::test]
async fn cached_session_observation_preserves_real_id_and_rejects_stale_response() {
    let (config, client, _cleanup) = setup("observation");
    let bucket = session_bucket(&config.session_bucket_key).unwrap();
    let snapshot = parse_free_session_snapshot(
        &serde_json::json!({"status":"active", "instanceId":"real-id", "remainingMs":60000}),
        &config,
    )
    .unwrap();
    bucket.lock().await.set_snapshot(Some(snapshot.clone()));
    let observation = ensure_free_session(&client, &config).await.unwrap();
    assert_eq!(observation.instance_id(), Some("real-id"));
    bucket.lock().await.set_snapshot(Some(snapshot));
    record_waiting_room_rejection(
        &config,
        &observation,
        FreeBuffWaitingRoomRejection::SessionExpired,
        "{}",
    )
    .await;
    assert_eq!(
        bucket.lock().await.snapshot.as_ref().unwrap().state,
        FreeBuffSessionState::Active
    );
}

#[tokio::test]
async fn non_free_request_and_unobserved_rejection_do_not_wait_for_session_lock() {
    let (mut config, client, _cleanup) = setup("non-free-locked");
    config.cost_mode = "paid".into();
    let bucket = session_bucket(&config.session_bucket_key).unwrap();
    let guard = bucket.lock().await;
    let observation = timeout(
        Duration::from_millis(500),
        ensure_free_session(&client, &config),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(observation.instance_id().is_none());
    timeout(
        Duration::from_millis(500),
        record_waiting_room_rejection(
            &config,
            &observation,
            FreeBuffWaitingRoomRejection::MissingInstance,
            "{}",
        ),
    )
    .await
    .unwrap();
    assert!(guard.snapshot.is_none());
}
