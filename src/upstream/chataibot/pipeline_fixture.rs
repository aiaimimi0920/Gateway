//! Normal public router, Local bootstrap, management-created Key and real SQLite settlement/audit.
use crate::{access_balance::AccessBalanceStore, db, http::router::build_router, state::AppState};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
use tower::ServiceExt;
#[path = "../../../tests/console_contract_support/mod.rs"]
pub(super) mod support;
pub const MODEL: &str = "public-chataibot-image";
pub const RESOLVED: &str = "google-nano-banana-2";
pub const TOKEN: &str = "synthetic-chataibot-exact-token";
pub struct Fixture {
    pub state: Arc<AppState>,
    pub id: String,
    pub token: String,
}
impl Fixture {
    pub async fn new(timeout: u64) -> Self {
        let root =
            std::env::temp_dir().join(format!("chataibot-pipeline-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let mut config = support::test_config();
        config.storage_mode = crate::config::GatewayStorageMode::Local;
        config.gateway_api_key = None;
        config.gateway_api_key_secret = None;
        config.console.management_ui_enabled = false;
        config.console.state_dir = root.join("state");
        config.console.routes_file = root.join("routes.yaml");
        config.upstream_timeout_secs = timeout;
        let mut document = json!({"providers":[{"id":"chataibot-pipeline","preset":"chataibot",
            "base_url":"https://chataibot.pro","explicit_credentials_only":true,
            "call_flow":{"special_adapter":"chataibot_web"},"request_wait":{"mode":"disabled"},
            "supported_models":[RESOLVED],"model_map":{MODEL:RESOLVED},"credentials":[
                {"id":"unrelated","api_key":"synthetic-unrelated","supported_models":["ideogram"]},
                {"id":"exact","api_key":TOKEN,"headers":{"User-Agent":"synthetic-account-ua"},"supported_models":[RESOLVED]},
                {"id":"disabled","enabled":false,"api_key":"synthetic-disabled","supported_models":[RESOLVED]}]}],
            "model_routes":[{"pattern":MODEL,"provider_ids":["chataibot-pipeline","chataibot-backup"]}]});
        let mut backup = document["providers"][0].clone();
        backup["id"] = json!("chataibot-backup");
        backup["credentials"] = json!([{"id":"backup","api_key":"synthetic-backup-token","supported_models":[RESOLVED]}]);
        document["providers"].as_array_mut().unwrap().push(backup);
        std::fs::write(
            &config.console.routes_file,
            serde_yaml::to_string(&document).unwrap(),
        )
        .unwrap();
        let mut state = Box::pin(crate::runtime::build_app_state(config))
            .await
            .unwrap();
        assert!(state.pg_pool.is_none() && state.redis_pool.is_closed());
        Arc::get_mut(&mut state).unwrap().upstream_client.http =
            rquest::Client::builder().no_proxy().build().unwrap();
        let (status, text) = management(&state, "/v1/internal/gateway/access/keys", json!({
            "ownerType":"user","ownerId":"fixture","resolvedProjectId":"local","resolvedTenantId":"local",
            "keyKind":"normal","publicKeyPrefix":"sk-gw","displayName":"ChatAIBot contract","metadata":{"models":[MODEL]}})).await;
        assert_eq!(status, StatusCode::OK, "{text}");
        let key: Value = serde_json::from_str(&text).unwrap();
        let fixture = Self {
            state,
            id: key["id"].as_str().unwrap().into(),
            token: key["token"].as_str().unwrap().into(),
        };
        fixture
            .adjust(
                json!({"balanceMode":"message_prepaid","totalMessages":2,"remainingMessages":2}),
            )
            .await;
        fixture
    }
    pub async fn adjust(&self, value: Value) {
        let (status, text) = management(
            &self.state,
            &format!(
                "/v1/internal/gateway/access/keys/{}/balances/adjust",
                self.id
            ),
            value,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{text}");
    }
    pub async fn open(&self, endpoint: &str, body: Value) -> axum::response::Response {
        self.open_raw(endpoint, "application/json", body.to_string().into_bytes())
            .await
    }
    pub async fn open_raw(
        &self,
        endpoint: &str,
        mime: &str,
        body: Vec<u8>,
    ) -> axum::response::Response {
        Box::pin(
            build_router(self.state.clone()).oneshot(
                Request::post(format!("/v1/{endpoint}"))
                    .header("authorization", format!("Bearer {}", self.token))
                    .header("content-type", mime)
                    .header("cookie", "token=foreign-caller-secret")
                    .header("origin", "https://caller.example.test")
                    .body(Body::from(body))
                    .unwrap(),
            ),
        )
        .await
        .unwrap()
    }
    pub async fn balance(&self) -> db::GatewayAccessKeyBalanceView {
        AccessBalanceStore::from_state(&self.state)
            .unwrap()
            .get(&self.id)
            .await
            .unwrap()
            .unwrap()
    }
    pub async fn audit(&self) -> db::GatewayRequestAuditView {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let rows = self
                    .state
                    .local_runtime
                    .as_ref()
                    .unwrap()
                    .list_audits(&db::RequestAuditFilters::default())
                    .await
                    .unwrap();
                if rows.len() == 1 && !matches!(rows[0].status.as_str(), "started" | "running") {
                    while Arc::strong_count(&self.state) > 1 {
                        tokio::task::yield_now().await;
                    }
                    return rows.into_iter().next().unwrap();
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap()
    }
    pub async fn finish(self) {
        crate::runtime::mark_runtime_draining(&self.state, "chataibot_contract_finish");
        self.state.shutdown.request("chataibot_contract_finish");
        tokio::time::timeout(Duration::from_secs(3), async {
            while Arc::strong_count(&self.state) > 1 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        self.state.local_runtime.as_ref().unwrap().close().await;
        self.state
            .upstream_client
            .freebuff
            .shutdown(Duration::from_secs(1))
            .await
            .unwrap();
    }
}
async fn management(state: &Arc<AppState>, path: &str, body: Value) -> (StatusCode, String) {
    let response = Box::pin(
        build_router(state.clone()).oneshot(
            Request::post(path)
                .header(
                    "authorization",
                    format!("Bearer {}", support::MANAGEMENT_TOKEN),
                )
                .header("x-management-token", support::MANAGEMENT_TOKEN)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        ),
    )
    .await
    .unwrap();
    (response.status(), drain(response.into_body()).await)
}
pub async fn drain(body: Body) -> String {
    String::from_utf8(body.collect().await.unwrap().to_bytes().to_vec()).unwrap()
}
pub fn body() -> Value {
    json!({"model":MODEL,"prompt":"合成图片测试","response_format":"b64_json"})
}
pub fn identity(row: &db::GatewayRequestAuditView) {
    assert_eq!(row.requested_model.as_deref(), Some(MODEL));
    assert_eq!(row.resolved_model.as_deref(), Some(RESOLVED));
    assert_eq!(
        row.provider_account_id.as_deref(),
        Some("chataibot-pipeline")
    );
    let trace = row.route_trace.as_ref().unwrap();
    assert_eq!(trace["realCredentialRef"], "exact");
    assert_eq!(trace["routeAttemptCount"], 1);
    let audit = serde_json::to_string(row).unwrap();
    for secret in [TOKEN, "synthetic-private", "foreign-caller-secret"] {
        assert!(!audit.contains(secret), "{audit}");
    }
}
