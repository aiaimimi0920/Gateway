use super::*;
use crate::protocol::freebuff::{run, tests::make_payload};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot, Semaphore};
use tokio::time::timeout;

struct Fixture {
    config: FreeBuffRuntimeConfig,
    client: Client,
    requests: mpsc::Receiver<serde_json::Value>,
    start_response: Option<oneshot::Sender<()>>,
    finish_response: Option<oneshot::Sender<()>>,
    server: tokio::task::JoinHandle<()>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
        run::run_buckets().remove(&self.config.bucket_key);
    }
}

async fn read_body(socket: &mut TcpStream) -> serde_json::Value {
    let mut bytes = Vec::new();
    loop {
        let mut chunk = [0; 1024];
        let n = socket.read(&mut chunk).await.unwrap();
        assert!(n > 0 && bytes.len() + n <= 16384);
        bytes.extend_from_slice(&chunk[..n]);
        let text = String::from_utf8_lossy(&bytes);
        if let Some(end) = text.find("\r\n\r\n") {
            let length = text[..end]
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap();
            if bytes.len() >= end + 4 + length {
                return serde_json::from_slice(&bytes[end + 4..end + 4 + length]).unwrap();
            }
        }
    }
}

async fn fixture() -> Fixture {
    fixture_with_body(r#"{"runId":"delayed-run"}"#).await
}

async fn fixture_with_body(start_body: &'static str) -> Fixture {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (sent, requests) = mpsc::channel(2);
    let (start_response, start_wait) = oneshot::channel();
    let (finish_response, finish_wait) = oneshot::channel();
    let server = tokio::spawn(async move {
        for (gate, body) in [(start_wait, start_body), (finish_wait, "{}")] {
            let (mut socket, _) = listener.accept().await.unwrap();
            sent.send(read_body(&mut socket).await).await.unwrap();
            let headers = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            socket.write_all(headers.as_bytes()).await.unwrap();
            gate.await.unwrap();
            socket.write_all(body.as_bytes()).await.unwrap();
        }
    });
    let mut payload = make_payload();
    payload.base_url = format!("http://{address}");
    payload.credential_id = Some(format!("start-test-{address}"));
    Fixture {
        config: FreeBuffRuntimeConfig::from_payload(&payload, "z-ai/glm-5.1").unwrap(),
        client: Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap(),
        requests,
        start_response: Some(start_response),
        finish_response: Some(finish_response),
        server,
    }
}

async fn received(fixture: &mut Fixture, action: &str) {
    let body = timeout(Duration::from_secs(2), fixture.requests.recv())
        .await
        .expect("expected run action did not arrive")
        .unwrap();
    assert_eq!(body["action"], action);
    if action == "FINISH" {
        assert_eq!(body["runId"], "delayed-run");
        assert_eq!(body["totalSteps"], 0);
    }
}

#[tokio::test]
async fn cancelled_start_keeps_slot_until_late_id_is_finished() {
    let mut fixture = fixture().await;
    let slots = Arc::new(Semaphore::new(1));
    let permit = slots.clone().try_acquire_owned().unwrap();
    let (client, config) = (fixture.client.clone(), fixture.config.clone());
    let caller = tokio::spawn(async move { start_tracked_run(&client, &config, permit).await });
    received(&mut fixture, "START").await;
    caller.abort();
    assert!(caller.await.unwrap_err().is_cancelled());
    assert_eq!(slots.available_permits(), 0);
    fixture.start_response.take().unwrap().send(()).unwrap();
    received(&mut fixture, "FINISH").await;
    assert_eq!(slots.available_permits(), 0);
    fixture.finish_response.take().unwrap().send(()).unwrap();
    let permit = timeout(Duration::from_secs(2), slots.clone().acquire_owned())
        .await
        .unwrap()
        .unwrap();
    drop(permit);
    assert_eq!(slots.available_permits(), 1);
}

async fn cancelled_caller_finishes_late_start(probe: bool) {
    let mut fixture = fixture().await;
    let (client, config) = (fixture.client.clone(), fixture.config.clone());
    let caller = tokio::spawn(async move {
        if probe {
            run::probe_run(&client, &config).await
        } else {
            run::acquire_run_lease(&client, &config).await.map(drop)
        }
    });
    received(&mut fixture, "START").await;
    caller.abort();
    assert!(caller.await.unwrap_err().is_cancelled());
    fixture.start_response.take().unwrap().send(()).unwrap();
    received(&mut fixture, "FINISH").await;
    fixture.finish_response.take().unwrap().send(()).unwrap();
    timeout(Duration::from_secs(2), &mut fixture.server)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn cancelled_probe_finishes_late_start() {
    cancelled_caller_finishes_late_start(true).await;
}

#[tokio::test]
async fn cancelled_acquisition_finishes_late_start() {
    cancelled_caller_finishes_late_start(false).await;
}

#[tokio::test]
async fn successful_start_transfers_slot_to_returned_lifetime() {
    let mut fixture = fixture().await;
    let slots = Arc::new(Semaphore::new(1));
    let permit = slots.clone().try_acquire_owned().unwrap();
    let (client, config) = (fixture.client.clone(), fixture.config.clone());
    let caller = tokio::spawn(async move { start_tracked_run(&client, &config, permit).await });
    received(&mut fixture, "START").await;
    fixture.start_response.take().unwrap().send(()).unwrap();
    let (id, lifetime) = caller.await.unwrap().unwrap();
    assert_eq!(id, "delayed-run");
    assert_eq!(slots.available_permits(), 0);
    assert!(fixture.requests.try_recv().is_err());
    let finish = tokio::spawn(lifetime.finish_now());
    received(&mut fixture, "FINISH").await;
    fixture.finish_response.take().unwrap().send(()).unwrap();
    finish.await.unwrap().unwrap();
    assert_eq!(slots.available_permits(), 1);
}

#[tokio::test]
async fn missing_start_id_preserves_error_and_returns_slot() {
    let mut fixture = fixture_with_body("{}").await;
    let slots = Arc::new(Semaphore::new(1));
    let permit = slots.clone().try_acquire_owned().unwrap();
    let (client, config) = (fixture.client.clone(), fixture.config.clone());
    let caller = tokio::spawn(async move { start_tracked_run(&client, &config, permit).await });
    received(&mut fixture, "START").await;
    fixture.start_response.take().unwrap().send(()).unwrap();
    let error = caller.await.unwrap().unwrap_err();
    assert_eq!(error.code.as_deref(), Some("freebuff_missing_run_id"));
    assert_eq!(slots.available_permits(), 1);
}

#[tokio::test]
async fn stalled_start_uses_real_supervisor_deadline_and_returns_slot() {
    let mut fixture = fixture().await;
    let slots = Arc::new(Semaphore::new(1));
    let permit = slots.clone().try_acquire_owned().unwrap();
    let config = fixture.config.clone();
    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .unwrap();
    let caller = tokio::spawn(async move { start_tracked_run(&client, &config, permit).await });
    received(&mut fixture, "START").await;
    assert_eq!(slots.available_permits(), 0);
    let error = timeout(Duration::from_secs(35), caller)
        .await
        .unwrap()
        .unwrap()
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("freebuff_start_timeout"));
    assert_eq!(slots.available_permits(), 1);
}

#[test]
fn missing_runtime_returns_explicit_error_and_slot_without_spawning() {
    use futures::FutureExt;
    let config = FreeBuffRuntimeConfig::from_payload(&make_payload(), "z-ai/glm-5.1").unwrap();
    let client = Client::builder().build().unwrap();
    let slots = Arc::new(Semaphore::new(1));
    let permit = slots.clone().try_acquire_owned().unwrap();
    let error = start_tracked_run(&client, &config, permit)
        .now_or_never()
        .unwrap()
        .unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("freebuff_start_runtime_unavailable")
    );
    assert_eq!(slots.available_permits(), 1);
}
