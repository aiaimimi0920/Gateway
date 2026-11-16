//! Two real random-loopback servers; held responses expose explicit cancellation barriers.
use super::fixture;
use axum::{
    body::Body,
    extract::{OriginalUri, State},
    http::HeaderMap,
    routing::post,
    Json, Router,
};
use bytes::Bytes;
use futures::{stream, StreamExt};
use serde_json::Value;
use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{sync::oneshot, task::JoinHandle, time::timeout};

pub const DEADLINE: Duration = Duration::from_secs(10);
pub struct Server {
    pub origin: String,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}
impl Server {
    pub async fn start(app: Router) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .await
            .unwrap();
        });
        Self {
            origin,
            stop: Some(stop),
            task,
        }
    }
    pub async fn close(mut self) {
        let _ = self.stop.take().unwrap().send(());
        timeout(DEADLINE, &mut self.task).await.unwrap().unwrap();
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Complete,
    BeforeHeaders,
    BeforeBody,
    PartialFrame,
    Partial,
    Reasoning,
    ToolArguments,
    BeforeEof,
}
#[derive(Clone)]
pub struct Upstream {
    pub calls: Arc<Mutex<Vec<(String, Value)>>>,
    pub waiting: Arc<AtomicBool>,
    pub dropped: Arc<AtomicUsize>,
    pub mode: Mode,
}
impl Upstream {
    pub fn new(mode: Mode) -> Self {
        Self {
            mode,
            calls: Default::default(),
            waiting: Default::default(),
            dropped: Default::default(),
        }
    }
    pub fn router(&self) -> Router {
        Router::new()
            .route("/model/fixture-model/converse", post(respond))
            .route("/model/fixture-model/converse-stream", post(respond))
            .with_state(self.clone())
    }
    pub async fn wait(&self) {
        timeout(DEADLINE, async {
            while !self.waiting.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
    }
    pub async fn released(&self) {
        timeout(DEADLINE, async {
            while self.dropped.load(Ordering::SeqCst) != 1 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("upstream handler/body must be dropped after caller disconnect");
    }
}
struct Released(Arc<AtomicUsize>);
impl Drop for Released {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
async fn respond(
    State(state): State<Upstream>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> impl axum::response::IntoResponse {
    assert_eq!(headers["authorization"], "Bearer synthetic-only");
    assert!(headers.get("cookie").is_none());
    assert!(
        body.get("gateway_bedrock_history").is_none() && body.get("gateway_bedrock_turn").is_none()
    );
    assert_eq!(uri.path(), "/model/fixture-model/converse-stream");
    state.calls.lock().unwrap().push((uri.path().into(), body));
    let content_type = "application/vnd.amazon.eventstream";
    let bytes = fixture::sequence().concat();
    if state.mode == Mode::Complete {
        return ([("content-type", content_type)], Body::from(bytes));
    }
    let released = Released(state.dropped.clone());
    if state.mode == Mode::BeforeHeaders {
        state.waiting.store(true, Ordering::SeqCst);
        let _released = released;
        std::future::pending::<()>().await;
        unreachable!();
    }
    let prefix = match state.mode {
        Mode::BeforeBody => Vec::new(),
        Mode::PartialFrame => {
            let frames = fixture::sequence();
            let mut prefix = frames[0].clone();
            prefix.extend_from_slice(&frames[1][..frames[1].len() - 3]);
            prefix
        }
        Mode::Partial => fixture::sequence()[..2].concat(),
        Mode::Reasoning => fixture::sequence()[..4].concat(),
        Mode::ToolArguments => {
            let mut frames = fixture::sequence();
            frames[7] = super::wire::event(
                "contentBlockDelta",
                serde_json::json!({"contentBlockIndex":2,"delta":{"toolUse":{"input":"{\"v\":"}}}),
            );
            frames[..8].concat()
        }
        Mode::BeforeEof => bytes,
        _ => unreachable!(),
    };
    let stream = stream::once(async move { Ok::<_, std::io::Error>(Bytes::from(prefix)) }).chain(
        stream::once(async move {
            let _released = released;
            state.waiting.store(true, Ordering::SeqCst);
            std::future::pending::<Result<Bytes, std::io::Error>>().await
        }),
    );
    ([("content-type", content_type)], Body::from_stream(stream))
}
