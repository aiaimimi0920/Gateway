//! Synthetic non-forwarding peer. Every network test must explicitly choose a loopback origin.
use super::{Client, ReceivedSession};
use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::{Request, Response},
    Router,
};
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::{collections::VecDeque, sync::Arc};

pub(crate) fn inherit_origin<F: std::future::Future + Send>(
    future: F,
) -> impl std::future::Future<Output = F::Output> + Send {
    let origin = super::transport::TEST_ORIGIN
        .try_with(Clone::clone)
        .expect("Lobster management tests require an explicit loopback scope");
    super::transport::TEST_ORIGIN.scope(origin, future)
}

pub(crate) fn client() -> Client {
    Client::new(uuid::Uuid::new_v4(), rquest::Client::builder().no_proxy()).unwrap()
}
pub(crate) fn tokens() -> Value {
    json!({"code":0,"data":{"accessToken":"synthetic-access","refreshToken":"synthetic-refresh",
        "accountMode":"personal", "user":{"yid":"synthetic-user"}}})
}
pub(crate) fn receipt() -> ReceivedSession {
    ReceivedSession::capture(client(), tokens(), None).unwrap()
}
pub(crate) fn callback(authorization: &str, code: &str) -> String {
    let url = url::Url::parse(authorization).unwrap();
    let (_, query) = url.fragment().unwrap().split_once('?').unwrap();
    let pairs: std::collections::HashMap<_, _> = url::form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect();
    let mut callback = url::Url::parse(&pairs["redirect_uri"]).unwrap();
    callback
        .query_pairs_mut()
        .extend_pairs([("state", pairs["state"].as_str()), ("code", code)]);
    callback.into()
}
pub(crate) struct Seen {
    pub path: String,
    pub method: String,
    pub headers: axum::http::HeaderMap,
    pub body: Value,
    pub query: Option<String>,
}
pub(crate) struct Reply {
    status: u16,
    chunks: Vec<Vec<u8>>,
    headers: Vec<(&'static str, String)>,
    stall: bool,
    gate: Option<Arc<tokio::sync::Notify>>,
}
impl Reply {
    pub fn json(value: Value) -> Self {
        Self::raw(200, value.to_string().into_bytes())
    }
    pub fn raw(status: u16, bytes: Vec<u8>) -> Self {
        Self {
            status,
            chunks: vec![bytes],
            headers: vec![],
            stall: false,
            gate: None,
        }
    }
    pub fn chunks(chunks: Vec<Vec<u8>>) -> Self {
        Self {
            chunks,
            ..Self::raw(200, vec![])
        }
    }
    pub fn stalled() -> Self {
        Self {
            stall: true,
            ..Self::raw(200, vec![])
        }
    }
    pub fn gated(mut self, gate: Arc<tokio::sync::Notify>) -> Self {
        self.gate = Some(gate);
        self
    }
    pub fn header(mut self, key: &'static str, value: &str) -> Self {
        self.headers.push((key, value.into()));
        self
    }
}
#[derive(Default)]
struct Script {
    replies: Mutex<VecDeque<Reply>>,
    seen: Mutex<Vec<Seen>>,
}
pub(crate) struct Peer {
    pub origin: String,
    script: Arc<Script>,
    task: tokio::task::JoinHandle<()>,
}
impl Peer {
    pub async fn start(replies: Vec<Reply>) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let script = Arc::new(Script {
            replies: Mutex::new(replies.into()),
            ..Default::default()
        });
        let app = Router::new().fallback(handle).with_state(script.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            origin,
            script,
            task,
        }
    }
    pub fn builder(&self) -> rquest::ClientBuilder {
        let mut headers = rquest::header::HeaderMap::new();
        for (key, value) in [
            ("authorization", "Bearer ambient-secret"),
            ("cookie", "ambient-cookie"),
            ("x-api-key", "ambient-key"),
            ("x-account-mode", "enterprise"),
        ] {
            headers.insert(
                rquest::header::HeaderName::from_static(key),
                rquest::header::HeaderValue::from_static(value),
            );
        }
        rquest::Client::builder()
            .default_headers(headers)
            .proxy(rquest::Proxy::all(&self.origin).unwrap())
    }
    pub fn client(&self) -> Client {
        Client::new(uuid::Uuid::new_v4(), self.builder()).unwrap()
    }
    pub fn receipt(&self) -> ReceivedSession {
        ReceivedSession::capture(self.client(), tokens(), None).unwrap()
    }
    pub fn inspect(&self, check: impl FnOnce(&[Seen])) {
        check(&self.script.seen.lock());
    }
}
async fn handle(State(script): State<Arc<Script>>, request: Request<Body>) -> Response<Body> {
    let (parts, body) = request.into_parts();
    let bytes = to_bytes(body, 65_536).await.unwrap();
    script.seen.lock().push(Seen {
        path: parts.uri.path().into(),
        method: parts.method.to_string(),
        headers: parts.headers,
        body: if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        },
        query: parts.uri.query().map(str::to_owned),
    });
    let reply = script
        .replies
        .lock()
        .pop_front()
        .expect("unexpected additional request");
    if let Some(gate) = reply.gate {
        gate.notified().await;
    }
    let mut response = Response::builder()
        .status(reply.status)
        .header("content-type", "application/json");
    for (key, value) in reply.headers {
        // Explicit fixture headers replace defaults, as a real response has one media type.
        response
            .headers_mut()
            .unwrap()
            .insert(key, value.parse().unwrap());
    }
    let body = if reply.stall {
        Body::from_stream(futures::stream::pending::<
            Result<bytes::Bytes, std::io::Error>,
        >())
    } else {
        Body::from_stream(futures::stream::iter(
            reply
                .chunks
                .into_iter()
                .map(|c| Ok::<_, std::io::Error>(bytes::Bytes::from(c))),
        ))
    };
    response.body(body).unwrap()
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
