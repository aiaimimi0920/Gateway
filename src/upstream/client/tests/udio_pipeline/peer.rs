//! Synthetic remote executor only: never forwards requests or serves real media.
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Request},
    response::Response,
    Router,
};
use serde_json::Value;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

#[derive(Clone)]
struct PeerState {
    result: Value,
    status: u16,
    hold: bool,
    calls: Arc<Mutex<Vec<(HeaderMap, Value)>>>,
    closed: Arc<AtomicUsize>,
    called: Arc<tokio::sync::Notify>,
}
pub struct Peer {
    pub origin: String,
    state: PeerState,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Peer {
    pub async fn start(result: Value, status: u16, hold: bool) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let state = PeerState {
            result,
            status,
            hold,
            calls: Arc::default(),
            closed: Arc::default(),
            called: Arc::default(),
        };
        let app = Router::new().fallback(handler).with_state(state.clone());
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self {
            origin,
            state,
            task,
        }
    }
    pub fn calls(&self) -> Vec<(HeaderMap, Value)> {
        self.state.calls.lock().unwrap().clone()
    }
    pub async fn wait_called(&self) {
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                let notified = self.state.called.notified();
                if !self.calls().is_empty() {
                    return;
                }
                notified.await;
            }
        })
        .await
        .expect("public request reaches the remote executor within its fixture budget");
    }
    pub async fn wait_closed(&self) {
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            while self.state.closed.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("dropping local caller closes remote response source");
    }
    pub async fn stop(mut self) {
        self.task.abort();
        let _ = (&mut self.task).await;
    }
}
async fn handler(State(state): State<PeerState>, request: Request<Body>) -> Response {
    assert_eq!(request.method(), "POST");
    assert_eq!(
        request.uri().path(),
        "/v1/internal/browser-executor/execute"
    );
    let (parts, body) = request.into_parts();
    let bytes = axum::body::to_bytes(body, 1024 * 1024).await.unwrap();
    state
        .calls
        .lock()
        .unwrap()
        .push((parts.headers, serde_json::from_slice(&bytes).unwrap()));
    state.called.notify_one();
    let body = if state.hold {
        Body::from_stream(futures::stream::unfold(
            CloseProbe(state.closed),
            |probe| async {
                std::future::pending::<()>().await;
                Some((Ok::<_, std::io::Error>(bytes::Bytes::new()), probe))
            },
        ))
    } else {
        Body::from(serde_json::to_vec(&state.result).unwrap())
    };
    Response::builder()
        .status(state.status)
        .header("content-type", "application/json")
        .body(body)
        .unwrap()
}
struct CloseProbe(Arc<AtomicUsize>);
impl Drop for CloseProbe {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
