//! Non-forwarding loopback fixtures; every remap is explicit and task-local.
use axum::{body::Body, extract::State, http::Request, response::Response, Router};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    future::Future,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

pub const ID: &str = "d290f1ee-6c54-4b01-90e6-d701748f0851";
pub const ASSET: &str = "https://assets.example.test/output.png?signature=private";
tokio::task_local! { static ORIGIN: String; }
pub async fn within<T>(origin: &str, future: impl Future<Output = T>) -> T {
    assert!(origin.starts_with("http://127.0.0.1:"));
    // Two task-local fixtures must not embed duplicate copies of a large model-probe future.
    crate::upstream::public_assets::within(
        origin,
        ORIGIN.scope(origin.to_owned(), Box::pin(future)),
    )
    .await
}
pub(super) fn target(url: &str) -> String {
    let path = url
        .strip_prefix("https://agents.lumalabs.ai")
        .or_else(|| url.strip_prefix("https://assets.example.test"))
        .expect("only the two declared synthetic origins can be remapped");
    ORIGIN
        .try_with(|origin| format!("{origin}{path}"))
        .expect("network tests must declare an explicit loopback scope")
}
#[derive(Clone, Debug)]
pub struct Call {
    pub method: String,
    pub path: String,
    pub headers: axum::http::HeaderMap,
    pub body: Value,
    pub raw_body: Vec<u8>,
}
pub struct Reply {
    pub status: u16,
    pub content_type: &'static str,
    pub bytes: Vec<u8>,
    pub hold: bool,
}
impl Reply {
    pub fn json(status: u16, value: Value) -> Self {
        Self {
            status,
            content_type: "application/json",
            bytes: serde_json::to_vec(&value).unwrap(),
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
            content_type: "image/png",
            bytes: out.into_inner(),
            hold: false,
        }
    }

    // A structural ISO-BMFF fixture, not a playable or real upstream video.
    pub fn mp4() -> Self {
        fn atom(kind: &[u8], body: &[u8]) -> Vec<u8> {
            let mut bytes = ((body.len() + 8) as u32).to_be_bytes().to_vec();
            bytes.extend_from_slice(kind);
            bytes.extend_from_slice(body);
            bytes
        }
        let mdia = atom(b"mdia", &atom(b"hdlr", b"\0\0\0\0\0\0\0\0vide"));
        let mut bytes = atom(b"ftyp", b"isom\0\0\0\0");
        bytes.extend(atom(b"moov", &atom(b"trak", &mdia)));
        bytes.extend(atom(b"mdat", b"synthetic"));
        Self {
            status: 200,
            content_type: "video/mp4",
            bytes,
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
    pub async fn wait_closed(&self) {
        tokio::time::timeout(Duration::from_secs(2), async {
            while self.state.closed.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("dropped HTTP owner must close its source");
    }
    pub fn client(&self) -> rquest::Client {
        loopback_client(&self.origin)
    }
}

pub fn loopback_client(origin: &str) -> rquest::Client {
    let url = url::Url::parse(origin).unwrap();
    assert_eq!(url.scheme(), "http");
    assert_eq!(url.host_str(), Some("127.0.0.1"));
    assert!(url.port().is_some());
    rquest::Client::builder()
        .no_proxy()
        .proxy(rquest::Proxy::all(origin).unwrap())
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap()
}
async fn handler(State(state): State<Fixture>, request: Request<Body>) -> Response {
    let (parts, body) = request.into_parts();
    assert!(
        matches!(parts.uri.scheme_str(), None | Some("http")),
        "fixture never forwards CONNECT"
    );
    let bytes = axum::body::to_bytes(body, 65 * 1024 * 1024).await.unwrap();
    let unsupported_method = parts.method == axum::http::Method::CONNECT;
    state.calls.lock().unwrap().push(Call {
        method: parts.method.to_string(),
        path: parts
            .uri
            .path_and_query()
            .map(|p| p.to_string())
            .unwrap_or_else(|| parts.uri.to_string()),
        body: if bytes.is_empty()
            || parts.method == axum::http::Method::PUT
            || parts
                .headers
                .get("content-type")
                .is_some_and(|v| v.as_bytes().starts_with(b"multipart/form-data"))
        {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        },
        headers: parts.headers,
        raw_body: bytes.to_vec(),
    });
    if unsupported_method {
        // Observe an accidental generic HTTPS attempt without forwarding it or poisoning the log.
        return Response::builder().status(502).body(Body::empty()).unwrap();
    }
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
        .header("content-type", reply.content_type)
        .body(body)
        .unwrap()
}
struct CloseProbe(Arc<AtomicUsize>);
impl Drop for CloseProbe {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
pub fn job(state: &str) -> Value {
    // Default deployment has no usage reporting; delayed settlement fixtures opt in explicitly.
    json!({"id":ID,"type":"image","model":"uni-1","state":state,
        "created_at":"2026-11-05T00:00:00Z","failure_reason":null,"failure_code":null,
        "output":if state=="completed" {json!([{"type":"image","url":ASSET}])}else{json!([])},
        "usage":null})
}
pub fn payload() -> crate::routing::candidate::ProviderAccountPayload {
    serde_json::from_value(json!({"adapter":"lumalabs_agents_compatible",
        "base_url":"https://agents.lumalabs.ai/v1","api_key":"luma-api-synthetic-only"}))
    .unwrap()
}
pub fn request(body: Value) -> crate::protocol::canonical::CanonicalRelayRequest {
    crate::protocol::gemini_business::normalize_image_generations(body).unwrap()
}
