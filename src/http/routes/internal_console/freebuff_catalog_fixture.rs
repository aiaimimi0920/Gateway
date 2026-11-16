//! Bounded synthetic upstream; random loopback port and awaited graceful shutdown.
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, Method, Uri},
    response::IntoResponse,
    Json, Router,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
type Hits = Arc<Mutex<Vec<(String, String, HeaderMap, Vec<u8>)>>>;
type SessionReply = (Value, u16, Option<Arc<dyn Fn() + Send + Sync>>);
#[derive(Clone)]
struct Data {
    hits: Hits,
    body: Value,
    status: u16,
    registration_status: Option<u16>,
    after_registration: Option<Arc<dyn Fn() + Send + Sync>>,
    session: Option<SessionReply>,
}
pub(super) struct Peer {
    pub origin: String,
    pub hits: Hits,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
}
impl Peer {
    pub async fn start(body: Value, status: u16) -> Self {
        Self::start_inner(body, status, None, None, None).await
    }
    pub async fn device(status: u16, after: Option<Arc<dyn Fn() + Send + Sync>>) -> Self {
        Self::start_inner(catalog(), 200, Some(status), after, None).await
    }
    pub async fn session(
        body: Value,
        status: u16,
        after: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> Self {
        Self::start_inner(catalog(), 200, Some(200), None, Some((body, status, after))).await
    }
    async fn start_inner(
        body: Value,
        status: u16,
        registration_status: Option<u16>,
        after_registration: Option<Arc<dyn Fn() + Send + Sync>>,
        session: Option<SessionReply>,
    ) -> Self {
        let hits = Hits::default();
        let data = Data {
            hits: hits.clone(),
            body,
            status,
            registration_status,
            after_registration,
            session,
        };
        let app = Router::new()
            .fallback(
                |State(data): State<Data>,
                 method: Method,
                 uri: Uri,
                 headers: HeaderMap,
                 body: Bytes| async move {
                    let mut hits = data.hits.lock().unwrap();
                    assert!(hits.len() < 32);
                    hits.push((method.to_string(), uri.to_string(), headers, body.to_vec()));
                    drop(hits);
                    if (method == Method::GET && uri.path() == "/api/v1/freebuff/session")
                        || (method == Method::DELETE
                            && uri.path() == "/api/v1/freebuff/session/attempt")
                    {
                        if let Some((body, status, after)) = data.session {
                            if let Some(after) = after {
                                after();
                            }
                            return (
                                axum::http::StatusCode::from_u16(status).unwrap(),
                                Json(body),
                            )
                                .into_response();
                        }
                    }
                    if method == Method::POST {
                        if let Some(status) = data.registration_status {
                            if let Some(after) = data.after_registration {
                                after();
                            }
                            return (
                                axum::http::StatusCode::from_u16(status).unwrap(),
                                Json(json!({"keyId":"device-fixture"})),
                            )
                                .into_response();
                        }
                    }
                    (
                        axum::http::StatusCode::from_u16(data.status).unwrap(),
                        Json(data.body),
                    )
                        .into_response()
                },
            )
            .layer(DefaultBodyLimit::max(32768))
            .with_state(data);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let (stop, stopped) = tokio::sync::oneshot::channel();
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
            hits,
            stop: Some(stop),
            task,
        }
    }
    pub async fn close(mut self) {
        self.stop.take().unwrap().send(()).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(3), &mut self.task)
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
pub(super) fn catalog() -> Value {
    let row = |key: &str, access: &str| {
        json!({"key":key,"handle":format!("fbm1.{key}"),"displayName":key,
        "tagline":"synthetic", "warning":null, "badges":[],"multimodal":false,"premium":false,
        "dataUse":"service","access":access,"sortOrder":0})
    };
    let mut future = row("future", "open");
    future["opensAt"] = json!(1001);
    json!({"protocol":1,"version":"test","issuedAt":1000,"refreshAt":2000,"plansUrl":"https://invalid.test",
        "fetchId":"synthetic-fetch", "rows":[row("open","open"),row("locked","locked"),future]})
}
