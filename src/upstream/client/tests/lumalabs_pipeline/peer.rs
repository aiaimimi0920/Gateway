//! Explicit loopback executor and asset peer. It never forwards or accesses real media.
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

pub const ASSET: &str = "https://assets.example.test/image.png?signature=synthetic-private";
#[derive(Clone, Debug)]
pub struct Call {
    pub method: String,
    pub path: String,
    pub headers: axum::http::HeaderMap,
    pub body: Vec<u8>,
}
pub struct Reply {
    pub status: u16,
    pub mime: &'static str,
    pub bytes: Vec<u8>,
    pub hold: bool,
}
impl Reply {
    pub fn task(mut result: Value) -> Self {
        result["ok"] = true.into();
        Self::bytes(
            "application/json",
            json!({"ok":true,"result":result}).to_string().into_bytes(),
        )
    }
    pub fn executor(url: &str) -> Self {
        Self::task(
            json!({"signedUrl":url,"outputId":"output-1","actionId":null,
            "resolvedActionType":"create_image_uni_1","realmId":"synthetic-realm",
            "mediaOperation":"image","artifactField":"image","state":"completed"}),
        )
    }
    pub fn bytes(mime: &'static str, bytes: Vec<u8>) -> Self {
        Self {
            status: 200,
            mime,
            bytes,
            hold: false,
        }
    }
    pub fn image(format: image::ImageFormat) -> Self {
        let mut output = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(2, 1)
            .write_to(&mut output, format)
            .unwrap();
        Self::bytes(
            if format == image::ImageFormat::Png {
                "image/png"
            } else {
                "image/jpeg"
            },
            output.into_inner(),
        )
    }
}
#[derive(Clone)]
struct PeerState {
    replies: Arc<Mutex<VecDeque<Reply>>>,
    calls: Arc<Mutex<Vec<Call>>>,
    closed: Arc<AtomicUsize>,
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
    pub async fn start(replies: Vec<Reply>) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let state = PeerState {
            replies: Arc::new(Mutex::new(replies.into())),
            calls: Arc::default(),
            closed: Arc::default(),
        };
        let app = Router::new().fallback(handler).with_state(state.clone());
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self {
            origin,
            state,
            task,
        }
    }
    pub fn calls(&self) -> Vec<Call> {
        self.state.calls.lock().unwrap().clone()
    }
    pub async fn wait_calls(&self, count: usize) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while self.calls().len() < count {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("expected synthetic request");
    }
    pub async fn wait_closed(&self) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while self.state.closed.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("cancelled caller closes held source");
    }
    pub async fn stop(mut self) {
        self.task.abort();
        let _ = (&mut self.task).await;
    }
}
async fn handler(State(state): State<PeerState>, request: Request<Body>) -> Response {
    let (parts, body) = request.into_parts();
    assert!(matches!(
        (parts.method.as_str(), parts.uri.path()),
        (
            "POST",
            "/v1/internal/browser-executor/luma-session-v1/execute"
        ) | ("GET", "/image.png")
    ));
    let bytes = axum::body::to_bytes(body, 1024 * 1024).await.unwrap();
    state.calls.lock().unwrap().push(Call {
        method: parts.method.to_string(),
        path: parts.uri.path_and_query().unwrap().to_string(),
        headers: parts.headers,
        body: bytes.to_vec(),
    });
    let mut reply = state
        .replies
        .lock()
        .unwrap()
        .pop_front()
        .expect("unexpected additional generation/download");
    if parts.method == axum::http::Method::POST {
        let input: Value = serde_json::from_slice(&bytes).unwrap();
        assert!(uuid::Uuid::parse_str(input["input"]["sessionBinding"].as_str().unwrap()).is_ok());
        let mut response: Value = serde_json::from_slice(&reply.bytes).unwrap();
        response["result"]["session"] = json!({"version":1,"binding":input["input"]["sessionBinding"],
            "reauthenticationRequired":false,"cookie":{"name":"wos-session","value":"synthetic-rotated-cookie",
            "domain":"app.lumalabs.ai","path":"/","expires":-1,"httpOnly":true,"secure":true,"sameSite":"Lax"}});
        reply.bytes = serde_json::to_vec(&response).unwrap();
    }
    let body = if reply.hold {
        Body::from_stream(futures::stream::unfold(
            CloseProbe(state.closed),
            |probe| async {
                std::future::pending::<()>().await;
                Some((Ok::<_, std::io::Error>(bytes::Bytes::new()), probe))
            },
        ))
    } else {
        let chunks: Vec<_> = reply
            .bytes
            .chunks(4096)
            .map(|p| Ok::<_, std::io::Error>(bytes::Bytes::copy_from_slice(p)))
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

pub fn wire(peer: &Peer, asset: bool) {
    let calls = peer.calls();
    assert_eq!(
        calls.len(),
        if asset { 2 } else { 1 },
        "never retry or switch accounts"
    );
    assert_eq!(calls[0].method, "POST");
    assert_eq!(
        calls[0].path,
        "/v1/internal/browser-executor/luma-session-v1/execute"
    );
    assert_eq!(
        calls[0].headers["authorization"],
        "Bearer synthetic-executor-token"
    );
    assert!(!calls[0].headers.contains_key("cookie") && !calls[0].headers.contains_key("origin"));
    let body: Value = serde_json::from_slice(&calls[0].body).unwrap();
    assert_eq!(body["provider"], "lumalabs");
    assert_eq!(body["providerAccountId"], "luma-pipeline");
    assert_eq!(body["endpointKind"], "images_generations");
    assert_eq!(body["input"]["sessionToken"], "luma-synthetic-exact");
    assert_eq!(
        body["input"]["actionBody"]["fields"]["model"],
        "luma-fixture-native"
    );
    assert!(!body.to_string().contains("caller-secret"));
    if asset {
        assert_eq!(calls[1].method, "GET");
        assert_eq!(calls[1].path, "/image.png?signature=synthetic-private");
        assert!(calls[1].body.is_empty());
        for name in [
            "authorization",
            "cookie",
            "origin",
            "referer",
            "x-api-key",
            "user-agent",
        ] {
            assert!(
                !calls[1].headers.contains_key(name),
                "asset must not inherit {name}"
            );
        }
    }
}
