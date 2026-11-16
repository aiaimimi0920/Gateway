//! Test-only final transport seam and non-forwarding local proxy; no production escape hatch.
use crate::{routing::candidate::ProviderAccountPayload, upstream::common::RequestPlan};
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

pub(super) async fn within<T>(origin: &str, future: impl Future<Output = T>) -> T {
    crate::request_wait::transport::within(false, ORIGIN.scope(origin.to_owned(), future)).await
}

pub(super) fn transport_plan(
    payload: &ProviderAccountPayload,
    plan: &RequestPlan,
) -> Option<RequestPlan> {
    if !crate::protocol::qwen::workspace_api::owns_payload(payload) {
        return None;
    }
    ORIGIN
        .try_with(|origin| {
            let url = url::Url::parse(&plan.url).unwrap();
            assert_eq!(url.scheme(), "https");
            assert!(url.host_str().unwrap().ends_with(".maas.aliyuncs.com"));
            assert_eq!(url.path(), "/compatible-mode/v1/chat/completions");
            RequestPlan {
                method: plan.method.clone(),
                url: format!("{origin}/{}{}", url.host_str().unwrap(), url.path()),
                query: plan.query.clone(),
                body: plan.body.clone(),
                response_kind: plan.response_kind,
            }
        })
        .ok()
}

#[derive(Clone, Debug)]
pub(super) struct Call {
    pub path: String,
    pub authorization: String,
    pub user_agent: String,
    pub body: Value,
}
#[derive(Clone)]
struct Reply {
    body: Vec<u8>,
    status: u16,
    oversized: bool,
    calls: Arc<Mutex<Vec<Call>>>,
    authority: String,
    attempts: Arc<AtomicUsize>,
    streaming: bool,
    hold: bool,
    closed: Arc<AtomicUsize>,
}
pub(super) struct Server {
    pub origin: String,
    calls: Arc<Mutex<Vec<Call>>>,
    attempts: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
    closed: Arc<AtomicUsize>,
}
impl Server {
    pub async fn start(body: impl Into<Vec<u8>>, status: u16, oversized: bool) -> Self {
        Self::start_reply(body.into(), status, oversized, false, false).await
    }
    pub async fn stream(body: String, hold: bool) -> Self {
        Self::start_reply(body.into_bytes(), 200, false, true, hold).await
    }
    pub async fn hold_json(body: String) -> Self {
        Self::start_reply(body.into_bytes(), 200, false, false, true).await
    }
    async fn start_reply(
        body: Vec<u8>,
        status: u16,
        oversized: bool,
        streaming: bool,
        hold: bool,
    ) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let calls = Arc::new(Mutex::new(Vec::new()));
        let attempts = Arc::new(AtomicUsize::new(0));
        let closed = Arc::new(AtomicUsize::new(0));
        let app = Router::new().fallback(handler).with_state(Reply {
            body,
            status,
            oversized,
            calls: calls.clone(),
            authority: listener.local_addr().unwrap().to_string(),
            attempts: attempts.clone(),
            streaming,
            hold,
            closed: closed.clone(),
        });
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            origin,
            calls,
            attempts,
            task,
            closed,
        }
    }
    pub fn client(&self) -> rquest::Client {
        // Even without a working seam the synthetic Key can only reach this non-forwarding proxy.
        rquest::Client::builder()
            .no_proxy()
            .proxy(rquest::Proxy::all(&self.origin).unwrap())
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap()
    }
    pub fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }
    pub fn attempts(&self) -> usize {
        self.attempts.load(Ordering::SeqCst)
    }
    pub fn closed(&self) -> usize {
        self.closed.load(Ordering::SeqCst)
    }
    pub async fn stop(self) {
        self.task.abort();
        let _ = self.task.await;
    }
}
async fn handler(State(reply): State<Reply>, request: Request<Body>) -> Response {
    reply.attempts.fetch_add(1, Ordering::SeqCst);
    if request.method() != axum::http::Method::POST {
        return Response::builder().status(502).body(Body::empty()).unwrap();
    }
    let (parts, body) = request.into_parts();
    // Both direct origin-form and proxy absolute-form are valid; neither may name another target.
    assert!(matches!(parts.uri.scheme_str(), None | Some("http")));
    let authority = parts
        .uri
        .authority()
        .map(|v| v.as_str())
        .unwrap_or_else(|| parts.headers["host"].to_str().unwrap());
    assert_eq!(authority, reply.authority);
    let body =
        serde_json::from_slice(&axum::body::to_bytes(body, 64 * 1024).await.unwrap()).unwrap();
    reply.calls.lock().unwrap().push(Call {
        path: parts.uri.path().into(),
        authorization: parts.headers["authorization"].to_str().unwrap().into(),
        user_agent: parts.headers["user-agent"].to_str().unwrap().into(),
        body,
    });
    let body = if reply.oversized {
        // No giant fixture allocation: valid JSON crosses the real 64 MiB accumulated-byte budget.
        let prefix = bytes::Bytes::from_static(
            b"{\"choices\":[{\"message\":{\"role\":\"assistant\",\"content\":\"",
        );
        let blocks =
            (0..65537).map(|_| Ok::<_, std::io::Error>(bytes::Bytes::from_static(&[b'x'; 1024])));
        let suffix = bytes::Bytes::from_static(b"\"},\"finish_reason\":\"stop\"}]}");
        Body::from_stream(futures::stream::iter(
            std::iter::once(Ok(prefix))
                .chain(blocks)
                .chain(std::iter::once(Ok(suffix))),
        ))
    } else if reply.streaming || reply.hold {
        // Real fragmented HTTP SSE with a drop probe; held responses never contact another origin.
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
        Body::from_stream(futures::stream::unfold(
            state,
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
        .header(
            "content-type",
            if reply.streaming {
                "text/event-stream"
            } else {
                "application/json"
            },
        )
        .body(body)
        .unwrap()
}
struct CloseProbe(Arc<AtomicUsize>);
impl Drop for CloseProbe {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
pub(super) fn completed() -> Value {
    json!({"id":"qwen-fixture","model":"qwen-synthetic","output":[],
        "choices":[{"index":0,"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}],
        "usage":{"prompt_tokens":8,"completion_tokens":2,"total_tokens":10,"prompt_tokens_details":{"cached_tokens":3}}})
}
