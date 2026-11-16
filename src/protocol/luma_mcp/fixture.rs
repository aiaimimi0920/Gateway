//! Sequential loopback peer; every test owns its server and joins it on normal completion.
use axum::{body::Body, http::HeaderMap, response::Response, routing::post, Router};
use bytes::Bytes;
use futures::{Stream, StreamExt};
use serde_json::Value;
use std::{
    collections::VecDeque,
    convert::Infallible,
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    task::{Context, Poll},
    time::Duration,
};

pub(super) struct Reply {
    pub status: u16,
    pub mime: &'static str,
    pub body: String,
    pub session: Option<&'static str>,
    pub location: Option<String>,
    pub hold: bool,
    pub delay: Duration,
}
impl Reply {
    pub fn json(value: Value) -> Self {
        Self {
            status: 200,
            mime: "application/json",
            body: value.to_string(),
            session: None,
            location: None,
            hold: false,
            delay: Duration::ZERO,
        }
    }
    pub fn initialized() -> Self {
        Self {
            status: 202,
            body: String::new(),
            ..Self::json(Value::Null)
        }
    }
}
pub(super) struct Peer {
    pub endpoint: String,
    pub calls: Arc<Mutex<Vec<(HeaderMap, Value)>>>,
    pub drops: Arc<AtomicUsize>,
    pub starts: Arc<AtomicUsize>,
    pub deletes: Arc<Mutex<Vec<HeaderMap>>>,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl Peer {
    pub async fn new(replies: Vec<Reply>) -> Self {
        Self::with_delete(replies, 204, Duration::ZERO).await
    }
    pub async fn with_delete(replies: Vec<Reply>, status: u16, delay: Duration) -> Self {
        Self::with_redirect(
            replies,
            status,
            delay,
            "http://127.0.0.1:1/must-not-follow".into(),
        )
        .await
    }
    pub async fn with_redirect(
        replies: Vec<Reply>,
        status: u16,
        delay: Duration,
        location: String,
    ) -> Self {
        let replies = Arc::new(Mutex::new(VecDeque::from(replies)));
        let calls = Arc::new(Mutex::new(Vec::new()));
        let seen = calls.clone();
        let drops = Arc::new(AtomicUsize::new(0));
        let dropped = drops.clone();
        let starts = Arc::new(AtomicUsize::new(0));
        let started = starts.clone();
        let deletes = Arc::new(Mutex::new(Vec::new()));
        let deleted = deletes.clone();
        let app = Router::new().route(
            "/mcp",
            post(move |headers: HeaderMap, body: String| {
                let reply = replies.lock().unwrap().pop_front();
                seen.lock()
                    .unwrap()
                    .push((headers, serde_json::from_str(&body).unwrap()));
                let drops = dropped.clone();
                let starts = started.clone();
                async move {
                    let Some(reply) = reply else {
                        return Response::builder().status(500).body(Body::empty()).unwrap();
                    };
                    tokio::time::sleep(reply.delay).await;
                    let mut response = Response::builder()
                        .status(reply.status)
                        .header("content-type", reply.mime);
                    if let Some(session) = reply.session {
                        response = response.header("mcp-session-id", session);
                    }
                    if let Some(location) = reply.location {
                        response = response.header("location", location);
                    }
                    let stream = futures::stream::iter([Ok(Bytes::from(reply.body))]);
                    let tail: Pin<Box<dyn Stream<Item = Result<Bytes, Infallible>> + Send>> =
                        if reply.hold {
                            Box::pin(futures::stream::pending())
                        } else {
                            Box::pin(futures::stream::empty())
                        };
                    response
                        .body(Body::from_stream(Tracked {
                            inner: Box::pin(stream.chain(tail)),
                            drops,
                            starts,
                            started: false,
                        }))
                        .unwrap()
                }
            })
            .delete(move |headers: HeaderMap| {
                deleted.lock().unwrap().push(headers);
                let location = location.clone();
                async move {
                    tokio::time::sleep(delay).await;
                    Response::builder()
                        .status(status)
                        .header("location", location)
                        .body(Body::empty())
                        .unwrap()
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/mcp", listener.local_addr().unwrap());
        let task = Some(tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        }));
        Self {
            endpoint,
            calls,
            drops,
            starts,
            deletes,
            task,
        }
    }
    pub fn count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
    pub async fn wait_calls(&self, count: usize) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while self.count() < count {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
    pub async fn wait_drops(&self, count: usize) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while self.drops.load(Ordering::SeqCst) < count {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
    pub async fn stop(mut self) {
        let task = self.task.take().unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
struct Tracked {
    inner: Pin<Box<dyn Stream<Item = Result<Bytes, Infallible>> + Send>>,
    drops: Arc<AtomicUsize>,
    starts: Arc<AtomicUsize>,
    started: bool,
}
impl Stream for Tracked {
    type Item = Result<Bytes, Infallible>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if !self.started {
            self.started = true;
            self.starts.fetch_add(1, Ordering::SeqCst);
        }
        self.inner.as_mut().poll_next(cx)
    }
}
impl Drop for Tracked {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
pub(super) fn http() -> rquest::Client {
    let mut headers = rquest::header::HeaderMap::new();
    headers.insert("authorization", "Bearer wrong-default".parse().unwrap());
    headers.insert("cookie", "private-default".parse().unwrap());
    headers.insert("x-api-key", "private-default".parse().unwrap());
    headers.insert("mcp-session-id", "stale-default-session".parse().unwrap());
    rquest::Client::builder()
        .no_proxy()
        .default_headers(headers)
        .build()
        .unwrap()
}
