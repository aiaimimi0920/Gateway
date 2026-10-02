//! Loopback archive fixture with a barrier before the verified read-back completes.
use axum::{
    body::Body,
    extract::{Request, State},
    response::Response,
    Router,
};
use std::sync::{Arc, Mutex};
use tokio::{sync::Notify, task::JoinHandle};

#[derive(Clone, Default)]
struct ArchiveState {
    object: Arc<Mutex<Option<(String, Vec<u8>)>>>,
    methods: Arc<Mutex<Vec<String>>>,
    readback_started: Arc<Notify>,
    allow_readback: Arc<Notify>,
}

pub(super) struct ArchiveServer {
    pub endpoint: String,
    state: ArchiveState,
    task: JoinHandle<()>,
}

impl ArchiveServer {
    pub async fn start() -> Self {
        let state = ArchiveState::default();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let router = Router::new().fallback(serve).with_state(state.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self {
            endpoint,
            state,
            task,
        }
    }

    pub async fn wait_for_readback(&self) {
        self.state.readback_started.notified().await;
    }

    pub fn release_readback(&self) {
        self.state.allow_readback.notify_one();
    }

    pub fn verify_archive(&self) -> anyhow::Result<()> {
        let object = self.state.object.lock().unwrap();
        let (path, bytes) = object
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("no archive PUT"))?;
        let record: serde_json::Value = serde_json::from_slice(bytes)?;
        anyhow::ensure!(path.starts_with("/dav/pool/gateway-pool/"));
        anyhow::ensure!(path.contains("/archive/") && path.ends_with(".json"));
        anyhow::ensure!(record["schemaVersion"] == 2);
        anyhow::ensure!(record["sourceRevision"]
            .as_str()
            .is_some_and(|r| r.starts_with('r')));
        anyhow::ensure!(record["providerId"] == "p" && record["credentialId"] == "a");
        anyhow::ensure!(record["credential"]["api_key"] == "fixture-a");
        anyhow::ensure!(record["reason"] == "permanent_driver_rejection");
        anyhow::ensure!(
            *self.state.methods.lock().unwrap() == ["POST", "GET", "PUT", "GET"],
            "archive must be conditionally created and read back before removal"
        );
        Ok(())
    }
}

impl Drop for ArchiveServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn serve(State(state): State<ArchiveState>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    let method = parts.method.as_str();
    let path = parts.uri.path().to_owned();
    state.methods.lock().unwrap().push(method.to_owned());
    let response = |status, bytes| {
        Response::builder()
            .status(status)
            .body(Body::from(bytes))
            .unwrap()
    };
    match (method, path.as_str()) {
        ("POST", "/driver") => response(200, br#"{"credentials":[],"prune":[{"credential_id":"a","classification":"permanent_auth_failure"}]}"#.to_vec()),
        ("PUT", _) => {
            if parts.headers.get("if-none-match").and_then(|value| value.to_str().ok()) != Some("*") {
                return response(400, Vec::new());
            }
            let bytes = axum::body::to_bytes(body, 1024 * 1024).await.unwrap().to_vec();
            let mut object = state.object.lock().unwrap();
            if object.is_some() {
                return response(412, Vec::new());
            }
            *object = Some((path, bytes));
            response(201, Vec::new())
        }
        ("GET", _) => {
            let object = state.object.lock().unwrap().clone();
            let Some((stored_path, bytes)) = object.filter(|(stored_path, _)| stored_path == &path) else {
                return response(404, Vec::new());
            };
            debug_assert_eq!(stored_path, path);
            // The object exists, but the production archival future has not verified it yet.
            state.readback_started.notify_one();
            state.allow_readback.notified().await;
            response(200, bytes)
        }
        _ => response(500, Vec::new()),
    }
}
