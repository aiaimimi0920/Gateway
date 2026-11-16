//! Scoped synthetic origin and non-forwarding proxy; no test can reach a real Grok session.
use axum::{
    body::Body,
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    Router,
};
use serde_json::{json, Value};
use std::{
    future::Future,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
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
    assert!(matches!(path, "/api/auth/session" | "/ws/mgw/"));
    ORIGIN
        .try_with(|origin| format!("{origin}{path}"))
        .expect("Grok Web tests require explicit loopback scope")
}
#[derive(Clone, Debug)]
pub(crate) struct Call {
    pub uri: String,
    pub headers: axum::http::HeaderMap,
}
#[derive(Clone)]
pub(crate) struct Reply {
    pub frames: Vec<String>,
    pub identity: Value,
    pub identity_status: u16,
    pub handshake_status: u16,
    pub attached_first: bool,
}
impl Default for Reply {
    fn default() -> Self {
        Self { frames: vec![
            json!({"session_id":"synthetic-conversation","event":{"type":"response.output_text.delta","delta":"合成答案"}}).to_string(),
            json!({"session_id":"synthetic-conversation","event":{"type":"response.done","response":{"id":"synthetic-response","status":"completed"}}}).to_string(),
        ], identity: json!({"status":"authenticated","session":{"userId":"00000000-0000-4000-8000-000000000001"}}),
            identity_status:200, handshake_status:101, attached_first:false }
    }
}
#[derive(Clone)]
struct Shared {
    reply: Reply,
    calls: Arc<Mutex<Vec<Call>>>,
    events: Arc<Mutex<Vec<Value>>>,
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
            events: Default::default(),
            closed: Default::default(),
        };
        let app = Router::new()
            .route("/api/auth/session", axum::routing::get(identity))
            .route("/ws/mgw/", axum::routing::get(upgrade))
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
    pub fn events(&self) -> Vec<Value> {
        self.shared.events.lock().unwrap().clone()
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
        if self.shared.reply.handshake_status == 101 && self.calls().len() == 2 {
            self.wait_closed().await;
        }
        let task = self.task.take().unwrap();
        task.abort();
        let _ = task.await;
    }
}
async fn identity(State(shared): State<Shared>, request: Request<Body>) -> Response {
    shared.calls.lock().unwrap().push(Call {
        uri: request.uri().to_string(),
        headers: request.headers().clone(),
    });
    (
        StatusCode::from_u16(shared.reply.identity_status).unwrap(),
        axum::Json(shared.reply.identity),
    )
        .into_response()
}
async fn upgrade(
    State(shared): State<Shared>,
    ws: WebSocketUpgrade,
    request: Request<Body>,
) -> Response {
    shared.calls.lock().unwrap().push(Call {
        uri: request.uri().to_string(),
        headers: request.headers().clone(),
    });
    if shared.reply.handshake_status != 101 {
        return (
            StatusCode::from_u16(shared.reply.handshake_status).unwrap(),
            [("location", "https://example.invalid/private")],
            "synthetic-private-error",
        )
            .into_response();
    }
    ws.on_upgrade(move |socket| peer(socket, shared))
        .into_response()
}
struct Closed(Arc<AtomicUsize>);
impl Drop for Closed {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
async fn peer(mut socket: WebSocket, shared: Shared) {
    let _closed = Closed(shared.closed.clone());
    let Some(Ok(Message::Text(initial))) = socket.recv().await else {
        return;
    };
    let initial: Value = serde_json::from_str(&initial).unwrap();
    let initial_id = initial["event"]["event_id"].clone();
    shared.events.lock().unwrap().push(initial);
    let created = json!({"session_id":"synthetic-conversation","event":{"type":"session.created","client_event_id":initial_id}}).to_string();
    let attached = json!({"session_id":"synthetic-conversation","event":{"type":"conversation.attached","conversation":{"id":"synthetic-conversation"}}}).to_string();
    let events = if shared.reply.attached_first {
        [attached, created]
    } else {
        [created, attached]
    };
    for event in events {
        if socket.send(Message::Text(event.into())).await.is_err() {
            return;
        }
    }
    for _ in 0..2 {
        let Some(Ok(Message::Text(event))) = socket.recv().await else {
            return;
        };
        shared
            .events
            .lock()
            .unwrap()
            .push(serde_json::from_str(&event).unwrap());
    }
    for event in &shared.reply.frames {
        if event == "<close>" {
            return;
        }
        if socket
            .send(Message::Text(event.clone().into()))
            .await
            .is_err()
        {
            return;
        }
    }
    while socket
        .recv()
        .await
        .is_some_and(|value| matches!(value, Ok(Message::Ping(_) | Message::Pong(_))))
    {}
}
pub(crate) fn payload() -> crate::routing::candidate::ProviderAccountPayload {
    serde_json::from_value(json!({"adapter":"grok_compatible","base_url":"https://grok.com","api_key":"synthetic-exact",
        "session_auth":{"transport":"cookie","primaryCookieName":"sso","secondaryCookieName":"sso-rw"},
        "headers":{"Cookie":"sso=synthetic-exact; sso-rw=synthetic-exact; cf_clearance=synthetic-clearance","User-Agent":"exact-UA"},
        "call_flow":{"special_adapter":"grok_web","fingerprint_adapter":"session"}})).unwrap()
}
pub(crate) fn request() -> crate::protocol::canonical::CanonicalRelayRequest {
    crate::protocol::responses::normalize_responses(
        json!({"model":"public-alias","input":"合成问题"}),
    )
    .unwrap()
}
