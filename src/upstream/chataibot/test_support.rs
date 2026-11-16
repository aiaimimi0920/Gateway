//! Synthetic loopback HTTP only; request bodies are retained to check actual multipart bytes.
use axum::{body::Body, extract::State, http::Request, response::Response, Router};
use serde_json::Value;
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

pub const ASSET: &str = "https://assets.example.test/image.png?signature=synthetic-private";
tokio::task_local! { static ORIGIN: String; }
pub async fn within<T>(origin: &str, future: impl std::future::Future<Output = T>) -> T {
    assert!(origin.starts_with("http://127.0.0.1:"));
    super::super::public_assets::within(origin, ORIGIN.scope(origin.to_owned(), future)).await
}
pub(super) fn target(url: &str) -> String {
    if let Some(path) = url.strip_prefix("https://chataibot.pro/") {
        ORIGIN
            .try_with(|origin| format!("{origin}/{path}"))
            .expect("explicit fixture scope")
    } else {
        assert!(url.starts_with("http://127.0.0.1:"));
        url.to_owned()
    }
}
#[derive(Clone, Debug)]
pub struct Call {
    pub method: String,
    pub path: String,
    pub headers: axum::http::HeaderMap,
    pub bytes: Vec<u8>,
}
pub struct Reply {
    pub status: u16,
    pub mime: &'static str,
    pub bytes: Vec<u8>,
    pub hold: bool,
}
impl Reply {
    pub fn json(value: Value) -> Self {
        Self {
            status: 200,
            mime: "application/json",
            bytes: value.to_string().into_bytes(),
            hold: false,
        }
    }
    pub fn png() -> Self {
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(1, 1)
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        Self {
            status: 200,
            mime: "image/png",
            bytes: out.into_inner(),
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
    pub async fn stop(mut self) {
        self.task.abort();
        let _ = (&mut self.task).await;
    }
    pub async fn wait_calls(&self, count: usize) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while self.calls().len() < count {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("expected fixture request");
    }
    pub async fn wait_closed(&self) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while self.state.closed.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("cancelled response owner must close its source");
    }
}
async fn handler(State(state): State<Fixture>, request: Request<Body>) -> Response {
    let (parts, body) = request.into_parts();
    assert_ne!(
        parts.method,
        axum::http::Method::CONNECT,
        "fixture never forwards"
    );
    let bytes = axum::body::to_bytes(body, 20 * 1024 * 1024)
        .await
        .unwrap()
        .to_vec();
    state.calls.lock().unwrap().push(Call {
        method: parts.method.to_string(),
        path: parts.uri.path_and_query().unwrap().to_string(),
        headers: parts.headers,
        bytes,
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
    serde_json::from_value(serde_json::json!({"adapter":"chataibot_compatible","base_url":origin,
        "api_key":"chataibot-synthetic-token","headers":{"Cookie":"token=chataibot-synthetic-token; cf_clearance=synthetic"},
        "session_auth":{"transport":"cookie","primary_cookie_name":"token"}})).unwrap()
}
pub fn request(body: Value) -> crate::protocol::canonical::CanonicalRelayRequest {
    crate::protocol::gemini_business::normalize_image_generations(body).unwrap()
}
