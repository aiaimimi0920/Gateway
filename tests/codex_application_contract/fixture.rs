//! Isolated local-data fixture and bounded loopback upstream.
use axum::{
    extract::{
        ws::{Message, WebSocketUpgrade},
        State,
    },
    http::HeaderMap,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use neuro_gateway::{
    access_balance::AccessBalanceStore, config::GatewayStorageMode, state::AppState,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct Capture {
    pub compact: Vec<String>,
    pub frames: Vec<String>,
    pub auth: Vec<String>,
    pub connections: usize,
    pub ws_headers: Vec<HeaderMap>,
}

async fn compact(
    State(capture): State<Arc<Mutex<Capture>>>,
    headers: HeaderMap,
    body: String,
) -> Json<Value> {
    let mut capture = capture.lock().unwrap();
    capture.compact.push(body);
    capture
        .auth
        .push(headers["authorization"].to_str().unwrap().into());
    Json(
        json!({"id":"compact-fixture","object":"response.compaction","output":[
        {"type":"compaction","encrypted_content":"opaque-fixture"}],
        "usage":{"input_tokens":2,"output_tokens":3,"total_tokens":5}}),
    )
}

async fn websocket(
    State(capture): State<Arc<Mutex<Capture>>>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    {
        let mut c = capture.lock().unwrap();
        c.connections += 1;
        c.ws_headers.push(headers.clone());
        c.auth
            .push(headers["authorization"].to_str().unwrap().into());
    }
    if headers
        .get("session-id")
        .is_some_and(|value| value == "slow-handshake")
    {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
    ws.on_upgrade(move |mut socket| async move {
        while let Some(Ok(Message::Text(text))) = socket.recv().await {
            let request: Value = serde_json::from_str(&text).unwrap();
            capture.lock().unwrap().frames.push(text);
            if request["input"] == "fail" {
                let event = json!({"type":"error","error":{"type":"fixture_error","code":"fixture_failed","message":"upstream fixture rejected"}});
                if socket.send(Message::Text(event.to_string())).await.is_err() { break; }
                continue;
            }
            if request["input"] == "wait" {
                if socket.send(Message::Text(json!({"type":"response.created","response":{"id":"waiting"}}).to_string())).await.is_err() { break; }
                continue;
            }
            let event = json!({"type":"response.completed","response":{"id":"resp-fixture",
                "status":"completed","object":"response","model":"gpt-fixture",
                "output":[{"type":"reasoning","encrypted_content":"opaque-fixture"}],
                "usage":{"input_tokens":2,"output_tokens":3,"total_tokens":5}}});
            if socket.send(Message::Text(event.to_string())).await.is_err() {
                break;
            }
        }
    })
}

pub struct Fixture {
    pub state: Arc<AppState>,
    pub base: String,
    pub http: reqwest::Client,
    pub capture: Arc<Mutex<Capture>>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}
impl Fixture {
    pub async fn start() -> Self {
        let capture = Arc::new(Mutex::new(Capture::default()));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream = listener.local_addr().unwrap();
        let app = Router::new()
            .route("/v1/responses", get(websocket))
            .route("/v1/responses/compact", post(compact))
            .with_state(capture.clone());
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let root = std::env::temp_dir().join(format!(
            "gateway-codex-application-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let mut config = super::support::test_config();
        config.storage_mode = GatewayStorageMode::Local;
        config.upstream_timeout_secs = 10;
        config.console.state_dir = root.join("state");
        config.console.routes_file = root.join("routes.yaml");
        std::fs::write(&config.console.routes_file, format!("providers:\n  - id: fixture\n    base_url: http://{upstream}\n    api_key: selected-fixture\n    request_client_profile: codex_cli\n    supported_models: [gpt-fixture]\nmodel_routes: []\n")).unwrap();
        let state = Box::pin(neuro_gateway::runtime::build_app_state(config))
            .await
            .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let router = neuro_gateway::http::router::build_router(state.clone());
        let gateway = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        Self {
            state,
            base,
            http: reqwest::Client::builder()
                .no_proxy()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap(),
            capture,
            tasks: vec![task, gateway],
        }
    }
    async fn management(&self, path: &str, body: Value) -> Value {
        let response = self
            .http
            .post(format!("{}{path}", self.base))
            .header("x-management-token", super::support::MANAGEMENT_TOKEN)
            .json(&body)
            .send()
            .await
            .unwrap();
        let status = response.status();
        let text = response.text().await.unwrap();
        assert!(status.is_success(), "{status} {text}");
        serde_json::from_str(&text).unwrap()
    }
    pub async fn key(&self, messages: i64) -> (String, String) {
        let key = self.management("/v1/internal/gateway/access/keys", json!({"ownerType":"user",
            "ownerId":"fixture","resolvedProjectId":"local","resolvedTenantId":"local","keyKind":"normal",
            "publicKeyPrefix":"sk-gw","displayName":"codex","metadata":{"models":["gpt-fixture"]}})).await;
        let id = key["id"].as_str().unwrap().to_owned();
        self.adjust(&id, messages).await;
        (id, key["token"].as_str().unwrap().to_owned())
    }
    pub async fn adjust(&self, id: &str, count: i64) {
        self.management(&format!("/v1/internal/gateway/access/keys/{id}/balances/adjust"),
            json!({"balanceMode":"message_prepaid","totalMessages":count,"remainingMessages":count})).await;
    }
    pub async fn balance(&self, id: &str) -> i64 {
        AccessBalanceStore::from_state(&self.state)
            .unwrap()
            .get(id)
            .await
            .unwrap()
            .unwrap()
            .remaining_messages
            .unwrap()
    }
    pub async fn wait_balance(&self, id: &str, count: i64) {
        for _ in 0..100 {
            if self.balance(id).await == count {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        assert_eq!(self.balance(id).await, count);
    }
    pub async fn stop(self) {
        for task in self.tasks {
            task.abort();
            let _ = task.await;
        }
        self.state.local_runtime.as_ref().unwrap().close().await;
    }
}
