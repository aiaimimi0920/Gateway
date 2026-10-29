//! Single default-group account: actual Responses HTTP/SSE and isolated Local health storage.
use super::*;
use axum::response::{IntoResponse, Response};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

const MESSAGE: &str = "Codex is at capacity, please try again later";
#[derive(Default)]
pub(super) struct Mock {
    pub(super) calls: AtomicUsize,
    pub(super) failures: AtomicUsize,
}
async fn respond(State(mock): State<Arc<Mock>>, Json(body): Json<Value>) -> Response {
    mock.calls.fetch_add(1, Ordering::SeqCst);
    if body["model"] == "gpt-fixture"
        && mock
            .failures
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
            .is_ok()
    {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error":{
            "code":"turn_rate_limit_exceeded", "message":MESSAGE}})),
        )
            .into_response();
    }
    let reply = json!({"id":"resp-fixture","object":"response","status":"completed","model":body["model"],
        "output":[{"type":"message","id":"msg-fixture","role":"assistant","status":"completed",
            "content":[{"type":"output_text","text":"OK","annotations":[]}]}],
        "usage":{"input_tokens":1,"output_tokens":1,"total_tokens":2}});
    if body["stream"] == true {
        let sse = format!("event: response.output_text.delta\ndata: {{\"type\":\"response.output_text.delta\",\"delta\":\"OK\"}}\n\nevent: response.completed\ndata: {}\n\ndata: [DONE]\n\n",
            json!({"type":"response.completed","response":reply}));
        return ([("content-type", "text/event-stream")], sse).into_response();
    }
    Json(reply).into_response()
}

pub(super) struct Fixture {
    pub(super) state: Arc<AppState>,
    pub(super) token: String,
    pub(super) mock: Arc<Mock>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Fixture {
    async fn new(failures: usize) -> Self {
        Box::pin(Self::with_wait(failures, Some(json!({"mode":"disabled"})))).await
    }
    pub(super) async fn with_wait(failures: usize, policy: Option<Value>) -> Self {
        let root = std::env::temp_dir().join(format!("gateway-capacity-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let mock = Arc::new(Mock::default());
        mock.failures.store(failures, Ordering::SeqCst);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let app = Router::new()
            .route("/v1/responses", post(respond))
            .with_state(mock.clone());
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let mut config = support::test_config();
        config.storage_mode = GatewayStorageMode::Local;
        config.console.state_dir = root.join("state");
        config.console.routes_file = root.join("routes.yaml");
        let mut document = json!({"providers":[{"id":"chatgpt-fixture","preset":"openai", "base_url":base,
            "supported_models":["gpt-fixture","gpt-other"],
            "credentials":[{"id":"only-account","api_key":"fixture-secret","supported_models":["gpt-fixture","gpt-other"]}]}],
            "account_groups":[], "model_routes":[{"pattern":"*","provider_ids":["chatgpt-fixture"]}]});
        if let Some(policy) = policy {
            document["providers"][0]["request_wait"] = policy;
        }
        std::fs::write(&config.console.routes_file, document.to_string()).unwrap();
        let state = Box::pin(neuro_gateway::runtime::build_app_state(config))
            .await
            .unwrap();
        let (status, key) = api(
            &state,
            ROOT,
            support::MANAGEMENT_TOKEN,
            Some(input(json!(["default"]))),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{key}");
        Self {
            state,
            token: key["token"].as_str().unwrap().into(),
            mock,
            task,
        }
    }
    async fn relay(
        &self,
        stream: bool,
        tools: bool,
        model: &str,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut body = json!({"model":model,"input":"fixture","stream":stream});
        if tools {
            body["tools"] =
                json!([{"type":"function","name":"fixture","parameters":{"type":"object"}}]);
        }
        let response = tokio::time::timeout(
            Duration::from_secs(10),
            Box::pin(
                build_router(self.state.clone()).oneshot(
                    Request::post("/v1/responses")
                        .header("content-type", "application/json")
                        .header("authorization", format!("Bearer {}", self.token))
                        .body(Body::from(body.to_string()))
                        .unwrap(),
                ),
            ),
        )
        .await
        .unwrap()
        .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let reply = serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| json!(String::from_utf8_lossy(&bytes)));
        (status, headers, reply)
    }
    pub(super) async fn no_persisted_cooldown(&self) {
        let rows = self
            .state
            .local_runtime
            .as_ref()
            .unwrap()
            .list_model_states(Default::default())
            .await
            .unwrap();
        assert!(
            rows.iter()
                .all(|r| r.cooldown_until.is_none() && r.status == "active"),
            "{rows:?}"
        );
    }
    pub(super) async fn close(mut self) {
        self.state.local_runtime.as_ref().unwrap().close().await;
        self.task.abort();
        let _ = (&mut self.task).await;
        // Synthetic state is retained only under the caller's isolated TEMP.
    }
}

async fn recovers_after_one_refusal(stream: bool) {
    let f = Box::pin(Fixture::new(1)).await;
    let (status, _, reply) = Box::pin(f.relay(stream, false, "gpt-fixture")).await;
    assert_eq!(status, StatusCode::OK, "{reply}");
    assert!(reply.to_string().contains("OK"), "{reply}");
    assert_eq!(f.mock.calls.load(Ordering::SeqCst), 2);
    f.no_persisted_cooldown().await;
    f.close().await;
}
#[tokio::test]
async fn capacity_responses_json_recovers_without_restart() {
    Box::pin(recovers_after_one_refusal(false)).await;
}
#[tokio::test]
async fn capacity_first_nonsticky_responses_stream_recovers_without_restart() {
    Box::pin(recovers_after_one_refusal(true)).await;
}

#[tokio::test]
async fn capacity_twice_returns_original_error_then_recovers_in_same_state() {
    let f = Box::pin(Fixture::new(100)).await;
    let (status, headers, reply) = Box::pin(f.relay(false, false, "gpt-fixture")).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{reply}");
    assert_eq!(reply["error"]["code"], "turn_rate_limit_exceeded");
    assert_eq!(reply["error"]["message"], MESSAGE);
    assert_eq!(headers["retry-after"], "4");
    assert_eq!(
        f.mock.calls.load(Ordering::SeqCst),
        2,
        "one automatic retry only"
    );
    let (status, _, denied) = Box::pin(f.relay(false, false, "gpt-fixture")).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{denied}");
    assert_eq!(denied["error"]["message"], MESSAGE);
    assert_eq!(
        f.mock.calls.load(Ordering::SeqCst),
        2,
        "local backoff is not another upstream call"
    );
    f.no_persisted_cooldown().await;
    f.mock.failures.store(0, Ordering::SeqCst);
    assert_eq!(
        Box::pin(f.relay(false, false, "gpt-other")).await.0,
        StatusCode::OK
    );
    tokio::time::sleep(Duration::from_millis(4_100)).await;
    assert_eq!(
        Box::pin(f.relay(false, false, "gpt-fixture")).await.0,
        StatusCode::OK
    );
    f.mock.failures.store(1, Ordering::SeqCst);
    let (status, headers, _) = Box::pin(f.relay(false, true, "gpt-fixture")).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        headers["retry-after"], "2",
        "complete success resets the level"
    );
    assert_eq!(
        f.mock.calls.load(Ordering::SeqCst),
        5,
        "tool request has no automatic retry"
    );
    f.no_persisted_cooldown().await;
    f.close().await;
}
