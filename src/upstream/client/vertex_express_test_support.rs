//! Tests retain the fixed production URL until final validated transport, then use a non-forwarding loopback server.
use crate::{routing::candidate::ProviderAccountPayload, upstream::common::RequestPlan};
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Request},
    response::Response,
    Router,
};
use serde_json::Value;
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
tokio::task_local! {static ORIGIN:String;}
type Calls = Arc<Mutex<Vec<(String, HeaderMap, Value)>>>;
pub(crate) async fn within<T>(origin: &str, future: impl std::future::Future<Output = T>) -> T {
    let url = url::Url::parse(origin).unwrap();
    assert_eq!(url.scheme(), "http");
    assert_eq!(url.host_str(), Some("127.0.0.1"));
    assert!(url.port().is_some());
    crate::request_wait::transport::within(false, ORIGIN.scope(origin.to_owned(), future)).await
}
pub(super) fn transport_plan(
    payload: &ProviderAccountPayload,
    plan: &RequestPlan,
) -> Option<RequestPlan> {
    if !crate::protocol::vertex_express::owns_payload(payload) {
        return None;
    }
    crate::protocol::vertex_express::validate_plan(payload, plan).unwrap();
    Some(
        ORIGIN
            .try_with(|origin| RequestPlan {
                method: plan.method.clone(),
                url: format!(
                    "{origin}{}",
                    plan.url
                        .strip_prefix("https://aiplatform.googleapis.com")
                        .unwrap()
                ),
                query: plan.query.clone(),
                body: plan.body.clone(),
                response_kind: plan.response_kind,
            })
            .expect("Express tests must explicitly select loopback transport"),
    )
}
#[derive(Clone)]
struct Reply {
    status: u16,
    bodies: Arc<Mutex<VecDeque<Vec<u8>>>>,
    repeat: bool,
    calls: Calls,
    authority: String,
    hold: bool,
    closed: Arc<AtomicUsize>,
}
pub(crate) struct Server {
    pub origin: String,
    pub calls: Calls,
    closed: Arc<AtomicUsize>,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl Server {
    pub async fn start(status: u16, body: Vec<u8>) -> Self {
        Self::stream(status, body, false).await
    }
    pub async fn stream(status: u16, body: Vec<u8>, hold: bool) -> Self {
        Self::serve(status, vec![body], hold, true).await
    }
    pub async fn sequence(bodies: Vec<Vec<u8>>) -> Self {
        assert!(!bodies.is_empty() && bodies.len() <= 16);
        Self::serve(200, bodies, false, false).await
    }
    async fn serve(status: u16, bodies: Vec<Vec<u8>>, hold: bool, repeat: bool) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let authority = listener.local_addr().unwrap().to_string();
        let origin = format!("http://{authority}");
        let calls = Arc::new(Mutex::new(Vec::new()));
        let closed = Arc::new(AtomicUsize::new(0));
        let app = Router::new().fallback(handler).with_state(Reply {
            status,
            bodies: Arc::new(Mutex::new(bodies.into())),
            repeat,
            calls: calls.clone(),
            authority,
            hold,
            closed: closed.clone(),
        });
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self {
            origin,
            calls,
            closed,
            task: Some(task),
        }
    }
    pub fn client(&self) -> rquest::Client {
        // Missing test seam can only reach this rejecting proxy, never a real Google endpoint.
        rquest::Client::builder()
            .no_proxy()
            .proxy(rquest::Proxy::all(&self.origin).unwrap())
            .timeout(std::time::Duration::from_secs(5))
            .redirect(rquest::redirect::Policy::limited(3))
            .build()
            .unwrap()
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
    if request.method() != axum::http::Method::POST {
        return Response::builder().status(502).body(Body::empty()).unwrap();
    }
    let (parts, body) = request.into_parts();
    assert_eq!(parts.headers["host"], reply.authority);
    assert!(matches!(parts.uri.scheme_str(), None | Some("http")));
    let bytes = axum::body::to_bytes(body, 64 * 1024).await.unwrap();
    reply.calls.lock().unwrap().push((
        parts.uri.path_and_query().unwrap().to_string(),
        parts.headers,
        serde_json::from_slice(&bytes).unwrap(),
    ));
    let bytes = {
        let mut bodies = reply.bodies.lock().unwrap();
        if reply.repeat {
            bodies.front().cloned()
        } else {
            bodies.pop_front()
        }
    };
    let Some(bytes) = bytes else {
        return Response::builder().status(502).body(Body::empty()).unwrap();
    };
    // Fragment at arbitrary UTF-8/JSON boundaries and observe response-source drop on cancellation.
    let chunks = bytes
        .chunks(7)
        .map(bytes::Bytes::copy_from_slice)
        .collect::<Vec<_>>();
    let body = Body::from_stream(futures::stream::unfold(
        (chunks.into_iter(), CloseProbe(reply.closed), reply.hold),
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
        .header(
            "location",
            format!(
                "{}/redirect-must-not-receive-key",
                format!("http://{}", reply.authority)
            ),
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
