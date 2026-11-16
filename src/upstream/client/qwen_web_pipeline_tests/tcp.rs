//! Real downstream sockets retain the existing exact-account and terminal contract.
use super::*;
use axum::middleware::{from_fn, Next};
use futures::StreamExt;
use tokio::{sync::oneshot, task::JoinHandle};

pub(super) struct Tcp {
    origin: String,
    client: rquest::Client,
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
}
impl Tcp {
    async fn start(fixture: &Fixture, upstream: &wire::Server) -> Self {
        Self::start_state(fixture.console.state.clone(), upstream).await
    }
    pub(super) async fn start_state(
        state: Arc<crate::state::AppState>,
        upstream: &wire::Server,
    ) -> Self {
        let origin = upstream.origin.clone();
        // Task locals do not cross axum's spawned connections. Only this test layer
        // binds the non-forwarding peer; production targets and routing are unchanged.
        let app = build_router(state).layer(from_fn(move |request: Request<Body>, next: Next| {
            let origin = origin.clone();
            async move { wire::within(&origin, next.run(request)).await }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await
                .unwrap();
        });
        Self {
            origin,
            client: rquest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap(),
            stop: Some(stop),
            task: Some(task),
        }
    }
    async fn request(&self, endpoint: &str, stream: bool) -> rquest::Response {
        self.request_token(endpoint, stream, "public-api-contract-secret")
            .await
    }
    pub(super) async fn request_token(
        &self,
        endpoint: &str,
        stream: bool,
        token: &str,
    ) -> rquest::Response {
        self.client
            .post(format!("{}/v1/{endpoint}", self.origin))
            .bearer_auth(token)
            .header("cookie", "foreign=caller-secret")
            .header("user-agent", "caller-UA")
            .json(&body(endpoint, stream))
            .send()
            .await
            .unwrap()
    }
    pub(super) async fn finish(mut self) {
        self.stop.take().unwrap().send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(3), self.task.as_mut().unwrap())
            .await
            .unwrap()
            .unwrap();
        self.task.take();
    }
}
impl Drop for Tcp {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}

async fn settled(fixture: &Fixture, upstream: &wire::Server, status: &str) {
    let rows = fixture.audits().await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, status);
    assert_eq!(rows[0].resolved_model.as_deref(), Some("qwen3.7-plus"));
    assert_eq!(
        rows[0].route_trace.as_ref().unwrap()["realCredentialRef"],
        "exact"
    );
    assert_wire(upstream);
    let serialized = serde_json::to_string(&rows).unwrap();
    for secret in ["synthetic-exact", "synthetic-clearance", "caller-secret"] {
        assert!(
            !serialized.contains(secret),
            "audit must not expose credential material"
        );
    }
}

#[test]
fn qwen_web_tcp_buffered_and_stream_success_preserve_usage_and_exact_account() {
    run(success);
}
async fn success() {
    for endpoint in ["chat/completions", "responses"] {
        for stream in [false, true] {
            for reported in [false, true] {
                let mut reply = wire::Reply::default();
                if !reported {
                    reply.chunks[1] = bytes::Bytes::from_static(b"data: {\"done\":true}\n\n");
                }
                let upstream = wire::Server::start(reply).await;
                let fixture = Fixture::new(&upstream).await;
                let tcp = Tcp::start(&fixture, &upstream).await;
                let response = tcp.request(endpoint, stream).await;
                assert_eq!(response.status().as_u16(), 200);
                let text = response.text().await.unwrap();
                assert!(text.contains("合成答案"), "{text}");
                if stream {
                    assert!(text.contains(if endpoint == "responses" {
                        "response.completed"
                    } else {
                        "[DONE]"
                    }));
                } else {
                    let value: Value = serde_json::from_str(&text).unwrap();
                    if reported {
                        assert_eq!(value["usage"]["total_tokens"], 5);
                    } else {
                        assert!(value["usage"].is_null());
                    }
                }
                settled(&fixture, &upstream, "completed").await;
                let rows = fixture.audits().await;
                assert_eq!(rows[0].total_tokens, reported.then_some(5));
                tcp.finish().await;
                fixture.finish().await;
                upstream.finish().await;
            }
        }
    }
}

#[test]
fn qwen_web_tcp_truncated_stream_never_reports_success_or_replays() {
    run(truncated);
}
async fn truncated() {
    for endpoint in ["chat/completions", "responses"] {
        for stream in [false, true] {
            let mut reply = wire::Reply::default();
            reply.chunks.pop();
            let upstream = wire::Server::start(reply).await;
            let fixture = Fixture::new(&upstream).await;
            let tcp = Tcp::start(&fixture, &upstream).await;
            let response = tcp.request(endpoint, stream).await;
            if !stream {
                assert_ne!(response.status().as_u16(), 200);
            }
            let mut chunks = response.bytes_stream();
            let mut received = Vec::new();
            while let Some(chunk) = chunks.next().await {
                match chunk {
                    Ok(bytes) => received.extend_from_slice(&bytes),
                    Err(_) if stream => break,
                    Err(error) => panic!("buffered error response was truncated: {error}"),
                }
                assert!(received.len() < 16384);
            }
            let text = String::from_utf8_lossy(&received);
            assert!(
                !text.contains("response.completed") && !text.contains("[DONE]"),
                "{text}"
            );
            drop(chunks);
            settled(&fixture, &upstream, "failed").await;
            tcp.finish().await;
            fixture.finish().await;
            upstream.finish().await;
        }
    }
}

#[test]
fn qwen_web_tcp_disconnect_after_increment_closes_upstream_and_settles_once() {
    run(disconnect);
}
async fn disconnect() {
    for endpoint in ["chat/completions", "responses"] {
        let mut reply = wire::Reply::default();
        reply.chunks.pop();
        reply.hold = true;
        let upstream = wire::Server::start(reply).await;
        let fixture = Fixture::new(&upstream).await;
        let tcp = Tcp::start(&fixture, &upstream).await;
        let response = tcp.request(endpoint, true).await;
        assert_eq!(response.status().as_u16(), 200);
        let mut chunks = response.bytes_stream();
        let mut seen = Vec::new();
        tokio::time::timeout(Duration::from_secs(2), async {
            while !String::from_utf8_lossy(&seen).contains("合成答案") {
                seen.extend_from_slice(&chunks.next().await.unwrap().unwrap());
                assert!(seen.len() < 16384);
            }
        })
        .await
        .unwrap();
        let text = String::from_utf8_lossy(&seen);
        assert!(!text.contains("response.completed") && !text.contains("[DONE]"));
        // The peer has no terminal event and stays open: seeing text proves
        // actual incremental delivery, not a buffered result flushed at EOF.
        drop(chunks);
        upstream.wait_closed().await;
        settled(&fixture, &upstream, "cancelled").await;
        tcp.finish().await;
        fixture.finish().await;
        upstream.finish().await;
    }
}
