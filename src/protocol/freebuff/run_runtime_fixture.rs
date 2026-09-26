use super::*;
use crate::protocol::freebuff::{tests::make_payload, FreeBuffRuntimeConfig};
use rquest::Client;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};

pub(super) struct Fixture {
    pub owner: Arc<RunRuntime>,
    pub client: Client,
    pub config: FreeBuffRuntimeConfig,
    pub requests: mpsc::Receiver<serde_json::Value>,
    pub start_reply: Option<oneshot::Sender<()>>,
    pub finish_reply: Option<oneshot::Sender<()>>,
    server: tokio::task::JoinHandle<()>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}

async fn request(socket: &mut TcpStream) -> serde_json::Value {
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

impl Fixture {
    pub async fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (sent, requests) = mpsc::channel(2);
        let (start_reply, start_gate) = oneshot::channel();
        let (finish_reply, finish_gate) = oneshot::channel();
        let server = tokio::spawn(async move {
            for (gate, body) in [
                (start_gate, r#"{"runId":"runtime-run"}"#),
                (finish_gate, "{}"),
            ] {
                let (mut socket, _) = listener.accept().await.unwrap();
                sent.send(request(&mut socket).await).await.unwrap();
                gate.await.unwrap();
                let reply = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                socket.write_all(reply.as_bytes()).await.unwrap();
            }
        });
        let mut payload = make_payload();
        payload.base_url = format!("http://{address}");
        let mut config = FreeBuffRuntimeConfig::from_payload(&payload, "z-ai/glm-5.1").unwrap();
        config.bucket_key = "same-key-across-runtime-fixtures".into();
        Self {
            owner: Arc::new(RunRuntime::default()),
            client: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap(),
            config,
            requests,
            start_reply: Some(start_reply),
            finish_reply: Some(finish_reply),
            server,
        }
    }

    pub async fn received(&mut self, action: &str) {
        let body = tokio::time::timeout(Duration::from_secs(2), self.requests.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(body["action"], action);
        if action == "FINISH" {
            assert_eq!(body["runId"], "runtime-run");
        }
    }

    pub fn acquire(
        &self,
    ) -> tokio::task::JoinHandle<Result<super::super::super::FreeBuffRunLease, GatewayError>> {
        let (owner, client, config) =
            (self.owner.clone(), self.client.clone(), self.config.clone());
        tokio::spawn(async move { owner.acquire_run_lease(&client, &config).await })
    }

    pub async fn ready_lease(&mut self) -> super::super::super::FreeBuffRunLease {
        let caller = self.acquire();
        self.received("START").await;
        self.start_reply.take().unwrap().send(()).unwrap();
        caller.await.unwrap().unwrap()
    }

    pub async fn complete(&mut self) {
        tokio::time::timeout(Duration::from_secs(2), &mut self.server)
            .await
            .unwrap()
            .unwrap();
    }
}
