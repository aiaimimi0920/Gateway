//! Compile-time, non-forwarding peer. Production plans and DB grant audiences stay unchanged.
#[path = "../../../tests/accio_relay_contract/fixture.rs"]
mod relay;
use crate::{routing::candidate::ProviderAccountPayload, upstream::common::RequestPlan};
use axum::{
    body::{Body, Bytes},
    extract::State,
    http::{HeaderMap, Request, StatusCode, Uri},
    middleware::Next,
    response::Response,
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use std::{
    future::Future,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{sync::oneshot, task::JoinHandle};

tokio::task_local! { static ORIGIN: String; }
pub(crate) fn selected_origin() -> Option<String> {
    ORIGIN.try_with(Clone::clone).ok()
}
pub(super) async fn within<T>(origin: String, future: impl Future<Output = T>) -> T {
    let url = url::Url::parse(&origin).unwrap();
    assert_eq!(url.scheme(), "http");
    assert_eq!(url.host_str(), Some("127.0.0.1"));
    assert!(url.port().is_some());
    crate::request_wait::transport::within(false, ORIGIN.scope(origin, future)).await
}
async fn transport_scope(
    State(origin): State<String>,
    request: Request<Body>,
    next: Next,
) -> Response {
    within(origin, next.run(request)).await
}
pub(crate) fn wrap_router(router: Router) -> Router {
    match selected_origin() {
        Some(origin) => router.layer(axum::middleware::from_fn_with_state(
            origin,
            transport_scope,
        )),
        None => router,
    }
}
pub(crate) fn transport_plan(
    payload: &ProviderAccountPayload,
    plan: &RequestPlan,
) -> Option<RequestPlan> {
    let origin = selected_origin()?;
    if payload.canonical_adapter() != "accio_compatible" {
        return None;
    }
    let url = url::Url::parse(&plan.url).unwrap();
    assert_eq!(
        url.origin().ascii_serialization(),
        "https://phoenix-gw.alibaba.com"
    );
    assert_eq!(url.path(), "/api/adk/llm/generateContent");
    Some(RequestPlan {
        method: plan.method.clone(),
        url: format!("{origin}{}", url.path()),
        query: plan.query.clone(),
        body: plan.body.clone(),
        response_kind: plan.response_kind,
    })
}

pub(crate) fn quota_url(endpoint: &str) -> Option<String> {
    let origin = selected_origin()?;
    let url = url::Url::parse(endpoint).unwrap();
    assert_eq!(
        url.origin().ascii_serialization(),
        "https://phoenix-gw.alibaba.com"
    );
    assert!(matches!(
        url.path(),
        "/api/entitlement/currentSubscription" | "/api/entitlement/quota"
    ));
    Some(format!("{origin}{}", url.path()))
}

#[derive(Clone)]
struct Wire {
    refresh: Arc<Mutex<Vec<Value>>>,
    quota: Arc<Mutex<Vec<String>>>,
    inference: Arc<Mutex<Vec<(HeaderMap, Value)>>>,
    delay: Duration,
    status: StatusCode,
}
async fn quota(State(wire): State<Wire>, uri: Uri) -> Json<Value> {
    let mut calls = wire.quota.lock().unwrap();
    assert!(calls.len() < 32);
    calls.push(uri.to_string());
    Json(json!({"total":1000,"remaining":1000}))
}
async fn refresh(State(wire): State<Wire>, Json(body): Json<Value>) -> (StatusCode, Json<Value>) {
    {
        let mut calls = wire.refresh.lock().unwrap();
        assert!(calls.len() < 16);
        calls.push(body);
    }
    tokio::time::sleep(wire.delay).await;
    (
        wire.status,
        Json(
            json!({"success":true,"data":{"accessToken":"server-rotated-access",
        "refreshToken":"server-rotated-refresh","expiresAt":"2099-01-01T00:00:00.123456789Z"}}),
        ),
    )
}
async fn infer(State(wire): State<Wire>, headers: HeaderMap, Json(body): Json<Value>) -> Response {
    {
        let mut calls = wire.inference.lock().unwrap();
        assert!(calls.len() < 64);
        calls.push((headers, body));
    }
    let stream = futures::stream::iter(
        [relay::text("OK 你好"), relay::end()]
            .into_iter()
            .map(|bytes| Ok::<Bytes, std::convert::Infallible>(Bytes::from(bytes))),
    );
    Response::builder()
        .header("content-type", "text/event-stream")
        .body(Body::from_stream(stream))
        .unwrap()
}
pub(super) struct Peer {
    pub origin: String,
    pub refresh: Arc<Mutex<Vec<Value>>>,
    pub quota: Arc<Mutex<Vec<String>>>,
    pub inference: Arc<Mutex<Vec<(HeaderMap, Value)>>>,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}
impl Peer {
    pub async fn start(delay: Duration, status: StatusCode) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let wire = Wire {
            refresh: Default::default(),
            quota: Default::default(),
            inference: Default::default(),
            delay,
            status,
        };
        let app = Router::new()
            .route("/oauth/refresh", post(refresh))
            .route("/api/entitlement/currentSubscription", get(quota))
            .route("/api/entitlement/quota", get(quota))
            .route("/api/adk/llm/generateContent", post(infer))
            .fallback(|| async { StatusCode::BAD_GATEWAY })
            .with_state(wire.clone());
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await
                .unwrap();
        });
        Self {
            origin,
            refresh: wire.refresh,
            quota: wire.quota,
            inference: wire.inference,
            stop: Some(stop),
            task,
        }
    }
    pub async fn wait_refresh(&self) {
        tokio::time::timeout(Duration::from_secs(10), async {
            while self.refresh.lock().unwrap().is_empty() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
    }
    pub async fn stop(mut self) {
        let _ = self.stop.take().unwrap().send(());
        tokio::time::timeout(Duration::from_secs(5), &mut self.task)
            .await
            .unwrap()
            .unwrap();
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
