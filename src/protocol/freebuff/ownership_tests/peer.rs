//! Bounded synthetic legacy wire peer. This does not emulate current paid admission or a typed login.
use super::*;
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, Uri},
    Json, Router,
};
use std::sync::Mutex;
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};

#[derive(Clone)]
pub(super) struct Hit {
    pub auth: String,
    pub path: String,
    pub body: Value,
}

#[derive(Clone)]
struct StateData {
    disabled: bool,
    hits: Arc<Mutex<Vec<Hit>>>,
}

pub(super) struct Peer {
    base: String,
    state: StateData,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}

async fn respond(
    State(state): State<StateData>,
    uri: Uri,
    headers: HeaderMap,
    bytes: Bytes,
) -> Json<Value> {
    let auth = headers
        .get("authorization")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let suffix = match auth.as_str() {
        "Bearer synthetic-old" => "old",
        "Bearer synthetic-new" => "new",
        _ => panic!("unexpected synthetic authorization"),
    };
    let body: Value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    let response = match uri.path() {
        "/api/v1/freebuff/session" if state.disabled => json!({"status":"disabled"}),
        "/api/v1/freebuff/session" => {
            json!({"status":"active","instanceId":format!("instance-{suffix}"),"remainingMs":60000})
        }
        "/api/v1/agent-runs" if body["action"] == "START" => {
            json!({"runId":format!("run-{suffix}")})
        }
        "/api/v1/agent-runs" if body["action"] == "FINISH" => json!({}),
        "/api/v1/chat/completions" => {
            json!({"id":"chat-synthetic","model":"z-ai/glm-5.1","choices":[{"index":0,"message":{"role":"assistant","content":"hello"},"finish_reason":"stop"}]})
        }
        _ => panic!("unexpected synthetic request"),
    };
    let mut hits = state.hits.lock().unwrap();
    assert!(hits.len() < 16, "bounded synthetic request count");
    hits.push(Hit {
        auth,
        path: uri.path().into(),
        body,
    });
    Json(response)
}

impl Peer {
    pub(super) async fn start(disabled: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let state = StateData {
            disabled,
            hits: Arc::new(Mutex::new(Vec::new())),
        };
        let app = Router::new()
            .fallback(respond)
            .layer(DefaultBodyLimit::max(32768))
            .with_state(state.clone());
        let (stop, shutdown) = oneshot::channel();
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown.await;
                })
                .await
                .unwrap();
        });
        Self {
            base,
            state,
            stop: Some(stop),
            task,
        }
    }

    pub(super) fn payload(&self) -> ProviderAccountPayload {
        let mut payload = make_payload();
        payload.base_url = self.base.clone();
        payload.credential_id = Some(format!("ownership-{}", self.base));
        payload.api_key = "synthetic-old".into();
        payload
    }

    pub(super) fn client(&self) -> Client {
        Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap()
    }

    pub(super) async fn close(mut self) -> Vec<Hit> {
        self.stop.take().unwrap().send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(3), &mut self.task)
            .await
            .unwrap()
            .unwrap();
        self.state.hits.lock().unwrap().clone()
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        // Failure-path fallback only; normal tests stop and join the HTTP owner explicitly.
        self.task.abort();
    }
}
