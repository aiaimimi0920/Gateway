use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode, Uri};
use axum::routing::post;
use axum::{Json, Router};
use neuro_gateway::console::{ConsoleConfig, ConsoleConfigValues};
use neuro_gateway::pipeline::PipelineContext;
use neuro_gateway::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
};
use neuro_gateway::routing::candidate::{ProviderExecutionMode, RouteCandidate};
use neuro_gateway::routing::config::RouteConfigStore;
use neuro_gateway::state::AppState;
use serde_json::{json, Value};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::timeout;

pub const DEADLINE: Duration = Duration::from_secs(10);
pub const JSON_REPLY: &str = r#"{"id":"upstream-id","object":"chat.completion","model":"upstream-model","choices":[{"index":0,"message":{"role":"assistant","content":"local upstream reply"},"finish_reason":"stop"}],"usage":{"prompt_tokens":7,"completion_tokens":3,"total_tokens":10}}"#;
pub const SSE_REPLY: &str = concat!(
    "data: {\"id\":\"stream-id\",\"model\":\"upstream-model\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"local stream reply\"},\"finish_reason\":null}]}\n\n",
    "data: {\"id\":\"stream-id\",\"model\":\"upstream-model\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":7,\"completion_tokens\":3,\"total_tokens\":10}}\n\n",
    "data: [DONE]\n\n",
);

#[derive(Clone)]
pub struct ObservedRequest {
    pub path: String,
    pub fixture_auth: bool,
    pub body: Value,
}

#[derive(Clone)]
struct Exchange {
    seen: Arc<Mutex<Vec<ObservedRequest>>>,
    status: StatusCode,
    content_type: &'static str,
    body: &'static str,
    release_response: Option<tokio::sync::watch::Receiver<bool>>,
}

async fn respond(
    State(exchange): State<Exchange>,
    uri: Uri,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (
    StatusCode,
    [(header::HeaderName, &'static str); 2],
    &'static str,
) {
    {
        let mut seen = exchange.seen.lock().unwrap();
        assert!(seen.len() < 8, "unexpected unbounded provider attempts");
        seen.push(ObservedRequest {
            path: uri.path().to_string(),
            fixture_auth: headers
                .get(header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                == Some("Bearer send-test-token"),
            body,
        });
    }
    if let Some(mut release) = exchange.release_response {
        // The fixture owns and releases pending responses before shutdown; no orphan handler.
        release.wait_for(|released| *released).await.unwrap();
    }
    (
        exchange.status,
        [
            (header::CONTENT_TYPE, exchange.content_type),
            (header::CONNECTION, "close"),
        ],
        exchange.body,
    )
}

pub struct Upstream {
    pub base_url: String,
    seen: Arc<Mutex<Vec<ObservedRequest>>>,
    shutdown: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
    release_response: tokio::sync::watch::Sender<bool>,
}

impl Upstream {
    pub async fn start(status: StatusCode, streaming: bool, body: &'static str) -> Self {
        Self::start_with_pending(status, streaming, body, false).await
    }

    pub async fn start_pending(streaming: bool) -> Self {
        Self::start_with_pending(StatusCode::OK, streaming, "", true).await
    }

    async fn start_with_pending(
        status: StatusCode,
        streaming: bool,
        body: &'static str,
        pending: bool,
    ) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let (release_response, receiver) = tokio::sync::watch::channel(false);
        let exchange = Exchange {
            seen: Arc::clone(&seen),
            status,
            content_type: if streaming {
                "text/event-stream"
            } else {
                "application/json"
            },
            body,
            release_response: pending.then_some(receiver),
        };
        let router = Router::new()
            .route("/v1/chat/completions", post(respond))
            .with_state(exchange);
        let (shutdown, receiver) = oneshot::channel();
        let task = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _ = receiver.await;
                })
                .await
                .unwrap();
        });
        Self {
            base_url,
            seen,
            shutdown: Some(shutdown),
            task,
            release_response,
        }
    }

    pub fn requests(&self) -> Vec<ObservedRequest> {
        self.seen.lock().unwrap().clone()
    }

    pub async fn finish(mut self) {
        let _ = self.release_response.send(true);
        self.shutdown.take().unwrap().send(()).unwrap();
        timeout(Duration::from_secs(2), &mut self.task)
            .await
            .expect("upstream shutdown")
            .unwrap();
    }
}

impl Drop for Upstream {
    fn drop(&mut self) {
        let _ = self.release_response.send(true);
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.task.abort();
    }
}

struct ConsoleDirectory(PathBuf);

impl Drop for ConsoleDirectory {
    fn drop(&mut self) {
        // Only remove the unique directory created by this fixture, never an inherited path.
        let parent = std::env::temp_dir().canonicalize().unwrap();
        let resolved = self.0.canonicalize().unwrap();
        assert_eq!(resolved.parent(), Some(parent.as_path()));
        assert!(resolved
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("gateway-send-runtime-"));
        std::fs::remove_dir_all(resolved).unwrap();
    }
}

pub struct TestState {
    pub state: Arc<AppState>,
    _directory: ConsoleDirectory,
}

impl TestState {
    pub fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("gateway-send-runtime-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();
        let directory = ConsoleDirectory(path.clone());
        let console = ConsoleConfig::from_values(ConsoleConfigValues {
            state_dir: Some(path.clone()),
            routes_file: Some(path.join("routes.yaml")),
            ..Default::default()
        })
        .unwrap();
        let state = super::support::build_test_app_state(
            super::config::test_config(console),
            RouteConfigStore::new(),
            None,
        );
        Self {
            state,
            _directory: directory,
        }
    }

    pub async fn finish(self) {
        // No-session/no-database finalization tasks must release their state captures.
        timeout(Duration::from_secs(2), async {
            while Arc::strong_count(&self.state) != 1 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("detached finalization tasks did not settle");
    }
}

pub fn candidate(upstream: &Upstream, label: &str) -> RouteCandidate {
    RouteCandidate {
        provider_account_id: format!("cred:send-{label}-{}", uuid::Uuid::new_v4()),
        provider_credential_id: None,
        label: label.to_string(),
        payload: serde_json::from_value(json!({
            "adapter": "openai_compatible", "base_url": upstream.base_url,
            "api_key": "send-test-token", "execution_mode": "direct_http"
        }))
        .unwrap(),
        protocol_family: "openai".to_string(),
        protocol_profile: "openai".to_string(),
        supported_protocol_families: vec!["openai_chat".to_string()],
        adapter: "openai_compatible".to_string(),
        model_alias: None,
        upstream_model: Some("upstream-model".to_string()),
        resolved_execution_mode: ProviderExecutionMode::DirectHttp,
        priority: 100,
        weight: 1,
        failure_count: 0,
        cooldown_until: None,
        routing_score: None,
        routing_health_weight: None,
        routing_capacity_weight: None,
        routing_degraded: None,
        routing_breaker_open: None,
        routing_degradation_reasons: Vec::new(),
    }
}

pub fn context(stream: bool, candidates: Vec<RouteCandidate>) -> PipelineContext {
    let request = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ChatCompletions,
        requested_model: Some("caller-alias".to_string()),
        stream,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "local request".to_string(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
        }],
        tools: Vec::new(),
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({}),
        previous_response_id: None,
        explicit_session_key: None,
        extra: Default::default(),
    };
    let mut ctx = PipelineContext::new(request, None);
    ctx.candidates = candidates;
    ctx
}
