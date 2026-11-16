//! Non-forwarding loopback HTTP owner with real byte fragmentation and response-drop probes.
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Request},
    response::Response,
    Router,
};
use serde_json::Value;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
type Calls = Arc<Mutex<Vec<(HeaderMap, Value)>>>;

#[derive(Clone)]
struct Reply {
    status: u16,
    content_type: &'static str,
    body: Vec<u8>,
    calls: Calls,
    authority: String,
    attempts: Arc<AtomicUsize>,
    closed: Arc<AtomicUsize>,
    fragmented: bool,
    hold: bool,
}
pub(crate) struct Server {
    pub origin: String,
    pub calls: Calls,
    attempts: Arc<AtomicUsize>,
    closed: Arc<AtomicUsize>,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl Server {
    pub async fn start(status: u16, content_type: &'static str, body: Vec<u8>) -> Self {
        Self::reply(status, content_type, body, false, false).await
    }
    pub async fn stream(body: Vec<u8>, hold: bool) -> Self {
        Self::reply(200, "application/vnd.amazon.eventstream", body, true, hold).await
    }
    async fn reply(
        status: u16,
        content_type: &'static str,
        body: Vec<u8>,
        fragmented: bool,
        hold: bool,
    ) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let authority = listener.local_addr().unwrap().to_string();
        let origin = format!("http://{authority}");
        let calls = Arc::new(Mutex::new(Vec::new()));
        let attempts = Arc::new(AtomicUsize::new(0));
        let closed = Arc::new(AtomicUsize::new(0));
        let app = Router::new().fallback(handler).with_state(Reply {
            status,
            content_type,
            body,
            calls: calls.clone(),
            authority,
            attempts: attempts.clone(),
            closed: closed.clone(),
            fragmented,
            hold,
        });
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self {
            origin,
            calls,
            attempts,
            closed,
            task: Some(task),
        }
    }
    pub fn client(&self) -> rquest::Client {
        // A missing transport seam reaches only this rejecting proxy, never the real runtime.
        rquest::Client::builder()
            .no_proxy()
            .proxy(rquest::Proxy::all(&self.origin).unwrap())
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap()
    }
    pub fn attempts(&self) -> usize {
        self.attempts.load(Ordering::SeqCst)
    }
    pub fn closed(&self) -> usize {
        self.closed.load(Ordering::SeqCst)
    }
    pub async fn stop(mut self) {
        let task = self.task.take().unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
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
        .map(|a| a.as_str())
        .unwrap_or_else(|| parts.headers["host"].to_str().unwrap());
    assert_eq!(authority, reply.authority);
    assert_eq!(parts.uri.path(), "/");
    let raw = axum::body::to_bytes(body, 64 * 1024).await.unwrap();
    reply
        .calls
        .lock()
        .unwrap()
        .push((parts.headers, serde_json::from_slice(&raw).unwrap()));
    let body = if reply.fragmented {
        let chunks = reply
            .body
            .chunks(7)
            .map(bytes::Bytes::copy_from_slice)
            .collect::<Vec<_>>();
        Body::from_stream(futures::stream::unfold(
            (chunks.into_iter(), CloseProbe(reply.closed), reply.hold),
            |(mut chunks, probe, hold)| async move {
                match chunks.next() {
                    Some(bytes) => Some((Ok::<_, std::io::Error>(bytes), (chunks, probe, hold))),
                    None if hold => std::future::pending().await,
                    None => None,
                }
            },
        ))
    } else {
        Body::from(reply.body)
    };
    Response::builder()
        .status(reply.status)
        .header("content-type", reply.content_type)
        .header("location", "http://127.0.0.1:9/never-follow")
        .body(body)
        .unwrap()
}
struct CloseProbe(Arc<AtomicUsize>);
impl Drop for CloseProbe {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
