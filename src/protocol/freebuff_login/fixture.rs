//! Synthetic random loopback peer; every server is aborted and joined, bodies track client cancellation.
use super::{LoginChallenge, LoginTransport, TEST_ORIGIN};
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Method, Uri},
    response::Response,
    routing::{get, post},
    Router,
};
use serde_json::{json, Value};
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

pub(crate) struct Spec {
    pub status: u16,
    pub body: Vec<u8>,
    pub headers: HeaderMap,
    pub header_delay: Duration,
    pub body_delay: Duration,
}
impl Spec {
    pub fn json(status: u16, body: Value) -> Self {
        Self {
            status,
            body: body.to_string().into_bytes(),
            headers: HeaderMap::new(),
            header_delay: Duration::ZERO,
            body_delay: Duration::ZERO,
        }
    }
}
pub(crate) fn code() -> Spec {
    Spec::json(200, super::tests::code())
}
pub(crate) fn user() -> Spec {
    Spec::json(
        200,
        json!({"user":{"id":"synthetic-user", "email":"synthetic@example.invalid",
        "name":null, "authToken":"synthetic-token", "fingerprintId":"enhanced-synthetic-device",
        "fingerprintHash":"synthetic-hash+/="}}),
    )
}
pub(crate) type Calls = Arc<Mutex<Vec<(Method, Uri, HeaderMap, Vec<u8>)>>>;
#[derive(Clone)]
struct ServerState {
    replies: Arc<Mutex<VecDeque<Spec>>>,
    calls: Calls,
    drops: Arc<AtomicUsize>,
}
async fn handle(
    State(state): State<ServerState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: bytes::Bytes,
) -> Response {
    state
        .calls
        .lock()
        .unwrap()
        .push((method, uri, headers, body.to_vec()));
    let spec = state
        .replies
        .lock()
        .unwrap()
        .pop_front()
        .expect("unexpected extra FreeBuff login request");
    tokio::time::sleep(spec.header_delay).await;
    let mut response = Response::builder()
        .status(spec.status)
        .header("content-type", "application/json");
    for (name, value) in spec.headers {
        if let Some(name) = name {
            response = response.header(name, value);
        }
    }
    response
        .body(Body::from_stream(TrackedBody {
            sleep: Box::pin(tokio::time::sleep(spec.body_delay)),
            body: Some(spec.body),
            drops: state.drops,
        }))
        .unwrap()
}
pub(crate) struct Mock {
    pub origin: String,
    pub calls: Calls,
    pub drops: Arc<AtomicUsize>,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl Mock {
    pub async fn new(replies: Vec<Spec>) -> Self {
        let calls = Calls::default();
        let drops = Arc::new(AtomicUsize::new(0));
        let state = ServerState {
            replies: Arc::new(Mutex::new(replies.into())),
            calls: calls.clone(),
            drops: drops.clone(),
        };
        let router = Router::new()
            .route("/api/auth/cli/code", post(handle))
            .route("/api/auth/cli/status", get(handle))
            .with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let task = Some(tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        }));
        Self {
            origin,
            calls,
            drops,
            task,
        }
    }
    pub fn count(&self) -> usize {
        self.calls.lock().unwrap().len()
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
    pub async fn begin(&self, http: &LoginTransport) -> LoginChallenge {
        TEST_ORIGIN
            .scope(
                self.origin.clone(),
                LoginChallenge::start(http, "enhanced-synthetic-device"),
            )
            .await
            .unwrap()
    }
    pub async fn stop(mut self) {
        let task = self.task.take().unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
    }
}
impl Drop for Mock {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
struct TrackedBody {
    sleep: Pin<Box<tokio::time::Sleep>>,
    body: Option<Vec<u8>>,
    drops: Arc<AtomicUsize>,
}
impl futures::Stream for TrackedBody {
    type Item = Result<bytes::Bytes, Infallible>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        use std::future::Future;
        let this = self.get_mut();
        if this.sleep.as_mut().poll(cx).is_pending() {
            return Poll::Pending;
        }
        Poll::Ready(this.body.take().map(|body| Ok(body.into())))
    }
}
impl Drop for TrackedBody {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
