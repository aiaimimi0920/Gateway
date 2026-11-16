//! Reflects the actual server-generated device, not a fixed fixture fingerprint or a relaxed validator.
use super::*;
use axum::{
    extract::State,
    http::{HeaderMap, Method, Uri},
    routing::{get, post},
    Json, Router,
};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Audit {
    devices: Vec<String>,
    statuses: usize,
}
pub(super) struct Peer {
    pub origin: String,
    audit: Arc<Mutex<Audit>>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<()>>,
}
async fn code(
    State(audit): State<Arc<Mutex<Audit>>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    assert!(headers.get("authorization").is_none() && headers.get("cookie").is_none());
    assert_eq!(body.as_object().unwrap().len(), 1);
    let device = body["fingerprintId"].as_str().unwrap().to_owned();
    assert!(
        uuid::Uuid::parse_str(&device).is_ok(),
        "Normal UI must let the server choose its device"
    );
    let mut audit = audit.lock().unwrap();
    assert!(audit.devices.len() < 4, "Unexpected login resubmission");
    audit.devices.push(device);
    let generation = audit.devices.len();
    Json(
        json!({"loginUrl":format!("https://freebuff.com/login?code=synthetic-browser-{generation}"),
        "fingerprintHash":format!("synthetic-hash-{generation}+/="),"expiresAt":format!("opaque expiry {generation}+/=")}),
    )
}
async fn status(
    State(audit): State<Arc<Mutex<Audit>>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
) -> Json<Value> {
    assert_eq!(method, Method::GET);
    assert!(headers.get("authorization").is_none() && headers.get("cookie").is_none());
    let url = url::Url::parse(&format!("http://127.0.0.1{uri}")).unwrap();
    let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(query.len(), 3);
    let mut audit = audit.lock().unwrap();
    let generation = audit.devices.len();
    let device = audit.devices.last().unwrap().clone();
    assert_eq!(query["fingerprintId"], device);
    assert_eq!(
        query["fingerprintHash"],
        format!("synthetic-hash-{generation}+/=")
    );
    assert_eq!(query["expiresAt"], format!("opaque expiry {generation}+/="));
    audit.statuses += 1;
    Json(
        json!({"user":{"id":"synthetic-user","email":"synthetic-user@example.invalid","name":null,
        "authToken":format!("synthetic-freebuff-token-{generation}"),"fingerprintId":device,
        "fingerprintHash":format!("synthetic-hash-{generation}+/=")}}),
    )
}
impl Peer {
    pub async fn start() -> Self {
        let audit = Arc::new(Mutex::new(Audit::default()));
        let app = Router::new()
            .route("/api/auth/cli/code", post(code))
            .route("/api/auth/cli/status", get(status))
            .with_state(audit.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let (shutdown, stop) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = stop.await;
                })
                .await
                .unwrap();
        });
        Self {
            origin,
            audit,
            shutdown: Some(shutdown),
            task: Some(task),
        }
    }
    pub fn devices(&self) -> Vec<String> {
        self.audit.lock().unwrap().devices.clone()
    }
    pub fn counts(&self) -> (usize, usize) {
        let audit = self.audit.lock().unwrap();
        (audit.devices.len(), audit.statuses)
    }
    pub async fn stop(mut self) {
        self.shutdown.take().unwrap().send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(5), self.task.as_mut().unwrap())
            .await
            .unwrap()
            .unwrap();
        self.task.take();
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        if let Some(stop) = self.shutdown.take() {
            let _ = stop.send(());
        }
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
