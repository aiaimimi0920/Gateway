//! Loopback only, no forwarding; the server task owns and drops every response stream.
use axum::{body::Body, extract::State, http::Request, response::Response, Router};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

#[derive(Clone, Debug)]
pub struct Call {
    pub method: axum::http::Method,
    pub path: String,
    pub headers: axum::http::HeaderMap,
    pub body: Value,
}
pub struct Reply {
    pub status: u16,
    pub mime: &'static str,
    pub bytes: Vec<u8>,
    pub hold: bool,
}
impl Reply {
    pub fn audio() -> Self {
        Self {
            status: 200,
            mime: "audio/mpeg",
            bytes: include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/audio/synthetic-tone.mp3"
            ))
            .to_vec(),
            hold: false,
        }
    }
    pub fn json(body: Value) -> Self {
        Self {
            status: 200,
            mime: "application/json",
            bytes: body.to_string().into_bytes(),
            hold: false,
        }
    }
    pub fn status(status: u16, body: &str) -> Self {
        Self {
            status,
            mime: "text/plain",
            bytes: body.as_bytes().to_vec(),
            hold: false,
        }
    }
}
#[derive(Clone)]
struct Fixture {
    replies: Arc<Mutex<VecDeque<Reply>>>,
    calls: Arc<Mutex<Vec<Call>>>,
    closed: Arc<AtomicUsize>,
}
pub struct Server {
    pub origin: String,
    state: Fixture,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Server {
    pub async fn start(replies: Vec<Reply>) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let state = Fixture {
            replies: Arc::new(Mutex::new(replies.into())),
            calls: Arc::default(),
            closed: Arc::default(),
        };
        let app = Router::new().fallback(handler).with_state(state.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            origin,
            state,
            task,
        }
    }
    pub fn calls(&self) -> Vec<Call> {
        self.state.calls.lock().unwrap().clone()
    }
    pub fn paths(&self) -> Vec<String> {
        self.calls().into_iter().map(|c| c.path).collect()
    }
    pub async fn wait_calls(&self, count: usize) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while self.calls().len() < count {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("expected loopback request");
    }
    pub async fn wait_closed(&self) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while self.state.closed.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("in-flight response owner must close on cancellation/timeout");
    }
    pub async fn stop(mut self) {
        self.task.abort();
        let _ = (&mut self.task).await;
    }
}
async fn handler(State(state): State<Fixture>, request: Request<Body>) -> Response {
    let (parts, body) = request.into_parts();
    let bytes = axum::body::to_bytes(body, 256 * 1024).await.unwrap();
    assert!(matches!(
        parts.method,
        axum::http::Method::POST | axum::http::Method::GET
    ));
    state.calls.lock().unwrap().push(Call {
        method: parts.method,
        path: parts.uri.path_and_query().unwrap().to_string(),
        headers: parts.headers,
        body: if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        },
    });
    let reply = state
        .replies
        .lock()
        .unwrap()
        .pop_front()
        .expect("unexpected extra send");
    let body = if reply.hold {
        Body::from_stream(futures::stream::unfold(
            CloseProbe(state.closed.clone()),
            |probe| async move {
                std::future::pending::<()>().await;
                Some((Ok::<_, std::io::Error>(bytes::Bytes::new()), probe))
            },
        ))
    } else {
        let chunks: Vec<_> = reply
            .bytes
            .chunks(4096)
            .map(|b| Ok::<_, std::io::Error>(bytes::Bytes::copy_from_slice(b)))
            .collect();
        Body::from_stream(futures::stream::iter(chunks))
    };
    Response::builder()
        .status(reply.status)
        .header("content-type", reply.mime)
        .header("location", "/must-not-follow")
        .body(body)
        .unwrap()
}
struct CloseProbe(Arc<AtomicUsize>);
impl Drop for CloseProbe {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
pub fn payload(origin: &str) -> crate::routing::candidate::ProviderAccountPayload {
    serde_json::from_value(json!({"adapter":"suno_compatible","base_url":origin,"api_key":"synthetic-clerk",
        "session_auth":{"transport":"bearer"},"headers":{"Cookie":"__session=synthetic-session","device-id":"synthetic-device"}})).unwrap()
}
pub fn request(options: Value) -> crate::protocol::canonical::CanonicalRelayRequest {
    let mut body = json!({"prompt":"synthetic music","model":"chirp-v3-5","pollIntervalMs":1000});
    body.as_object_mut()
        .unwrap()
        .extend(options.as_object().unwrap().clone());
    crate::protocol::suno::normalize_music_generations(body).unwrap()
}
pub fn clip(id: &str, status: &str) -> Value {
    json!({"id":id,"status":status,"audio_url":"https://assets.example.test/track.mp3"})
}
