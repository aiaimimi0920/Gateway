//! Random loopback peer with bounded inputs/hits and joined graceful shutdown; no external provider calls.
use axum::{
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, Method, Uri},
    response::Response,
    Router,
};
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
pub(super) struct Spec {
    pub status: u16,
    pub body: Vec<u8>,
    pub headers: HeaderMap,
    pub header_delay: Duration,
    pub body_delay: Duration,
    pub eof_delay: Duration,
    eof_gate: Option<tokio::sync::oneshot::Receiver<()>>,
}
impl Spec {
    pub fn json(status: u16, body: Value) -> Self {
        Self {
            status,
            body: serde_json::to_vec(&body).unwrap(),
            headers: HeaderMap::new(),
            header_delay: Duration::ZERO,
            body_delay: Duration::ZERO,
            eof_delay: Duration::ZERO,
            eof_gate: None,
        }
    }
    pub fn catalog() -> Self {
        Self::json(200, super::tests::body())
    }
    pub fn hold_eof(&mut self) -> tokio::sync::oneshot::Sender<()> {
        let (release, gate) = tokio::sync::oneshot::channel();
        self.eof_gate = Some(gate);
        release
    }
}
pub(super) struct Hit {
    pub method: Method,
    pub uri: Uri,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}
#[derive(Clone, Default)]
struct Data {
    replies: Arc<Mutex<VecDeque<Spec>>>,
    hits: Arc<Mutex<Vec<Hit>>>,
    drops: Arc<AtomicUsize>,
}
async fn respond(
    State(data): State<Data>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    {
        let mut hits = data.hits.lock().unwrap();
        assert!(hits.len() < 32, "bounded synthetic calls");
        hits.push(Hit {
            method,
            uri,
            headers,
            body: body.to_vec(),
        });
    }
    let spec = data
        .replies
        .lock()
        .unwrap()
        .pop_front()
        .unwrap_or_else(|| Spec::json(500, serde_json::json!({"unexpected":true})));
    tokio::time::sleep(spec.header_delay).await;
    let mut response = Response::builder().status(spec.status);
    if !spec.headers.contains_key("content-type") {
        response = response.header("content-type", "application/json");
    }
    for (name, value) in spec.headers {
        if let Some(name) = name {
            response = response.header(name, value);
        }
    }
    response
        .body(Body::from_stream(TrackedBody {
            delay: Box::pin(tokio::time::sleep(spec.body_delay)),
            bytes: Some(spec.body),
            eof_delay: spec.eof_delay,
            eof: None,
            eof_gate: spec.eof_gate,
            drops: data.drops,
        }))
        .unwrap()
}
pub(super) struct Peer {
    pub origin: String,
    pub hits: Arc<Mutex<Vec<Hit>>>,
    data: Data,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
}
impl Peer {
    pub async fn start(replies: Vec<Spec>) -> Self {
        let data = Data::default();
        data.replies.lock().unwrap().extend(replies);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let app = Router::new()
            .fallback(respond)
            .layer(DefaultBodyLimit::max(32_768))
            .with_state(data.clone());
        let (stop, stopped) = tokio::sync::oneshot::channel();
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
            hits: data.hits.clone(),
            data,
            stop: Some(stop),
            task,
        }
    }
    pub fn count(&self) -> usize {
        self.hits.lock().unwrap().len()
    }
    pub fn push(&self, spec: Spec) {
        let mut replies = self.data.replies.lock().unwrap();
        assert!(replies.len() < 32, "bounded synthetic replies");
        replies.push_back(spec);
    }
    pub async fn wait_drops(&self, count: usize) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while self.data.drops.load(Ordering::SeqCst) < count {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
    pub async fn close(mut self) {
        self.stop.take().unwrap().send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(3), &mut self.task)
            .await
            .unwrap()
            .unwrap();
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
struct TrackedBody {
    delay: Pin<Box<tokio::time::Sleep>>,
    bytes: Option<Vec<u8>>,
    eof_delay: Duration,
    eof: Option<Pin<Box<tokio::time::Sleep>>>,
    eof_gate: Option<tokio::sync::oneshot::Receiver<()>>,
    drops: Arc<AtomicUsize>,
}
impl futures::Stream for TrackedBody {
    type Item = Result<Bytes, Infallible>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        use std::future::Future;
        let this = self.get_mut();
        if this.delay.as_mut().poll(cx).is_pending() {
            return Poll::Pending;
        }
        if let Some(bytes) = this.bytes.take() {
            if !this.eof_delay.is_zero() {
                this.eof = Some(Box::pin(tokio::time::sleep(this.eof_delay)));
            }
            return Poll::Ready(Some(Ok(bytes.into())));
        }
        if this
            .eof
            .as_mut()
            .is_some_and(|delay| delay.as_mut().poll(cx).is_pending())
        {
            return Poll::Pending;
        }
        if let Some(gate) = &mut this.eof_gate {
            if Pin::new(gate).poll(cx).is_pending() {
                return Poll::Pending;
            }
            this.eof_gate = None;
        }
        Poll::Ready(None)
    }
}
impl Drop for TrackedBody {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
