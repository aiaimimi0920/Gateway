use super::{
    support,
    upstream::{self, Control},
};
use axum::{
    body::Body,
    http::{HeaderMap, Request, StatusCode},
    response::Response,
    routing::post,
    Router,
};
use http_body_util::BodyExt;
use neuro_gateway::{
    config::{Config, GatewayStorageMode},
    http::router::build_router,
    routing::config::RouteConfigYaml,
    state::AppState,
};
use serde_json::{json, Value};
use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};
use tower::ServiceExt;

pub const KEYS: &str = "/v1/internal/gateway/access/keys";
pub const PRICING: &str = "/v1/internal/gateway/provider-accounts/cash-fixture/model-pricing";

pub struct Fixture {
    pub state: Arc<AppState>,
    pub config: Config,
    pub document: RouteConfigYaml,
    pub control: Arc<Control>,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl Fixture {
    pub fn new() -> std::pin::Pin<Box<impl std::future::Future<Output = Self>>> {
        // Keep the sizeable application-construction future off the Windows test thread stack.
        Box::pin(Self::with_adapter("openai_compatible"))
    }

    pub async fn with_adapter(adapter: &str) -> Self {
        let root = std::env::temp_dir().join(format!("gateway-cash-http-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let control = Arc::new(Control::default());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = Router::new()
            .route("/v1/chat/completions", post(upstream::reply))
            .route("/v1/messages", post(upstream::anthropic))
            .with_state(control.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let document: RouteConfigYaml = serde_json::from_value(json!({
            "providers":[{"id":"cash-fixture", "adapter":adapter,
                "protocol_family":if adapter == "anthropic_compatible" {"anthropic"} else {"openai"},
                "base_url":format!("http://{address}"),"supported_models":["cash-model","missing","gpt-5.4"],
                "credentials":[{"id":"account-a","api_key":"cash-fixture-upstream-secret"}]}],
            "account_groups":[
                {"id":"cheap","name":"Cheap","billing_multiplier":1.5,"provider_credential_ids":["account-a"]},
                {"id":"dear","name":"Dear","billing_multiplier":2.0,"provider_credential_ids":["account-a"]},
                {"id":"unauthorized","name":"Other","billing_multiplier":0.1,"provider_credential_ids":["account-a"]},
                {"id":"disabled","name":"Disabled","enabled":false,"billing_multiplier":0.0,"provider_credential_ids":["account-a"]}],
            "aliases":{"cash-alias":"cash-model"},"model_routes":[]
        })).unwrap();
        let mut config = support::test_config();
        config.storage_mode = GatewayStorageMode::Local;
        config.upstream_timeout_secs = 10;
        config.console.state_dir = root.join("state");
        config.console.routes_file = root.join("routes.yaml");
        std::fs::write(
            &config.console.routes_file,
            serde_yaml::to_string(&document).unwrap(),
        )
        .unwrap();
        let state = Box::pin(neuro_gateway::runtime::build_app_state(config.clone()))
            .await
            .unwrap();
        Self {
            state,
            config,
            document,
            control,
            task: Some(task),
        }
    }

    pub async fn manage(&self, path: &str, body: Option<Value>) -> (StatusCode, HeaderMap, Value) {
        Box::pin(api(
            &self.state,
            path,
            support::MANAGEMENT_TOKEN,
            body,
            None,
        ))
        .await
    }

    pub async fn price(&self, prompt: i64, completion: i64, multiplier: &str) {
        let (status, _, body) = self
            .manage(
                PRICING,
                Some(json!({"entries":[{"model":"cash-model",
            "promptMicrosPer1kTokens":prompt,"completionMicrosPer1kTokens":completion}],
            "accountBillingMultipliers":{"account-a":multiplier}})),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }

    pub async fn key(&self, limit: i64) -> Value {
        let (status, _, body) = self.manage(KEYS, Some(key_input(limit))).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }

    pub async fn limit(&self, key: &Value, limit: i64) {
        let (status, _, body) = self
            .manage(
                &format!("{KEYS}/{}", key["id"].as_str().unwrap()),
                Some(key_input(limit)),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }

    pub async fn balance(&self, key: &Value) -> Value {
        let (status, _, body) = self
            .manage(
                &format!("{KEYS}/{}/balance", key["id"].as_str().unwrap()),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body["cash"].clone()
    }

    pub async fn ledger(&self, key: &Value) -> Value {
        let (status, headers, body) = self
            .manage(
                &format!("{KEYS}/{}/cash-ledger", key["id"].as_str().unwrap()),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(headers["cache-control"], "no-store");
        assert!(!body.to_string().contains("cash-fixture-upstream-secret"));
        assert!(!body.to_string().contains(key["token"].as_str().unwrap()));
        body
    }

    pub async fn calls(&self, count: usize) {
        tokio::time::timeout(Duration::from_secs(5), async {
            while self.control.calls.load(Ordering::SeqCst) < count {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("upstream was not reached");
    }

    pub async fn finalized(&self, key: &Value, expected: usize) -> Value {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let ledger = self.ledger(key).await;
                if ledger.as_array().unwrap().len() == expected
                    && ledger
                        .as_array()
                        .unwrap()
                        .iter()
                        .all(|row| row["status"] != "reserved")
                {
                    return ledger;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("cash finalizers did not settle")
    }

    pub async fn restart(&mut self) {
        self.state.local_runtime.as_ref().unwrap().close().await;
        self.state = neuro_gateway::runtime::build_app_state(self.config.clone())
            .await
            .unwrap();
    }

    pub async fn finish(mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
            let _ = task.await;
        }
        self.state.local_runtime.as_ref().unwrap().close().await;
        // Preserve the isolated SQLite artifact as evidence; no live data is ever opened.
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}

pub fn key_input(limit: i64) -> Value {
    json!({"ownerType":"user","ownerId":"fixture","resolvedProjectId":"local","resolvedTenantId":"local",
        "keyKind":"normal","publicKeyPrefix":"sk-gw","displayName":"cash-key",
        "metadata":{"accountGroupIds":["cheap","dear"]},
        "quota":{"mode":"cash_prepaid","limit":limit,"currency":"USD"}})
}

pub fn request(stream: bool) -> Value {
    json!({"model":"cash-model","messages":[{"role":"user","content":"fixture"}],
        "max_tokens":64,"stream":stream})
}

pub async fn send(
    state: &Arc<AppState>,
    path: &str,
    token: &str,
    body: Option<Value>,
    group: Option<&str>,
) -> Response {
    let mut builder = if body.is_some() {
        Request::post(path)
    } else {
        Request::get(path)
    }
    .header("content-type", "application/json")
    .header("authorization", format!("Bearer {token}"));
    if token == support::MANAGEMENT_TOKEN {
        builder = builder.header("x-management-token", token);
    }
    if let Some(group) = group {
        builder = builder
            .header("x-neuro-account-group", group)
            .header("x-internal-api-key", support::MANAGEMENT_TOKEN);
    }
    tokio::time::timeout(
        Duration::from_secs(15),
        build_router(state.clone()).oneshot(
            builder
                .body(body.map_or_else(Body::empty, |body| Body::from(body.to_string())))
                .unwrap(),
        ),
    )
    .await
    .unwrap()
    .unwrap()
}

pub async fn api(
    state: &Arc<AppState>,
    path: &str,
    token: &str,
    body: Option<Value>,
    group: Option<&str>,
) -> (StatusCode, HeaderMap, Value) {
    let response = send(state, path, token, body, group).await;
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = tokio::time::timeout(Duration::from_secs(15), response.into_body().collect())
        .await
        .unwrap()
        .unwrap()
        .to_bytes();
    (
        status,
        headers,
        serde_json::from_slice(&bytes).unwrap_or_else(|_| json!(String::from_utf8_lossy(&bytes))),
    )
}

pub async fn relay(
    state: &Arc<AppState>,
    key: &Value,
    body: Value,
    group: Option<&str>,
) -> (StatusCode, Value) {
    let (status, _, body) = Box::pin(api(
        state,
        "/v1/chat/completions",
        key["token"].as_str().unwrap(),
        Some(body),
        group,
    ))
    .await;
    (status, body)
}
