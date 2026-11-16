//! One bounded synthetic catalog/device/price/admission peer, with explicit awaited shutdown.
use super::*;
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, Uri},
    response::IntoResponse,
    Json, Router,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};
#[derive(Default)]
struct Data {
    hits: Mutex<Vec<(Method, String, HeaderMap)>>,
    price: AtomicUsize,
}
pub(super) struct Peer {
    pub origin: String,
    data: Arc<Data>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
}
impl Peer {
    pub async fn new() -> Self {
        let data = Arc::new(Data {
            price: AtomicUsize::new(10),
            ..Default::default()
        });
        let app = Router::new().fallback(|State(data): State<Arc<Data>>, method: Method, uri: Uri, headers: HeaderMap, bytes: Bytes| async move {
            { let mut hits = data.hits.lock().unwrap(); assert!(hits.len() < 128); hits.push((method.clone(),uri.path().to_owned(),headers.clone())); }
            let value = match (method,uri.path()) {
                (Method::GET,"/api/v1/freebuff/models") => catalog_fixture::catalog(),
                (Method::POST,"/api/v1/freebuff/device-keys") => json!({"keyId":"ephemeral-device"}),
                (Method::GET,"/api/v1/freebuff/session") => json!({"status":"none","freebucks":{"balance":20,"daily":{"remaining":8},
                    "wallet":{"balance":12},"prices":{"open":data.price.load(Ordering::SeqCst)}}}),
                (Method::POST,"/api/v1/freebuff/session/admission") => {
                    assert!(bytes.is_empty());
                    assert_eq!(headers["authorization"],"Bearer synthetic-b");
                    assert!(headers.contains_key("x-freebuff-device-sig"));
                    assert_eq!(headers["x-freebuff-model"],"fbm1.open");
                    assert_eq!(headers["x-freebuff-wallet-spend-limit"],"session");
                    let now = time::OffsetDateTime::now_utc();
                    json!({"status":"active","instanceId":headers["x-freebuff-instance-id"].to_str().unwrap(),"model":"open","accessTier":"full",
                        "admittedAt":(now-time::Duration::seconds(1)).format(&time::format_description::well_known::Rfc3339).unwrap(),
                        "expiresAt":(now+time::Duration::hours(1)).format(&time::format_description::well_known::Rfc3339).unwrap(),"remainingMs":3600000})
                },
                _ => return StatusCode::NOT_FOUND.into_response(),
            }; Json(value).into_response()
        }).layer(axum::extract::DefaultBodyLimit::max(16384)).with_state(data.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await
                .unwrap();
        });
        Self {
            origin,
            data,
            stop: Some(stop),
            task,
        }
    }
    pub fn hits(&self) -> usize {
        self.data.hits.lock().unwrap().len()
    }
    pub fn purchases(&self) -> usize {
        self.data
            .hits
            .lock()
            .unwrap()
            .iter()
            .filter(|(method, path, _)| method == Method::POST && path.ends_with("/admission"))
            .count()
    }
    pub fn price(&self, price: usize) {
        self.data.price.store(price, Ordering::SeqCst);
    }
    pub async fn close(mut self) {
        self.stop.take().unwrap().send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(3), &mut self.task)
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
