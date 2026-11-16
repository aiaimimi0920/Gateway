//! Non-forwarding loopback peer; only explicit task-scoped current endpoints exist in tests.
use axum::{
    body::{Body, Bytes},
    extract::State,
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    Router,
};
use futures::Stream;
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    task::{Context, Poll},
    time::Duration,
};
tokio::task_local! { static ORIGIN: String; }
pub(crate) async fn within<T>(origin: &str, future: impl Future<Output = T>) -> T {
    let url = url::Url::parse(origin).unwrap();
    assert_eq!(url.host_str(), Some("127.0.0.1"));
    assert_eq!(url.scheme(), "http");
    assert!(url.port().is_some());
    crate::request_wait::transport::within(false, ORIGIN.scope(origin.into(), future)).await
}
pub(super) fn target(path: &str) -> String {
    assert!(matches!(
        path,
        "/api/v2/chats/new" | "/api/v2/chat/completions"
    ));
    ORIGIN
        .try_with(|origin| format!("{origin}{path}"))
        .expect("Qwen Web tests require explicit loopback scope")
}
#[derive(Clone)]
pub(crate) struct Call {
    pub uri: String,
    pub headers: axum::http::HeaderMap,
    pub body: Value,
}
#[derive(Clone)]
pub(crate) struct Reply {
    pub create_status: u16,
    pub create: Value,
    pub status: u16,
    pub content_type: &'static str,
    pub chunks: Vec<Bytes>,
    pub hold: bool,
}
impl Default for Reply {
    fn default() -> Self {
        Self { create_status:200, create:json!({"success":true,"data":{"id":"synthetic-chat"}}), status:200, content_type:"text/event-stream", chunks:vec![Bytes::from_static("data: {\"response.created\":{\"chat_id\":\"synthetic-chat\",\"response_id\":\"synthetic-response\"}}\n\ndata: {\"response_id\":\"synthetic-response\",\"choices\":[{\"delta\":{\"content\":\"合成答案\",\"phase\":\"answer\",\"status\":\"finished\"}}]}\n\n".as_bytes()), Bytes::from_static(b"data: {\"done\":true,\"usage\":{\"input_tokens\":2,\"output_tokens\":3}}\n\n")], hold:false }
    }
}
#[derive(Clone)]
struct Shared {
    reply: Reply,
    calls: Arc<Mutex<Vec<Call>>>,
    closed: Arc<AtomicUsize>,
}
pub(crate) struct Server {
    pub origin: String,
    shared: Shared,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl Drop for Server {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
impl Server {
    pub async fn start(reply: Reply) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let shared = Shared {
            reply,
            calls: Default::default(),
            closed: Default::default(),
        };
        let app = Router::new()
            .route("/api/v2/chats/new", axum::routing::post(create))
            .route("/api/v2/chat/completions", axum::routing::post(completion))
            .fallback(|| async { StatusCode::BAD_GATEWAY })
            .with_state(shared.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            origin,
            shared,
            task: Some(task),
        }
    }
    pub fn client(&self) -> rquest::Client {
        rquest::Client::builder()
            .emulation(rquest_util::Emulation::Chrome136)
            .no_proxy()
            .proxy(rquest::Proxy::all(&self.origin).unwrap())
            .timeout(Duration::from_secs(5))
            .retry(rquest::retry::Policy::never())
            .build()
            .unwrap()
    }
    pub fn calls(&self) -> Vec<Call> {
        self.shared.calls.lock().unwrap().clone()
    }
    pub async fn wait_closed(&self) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while self.shared.closed.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
    pub async fn finish(mut self) {
        if self.calls().len() == 2 {
            self.wait_closed().await;
        }
        let task = self.task.take().unwrap();
        task.abort();
        let _ = task.await;
    }
}
async fn record(shared: &Shared, request: Request<Body>) {
    let (parts, body) = request.into_parts();
    let bytes = axum::body::to_bytes(body, 128 * 1024).await.unwrap();
    shared.calls.lock().unwrap().push(Call {
        uri: parts.uri.path_and_query().unwrap().to_string(),
        headers: parts.headers,
        body: serde_json::from_slice(&bytes).unwrap(),
    });
}
async fn create(State(shared): State<Shared>, request: Request<Body>) -> Response {
    record(&shared, request).await;
    (
        StatusCode::from_u16(shared.reply.create_status).unwrap(),
        [("location", "https://example.invalid/private")],
        axum::Json(shared.reply.create),
    )
        .into_response()
}
struct Peer {
    chunks: VecDeque<Bytes>,
    hold: bool,
    closed: Arc<AtomicUsize>,
}
impl Stream for Peer {
    type Item = Result<Bytes, std::io::Error>;
    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(bytes) = self.chunks.pop_front() {
            Poll::Ready(Some(Ok(bytes)))
        } else if self.hold {
            Poll::Pending
        } else {
            Poll::Ready(None)
        }
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.closed.fetch_add(1, Ordering::SeqCst);
    }
}
async fn completion(State(shared): State<Shared>, request: Request<Body>) -> Response {
    record(&shared, request).await;
    let reply = shared.reply;
    let mut response = Response::new(Body::from_stream(Peer {
        chunks: reply.chunks.into(),
        hold: reply.hold,
        closed: shared.closed,
    }));
    *response.status_mut() = StatusCode::from_u16(reply.status).unwrap();
    response
        .headers_mut()
        .insert("content-type", reply.content_type.parse().unwrap());
    response.headers_mut().insert(
        "location",
        "https://example.invalid/private".parse().unwrap(),
    );
    response
}
pub(crate) fn payload() -> crate::routing::candidate::ProviderAccountPayload {
    serde_json::from_value(json!({"adapter":"qwen_web_compatible","base_url":"https://chat.qwen.ai/api/v1","api_key":"synthetic-exact","headers":{"Cookie":"token=synthetic-exact; cf_clearance=synthetic-clearance","User-Agent":"exact-UA"},"call_flow":{"special_adapter":"qwen_web","fingerprint_adapter":"session"}})).unwrap()
}
pub(crate) fn request() -> crate::protocol::canonical::CanonicalRelayRequest {
    crate::protocol::responses::normalize_responses(
        json!({"model":"public-alias","input":" 合成问题 "}),
    )
    .unwrap()
}
