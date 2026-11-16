//! Scoped final transport replacement plus a non-forwarding proxy: synthetic material stays local.
use axum::{body::Body, extract::State, http::Request, response::Response, Router};
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
    assert_eq!(url.scheme(), "http");
    assert_eq!(url.host_str(), Some("127.0.0.1"));
    assert!(url.port().is_some());
    crate::request_wait::transport::within(false, ORIGIN.scope(origin.to_owned(), future)).await
}

pub(super) fn target(url: &str) -> String {
    assert_eq!(url, super::ASK_URL);
    ORIGIN
        .try_with(|origin| {
            format!(
                "{origin}{}",
                crate::protocol::perplexity_web::material::ASK_PATH
            )
        })
        .expect("Perplexity dispatcher tests require an explicit loopback scope")
}

#[derive(Clone, Debug)]
pub(crate) struct Call {
    pub path: String,
    pub headers: axum::http::HeaderMap,
    pub body: Value,
}
#[derive(Clone)]
struct Reply {
    body: Vec<u8>,
    status: u16,
    hold: bool,
    calls: Arc<Mutex<Vec<Call>>>,
    attempts: Arc<AtomicUsize>,
    closed: Arc<AtomicUsize>,
    authority: String,
}
pub(crate) struct Server {
    pub origin: String,
    reply: Reply,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Server {
    pub async fn start(body: impl Into<Vec<u8>>, status: u16, hold: bool) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let reply = Reply {
            body: body.into(),
            status,
            hold,
            calls: Arc::new(Mutex::new(Vec::new())),
            attempts: Arc::new(AtomicUsize::new(0)),
            closed: Arc::new(AtomicUsize::new(0)),
            authority: address.to_string(),
        };
        let app = Router::new().fallback(handler).with_state(reply.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            origin: format!("http://{address}"),
            reply,
            task,
        }
    }
    pub fn client(&self) -> rquest::Client {
        // A missing remap can only hit this proxy; it never forwards HTTPS CONNECT or redirects.
        rquest::Client::builder()
            .no_proxy()
            .proxy(rquest::Proxy::all(&self.origin).unwrap())
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap()
    }
    pub fn calls(&self) -> Vec<Call> {
        self.reply.calls.lock().unwrap().clone()
    }
    pub fn attempts(&self) -> usize {
        self.reply.attempts.load(Ordering::SeqCst)
    }
    pub fn closed(&self) -> usize {
        self.reply.closed.load(Ordering::SeqCst)
    }
}
async fn handler(State(reply): State<Reply>, request: Request<Body>) -> Response {
    reply.attempts.fetch_add(1, Ordering::SeqCst);
    if request.method() != axum::http::Method::POST {
        return Response::builder().status(502).body(Body::empty()).unwrap();
    }
    let (parts, body) = request.into_parts();
    assert!(matches!(parts.uri.scheme_str(), None | Some("http")));
    let authority = parts
        .uri
        .authority()
        .map(|v| v.as_str())
        .unwrap_or_else(|| parts.headers["host"].to_str().unwrap());
    assert_eq!(authority, reply.authority);
    let body =
        serde_json::from_slice(&axum::body::to_bytes(body, 128 * 1024).await.unwrap()).unwrap();
    reply.calls.lock().unwrap().push(Call {
        path: parts.uri.path().into(),
        headers: parts.headers,
        body,
    });
    let chunks: Vec<_> = reply
        .body
        .chunks(7)
        .map(bytes::Bytes::copy_from_slice)
        .collect();
    let state = (
        chunks.into_iter(),
        CloseProbe(reply.closed.clone()),
        reply.hold,
    );
    let body = Body::from_stream(futures::stream::unfold(
        state,
        |(mut chunks, probe, hold)| async move {
            match chunks.next() {
                Some(bytes) => Some((Ok::<_, std::io::Error>(bytes), (chunks, probe, hold))),
                None if hold => std::future::pending().await,
                None => None,
            }
        },
    ));
    Response::builder()
        .status(reply.status)
        .header("content-type", "text/event-stream")
        .body(body)
        .unwrap()
}
struct CloseProbe(Arc<AtomicUsize>);
impl Drop for CloseProbe {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
pub(crate) fn completed() -> String {
    format!(
        "data: {}\n\nevent: end_of_stream\n\n",
        json!({"blocks":[
        {"intended_usage":"ask_text","markdown_block":{"answer":"合成答案"}},
        {"intended_usage":"web_results","web_result_block":{"web_results":[{"name":"来源","url":"https://example.invalid/source","snippet":"摘要"}]}}
    ],"read_write_token":"synthetic-private-followup"})
    )
}
