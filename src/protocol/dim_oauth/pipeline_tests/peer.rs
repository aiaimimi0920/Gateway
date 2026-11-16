//! Non-forwarding native peer: arbitrary byte fragmentation and observable source drop.
use super::*;
use axum::{extract::State, http::HeaderMap, response::Response, Router};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};
pub(super) type Calls = Vec<(String, HeaderMap, Value)>;
#[derive(Clone)]
struct Reply {
    family: &'static str,
    tools: bool,
    hold: bool,
    status: u16,
    calls: Arc<Mutex<Calls>>,
    closed: Arc<AtomicUsize>,
    raw: Option<Vec<u8>>,
}
pub(super) struct Peer {
    pub origin: String,
    pub calls: Arc<Mutex<Calls>>,
    closed: Arc<AtomicUsize>,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl Peer {
    pub async fn new(family: &'static str, tools: bool, hold: bool, status: u16) -> Self {
        Self::serve(family, tools, hold, status, None).await
    }
    pub async fn raw(bytes: Vec<u8>) -> Self {
        Self::serve("gemini", false, false, 200, Some(bytes)).await
    }
    async fn serve(
        family: &'static str,
        tools: bool,
        hold: bool,
        status: u16,
        raw: Option<Vec<u8>>,
    ) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let calls = Arc::new(Mutex::new(Vec::new()));
        let closed = Arc::new(AtomicUsize::new(0));
        let app = Router::new().fallback(handler).with_state(Reply {
            family,
            tools,
            hold,
            status,
            calls: calls.clone(),
            closed: closed.clone(),
            raw,
        });
        let task = Some(tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap()
        }));
        Self {
            origin,
            calls,
            closed,
            task,
        }
    }
    pub fn client(&self) -> rquest::Client {
        // Any missing fixed-target test seam reaches only this rejecting loopback proxy.
        rquest::Client::builder()
            .no_proxy()
            .proxy(rquest::Proxy::all(&self.origin).unwrap())
            .timeout(Duration::from_secs(5))
            .redirect(rquest::redirect::Policy::limited(3))
            .build()
            .unwrap()
    }
    pub async fn wait_calls(&self) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while self.calls.lock().unwrap().is_empty() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
    pub async fn wait_closed(&self) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while self.closed.load(Ordering::SeqCst) == 0 {
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
impl Drop for Peer {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
async fn handler(State(reply): State<Reply>, request: Request<Body>) -> Response {
    if request.method() != axum::http::Method::POST
        || request.uri().scheme_str().is_some_and(|s| s != "http")
    {
        return Response::builder().status(502).body(Body::empty()).unwrap();
    }
    let (parts, body) = request.into_parts();
    let raw: Value =
        serde_json::from_slice(&axum::body::to_bytes(body, 128 * 1024).await.unwrap()).unwrap();
    let stream = raw["stream"] == true || parts.uri.path().ends_with(":streamGenerateContent");
    let first = {
        let mut calls = reply.calls.lock().unwrap();
        calls.push((
            parts.uri.path_and_query().unwrap().to_string(),
            parts.headers,
            raw,
        ));
        calls.len() == 1
    };
    let bytes = if let Some(raw) = reply.raw {
        raw
    } else if reply.status == 200 {
        native::reply(reply.family, reply.tools && first, stream, !reply.hold)
    } else {
        b"synthetic-private-error synthetic-dim-public-access".to_vec()
    };
    let chunks = bytes
        .chunks(7)
        .map(bytes::Bytes::copy_from_slice)
        .collect::<Vec<_>>();
    let body = Body::from_stream(futures::stream::unfold(
        (chunks.into_iter(), Close(reply.closed), reply.hold),
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
        .header(
            "content-type",
            if stream {
                "text/event-stream"
            } else {
                "application/json"
            },
        )
        .body(body)
        .unwrap()
}
struct Close(Arc<AtomicUsize>);
impl Drop for Close {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
