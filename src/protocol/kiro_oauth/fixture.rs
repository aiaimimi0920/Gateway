//! Random loopback synthetic AWS OIDC peer, with joined server and tracked response-body lifetime.
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Uri},
    response::Response,
    routing::post,
    Router,
};
use futures::Stream;
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

#[derive(Clone)]
pub(crate) struct ResponseSpec {
    pub status: u16,
    pub body: Vec<u8>,
    pub headers: HeaderMap,
    pub header_delay: Duration,
    pub body_delay: Duration,
}
impl ResponseSpec {
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
pub(super) fn registration() -> ResponseSpec {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    ResponseSpec::json(
        200,
        json!({"clientId":"synthetic-gateway-client","clientSecret":"synthetic-registered-secret",
        "clientIdIssuedAt":now,"clientSecretExpiresAt":now+86400}),
    )
}
pub(super) fn device(interval: Option<u64>) -> ResponseSpec {
    let mut value = json!({"deviceCode":"synthetic-device-secret","userCode":"ABCD-EFGH",
        "verificationUri":"https://device.sso.us-east-1.amazonaws.com/",
        "verificationUriComplete":"https://device.sso.us-east-1.amazonaws.com/?user_code=ABCD-EFGH","expiresIn":600});
    if let Some(interval) = interval {
        value["interval"] = json!(interval);
    }
    ResponseSpec::json(200, value)
}
pub(super) fn tokens() -> ResponseSpec {
    ResponseSpec::json(
        200,
        json!({"accessToken":"synthetic-new-access","refreshToken":"synthetic-new-refresh","tokenType":"Bearer","expiresIn":3600}),
    )
}
pub(crate) type Calls = Arc<Mutex<Vec<(String, HeaderMap, Vec<u8>)>>>;
#[derive(Clone)]
struct ServerState {
    replies: Arc<Mutex<VecDeque<ResponseSpec>>>,
    calls: Calls,
    drops: Arc<AtomicUsize>,
}
async fn handle(
    State(state): State<ServerState>,
    uri: Uri,
    headers: HeaderMap,
    body: bytes::Bytes,
) -> Response<Body> {
    state
        .calls
        .lock()
        .unwrap()
        .push((uri.path().into(), headers, body.to_vec()));
    let reply = state
        .replies
        .lock()
        .unwrap()
        .pop_front()
        .expect("unexpected extra OIDC request");
    tokio::time::sleep(reply.header_delay).await;
    let mut builder = Response::builder()
        .status(reply.status)
        .header("content-type", "application/json");
    for (name, value) in reply.headers {
        if let Some(name) = name {
            builder = builder.header(name, value);
        }
    }
    builder
        .body(Body::from_stream(TrackedBody {
            delay: Box::pin(tokio::time::sleep(reply.body_delay)),
            body: Some(reply.body),
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
    pub async fn new(replies: Vec<ResponseSpec>) -> Self {
        let calls = Calls::default();
        let drops = Arc::new(AtomicUsize::new(0));
        let state = ServerState {
            replies: Arc::new(Mutex::new(replies.into())),
            calls: calls.clone(),
            drops: drops.clone(),
        };
        let app = Router::new()
            .route("/client/register", post(handle))
            .route("/device_authorization", post(handle))
            .route("/token", post(handle))
            .with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let task = Some(tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap()
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
    pub async fn wait_count(&self, count: usize) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while self.count() < count {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
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
    delay: Pin<Box<tokio::time::Sleep>>,
    body: Option<Vec<u8>>,
    drops: Arc<AtomicUsize>,
}
impl Stream for TrackedBody {
    type Item = Result<bytes::Bytes, Infallible>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        use std::future::Future;
        let this = self.get_mut();
        if this.delay.as_mut().poll(cx).is_pending() {
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
pub(super) fn http() -> rquest::Client {
    let mut headers = rquest::header::HeaderMap::new();
    for name in ["authorization", "cookie", "x-api-key"] {
        headers.insert(name, "must-not-leak".parse().unwrap());
    }
    rquest::Client::builder()
        .no_proxy()
        .default_headers(headers)
        .build()
        .unwrap()
}
pub(super) async fn register(mock: &Mock, http: &rquest::Client) -> super::ClientRegistration {
    super::TEST_ORIGIN
        .scope(
            mock.origin.clone(),
            super::ClientRegistration::register(
                http,
                super::Directory::builder_id(),
                &[super::Scope::Conversations],
            ),
        )
        .await
        .unwrap()
}
pub(super) async fn begin(
    mock: &Mock,
    client: &super::ClientRegistration,
    http: &rquest::Client,
) -> super::DeviceChallenge {
    super::TEST_ORIGIN
        .scope(mock.origin.clone(), client.begin_device(http))
        .await
        .unwrap()
}
