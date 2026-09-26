use super::*;
use crate::protocol::freebuff::tests::make_payload;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::time::timeout;

struct Cleanup {
    key: String,
    server: tokio::task::JoinHandle<()>,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        self.server.abort();
        run_buckets().remove(&self.key);
    }
}

#[tokio::test]
async fn final_raw_lease_drop_finishes_retired_run_without_another_request() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (sent, mut requests) = mpsc::channel(4);
    let server = tokio::spawn(async move {
        for index in 0..3 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            loop {
                let mut chunk = [0u8; 1024];
                let n = socket.read(&mut chunk).await.unwrap();
                assert!(n > 0 && request.len() + n <= 8192);
                request.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&request);
                if let Some(end) = text.find("\r\n\r\n") {
                    let length = text[..end]
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if request.len() >= end + 4 + length {
                        sent.send(text[end + 4..].to_string()).await.unwrap();
                        break;
                    }
                }
            }
            let body = if index < 2 {
                format!("{{\"runId\":\"run-{index}\"}}")
            } else {
                "{}".into()
            };
            let reply = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(reply.as_bytes()).await.unwrap();
        }
    });
    let mut payload = make_payload();
    payload.base_url = format!("http://{address}");
    payload.credential_id = Some(format!("retirement-{address}"));
    let mut config = FreeBuffRuntimeConfig::from_payload(&payload, "z-ai/glm-5.1").unwrap();
    config.rotation_interval = Duration::ZERO;
    let _cleanup = Cleanup {
        key: config.bucket_key.clone(),
        server,
    };
    let client = Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    let first = acquire_run_lease(&client, &config).await.unwrap();
    let second = acquire_run_lease(&client, &config).await.unwrap();
    assert!(requests.recv().await.unwrap().contains("START"));
    assert!(requests.recv().await.unwrap().contains("START"));
    assert!(requests.try_recv().is_err());
    drop(first);
    let body = timeout(Duration::from_secs(1), requests.recv())
        .await
        .expect("retired run was not finished after its last lease dropped")
        .unwrap();
    let body: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(body["action"], "FINISH");
    assert_eq!(body["runId"], "run-0");
    assert_eq!(body["totalSteps"], 1);
    invalidate_run_lease(&config, second, "test cleanup").await;
}
