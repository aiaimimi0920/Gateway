//! Production local bootstrap, normal access Key and SQLite audit/balance owners.
use super::test_support::Server;
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
pub(crate) mod support;

pub const IMAGE: &str = "public-luma-image";
pub const VIDEO: &str = "public-luma-video";
pub const KEY: &str = "luma-api-synthetic-exact";
pub struct Fixture {
    pub state: Arc<AppState>,
    pub id: String,
    pub token: String,
}
impl Fixture {
    pub async fn new(server: &Server, timeout_secs: u64) -> Self {
        let root = std::env::temp_dir().join(format!("luma-pipeline-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let mut config = support::test_config();
        config.storage_mode = crate::config::GatewayStorageMode::Local;
        config.gateway_api_key = None;
        config.gateway_api_key_secret = None;
        config.console.management_ui_enabled = false;
        config.console.state_dir = root.join("state");
        config.console.routes_file = root.join("routes.yaml");
        config.upstream_timeout_secs = timeout_secs;
        let mut document = json!({"providers":[{"id":"luma-pipeline","preset":"lumalabs-api",
            "base_url":crate::protocol::luma_agents::BASE,"explicit_credentials_only":true,
            "request_wait":{"mode":"disabled"},"supported_models":["uni-1","ray-3.2"],
            "model_map":{IMAGE:"uni-1",VIDEO:"ray-3.2"},
            "credentials":[{"id":"unrelated","api_key":"luma-api-unrelated","supported_models":["uni-1-max"]},
                {"id":"exact","api_key":KEY,"supported_models":["uni-1","ray-3.2"]},
                {"id":"disabled","enabled":false,"api_key":"luma-api-disabled","supported_models":["uni-1","ray-3.2"]}]}],
            "model_routes":[]});
        let mut backup = document["providers"][0].clone();
        backup["id"] = json!("luma-backup");
        backup["credentials"] = json!([{"id":"backup-exact","api_key":"luma-api-backup","supported_models":["uni-1","ray-3.2"]}]);
        document["providers"].as_array_mut().unwrap().push(backup);
        document["model_routes"] = json!([
            {"pattern":IMAGE,"provider_ids":["luma-pipeline","luma-backup"]},
            {"pattern":VIDEO,"provider_ids":["luma-pipeline","luma-backup"]}]);
        std::fs::write(
            &config.console.routes_file,
            serde_yaml::to_string(&document).unwrap(),
        )
        .unwrap();
        let mut state = Box::pin(crate::runtime::build_app_state(config))
            .await
            .unwrap();
        assert!(state.pg_pool.is_none() && state.redis_pool.is_closed());
        Arc::get_mut(&mut state).unwrap().upstream_client.http = server.client();
        let (status, text) = request(&state, "/v1/internal/gateway/access/keys", support::MANAGEMENT_TOKEN,
            json!({"ownerType":"user","ownerId":"fixture","resolvedProjectId":"local","resolvedTenantId":"local",
            "keyKind":"normal","publicKeyPrefix":"sk-gw","displayName":"Luma prepaid contract",
            "metadata":{"models":[IMAGE,VIDEO]}})).await;
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
    pub async fn adjust(&self, body: Value) {
        let (status, text) = request(
            &self.state,
            &format!(
                "/v1/internal/gateway/access/keys/{}/balances/adjust",
                self.id
            ),
            support::MANAGEMENT_TOKEN,
            body,
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
        content_type: &str,
        body: Vec<u8>,
    ) -> axum::response::Response {
        tokio::time::timeout(
            Duration::from_secs(10),
            Box::pin(
                build_router(self.state.clone()).oneshot(
                    Request::post(format!("/v1/{endpoint}"))
                        .header("authorization", format!("Bearer {}", self.token))
                        .header("cookie", "foreign=caller-secret")
                        .header("origin", "https://caller.example.test")
                        .header("content-type", content_type)
                        .body(Body::from(body))
                        .unwrap(),
                ),
            ),
        )
        .await
        .expect("bounded local public request")
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
                    // Terminal audit precedes settlement; wait for its owner, not an expected balance.
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
        crate::runtime::mark_runtime_draining(&self.state, "luma_pipeline_finish");
        self.state.shutdown.request("luma_pipeline_finish");
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
async fn request(
    state: &Arc<AppState>,
    path: &str,
    token: &str,
    body: Value,
) -> (StatusCode, String) {
    let response = Box::pin(
        build_router(state.clone()).oneshot(
            Request::post(path)
                .header("authorization", format!("Bearer {token}"))
                .header("x-management-token", token)
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
pub fn body(model: &str) -> Value {
    json!({"model":model,"prompt":"合成媒体测试","response_format":"b64_json"})
}
pub fn identity(row: &db::GatewayRequestAuditView, model: &str, resolved: &str) {
    assert_eq!(row.requested_model.as_deref(), Some(model));
    assert_eq!(row.resolved_model.as_deref(), Some(resolved));
    assert_eq!(row.provider_account_id.as_deref(), Some("luma-pipeline"));
    let trace = row.route_trace.as_ref().unwrap();
    assert_eq!(trace["realCredentialRef"], "exact");
    if row.status == "completed" {
        assert_eq!(
            trace["selectedProtocolProfile"],
            crate::protocol::luma_agents::PROFILE
        );
    } else {
        // Failed/cancelled attempts retain audited identity but have no winning profile.
        assert!(trace["selectedProtocolProfile"].is_null());
    }
    assert_eq!(trace["routeAttemptCount"], 1);
}
