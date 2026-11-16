//! Normal public router, normal management Key and real local SQLite settlement.
use super::peer::Peer;
use crate::{access_balance::AccessBalanceStore, db, http::router::build_router, state::AppState};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
use tower::ServiceExt;
#[path = "../../../../../tests/console_contract_support/mod.rs"]
mod support;

pub const MODEL: &str = "public-udio-music";
// Synthetic configured native identity, not a claimed current Udio model.
pub const NATIVE_MODEL: &str = "udio-fixture-native";
pub const SESSION: &str = "udio-synthetic-exact";
pub struct Fixture {
    pub state: Arc<AppState>,
    id: String,
    token: String,
}
impl Fixture {
    pub async fn new(peer: &Peer) -> Self {
        let root = std::env::temp_dir().join(format!("udio-pipeline-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let mut config = support::test_config();
        config.storage_mode = crate::config::GatewayStorageMode::Local;
        config.gateway_api_key = None;
        config.gateway_api_key_secret = None;
        config.console.management_ui_enabled = false;
        config.console.state_dir = root.join("state");
        config.console.routes_file = root.join("routes.yaml");
        config.upstream_timeout_secs = 5;
        let mut provider = json!({"id":"udio-pipeline","preset":"udio","base_url":"https://www.udio.com",
            "explicit_credentials_only":true,"request_wait":{"mode":"disabled"},
            "supported_models":[NATIVE_MODEL],"model_map":{MODEL:NATIVE_MODEL},
            "call_flow":{"call_adapter":"auto","fingerprint_adapter":"session","special_adapter":"udio_web"},
            "credentials":[{"id":"unrelated","api_key":"udio-unrelated","supported_models":["unrelated"]},
                {"id":"exact","api_key":SESSION,"supported_models":[NATIVE_MODEL]},
                {"id":"disabled","enabled":false,"api_key":"udio-disabled","supported_models":[NATIVE_MODEL]}]});
        let primary = provider.clone();
        provider["id"] = json!("udio-backup");
        provider["credentials"] =
            json!([{"id":"backup","api_key":"udio-backup","supported_models":[NATIVE_MODEL]}]);
        let document = json!({"providers":[primary,provider],"model_routes":[
            {"pattern":MODEL,"provider_ids":["udio-pipeline","udio-backup"]}]});
        std::fs::write(
            &config.console.routes_file,
            serde_yaml::to_string(&document).unwrap(),
        )
        .unwrap();
        let mut state = Box::pin(crate::runtime::build_app_state(config))
            .await
            .unwrap();
        assert!(state.pg_pool.is_none() && state.redis_pool.is_closed());
        let client = &mut Arc::get_mut(&mut state).unwrap().upstream_client;
        client.http = rquest::Client::builder().no_proxy().build().unwrap();
        client.browser_executor_base_url = Some(peer.origin.clone());
        client.browser_executor_bearer_token = Some("synthetic-executor-token".into());
        client.request_time_browser_policy =
            crate::upstream::request_time_browser_policy::RequestTimeBrowserPolicy::RemoteOnly;
        let (status,text) = request(&state,"/v1/internal/gateway/access/keys",support::MANAGEMENT_TOKEN,
            json!({"ownerType":"user","ownerId":"fixture","resolvedProjectId":"local","resolvedTenantId":"local",
                "keyKind":"normal","publicKeyPrefix":"sk-gw","displayName":"Udio prepaid contract","metadata":{"models":[MODEL]}})).await;
        assert_eq!(status, StatusCode::OK, "{text}");
        let key: Value = serde_json::from_str(&text).unwrap();
        let fixture = Self {
            state,
            id: key["id"].as_str().unwrap().into(),
            token: key["token"].as_str().unwrap().into(),
        };
        let (status, text) = request(
            &fixture.state,
            &format!(
                "/v1/internal/gateway/access/keys/{}/balances/adjust",
                fixture.id
            ),
            support::MANAGEMENT_TOKEN,
            json!({"balanceMode":"message_prepaid","totalMessages":2,"remainingMessages":2}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{text}");
        fixture
    }
    pub async fn open(&self, wait: bool) -> axum::response::Response {
        self.open_body(json!({"model":MODEL,"prompt":"合成音乐元数据测试","wait_audio":wait}))
            .await
    }
    pub async fn open_body(&self, body: Value) -> axum::response::Response {
        tokio::time::timeout(
            Duration::from_secs(10),
            Box::pin(
                build_router(self.state.clone()).oneshot(
                    Request::post("/v1/music/generations")
                        .header("authorization", format!("Bearer {}", self.token))
                        .header("cookie", "foreign=caller-secret")
                        .header("origin", "https://caller.invalid")
                        .header("content-type", "application/json")
                        .body(Body::from(body.to_string()))
                        .unwrap(),
                ),
            ),
        )
        .await
        .unwrap()
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
        crate::runtime::mark_runtime_draining(&self.state, "udio_pipeline_finish");
        self.state.shutdown.request("udio_pipeline_finish");
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
pub fn identity(row: &db::GatewayRequestAuditView) {
    assert_eq!(row.requested_model.as_deref(), Some(MODEL));
    assert_eq!(row.resolved_model.as_deref(), Some(NATIVE_MODEL));
    assert_eq!(row.provider_account_id.as_deref(), Some("udio-pipeline"));
    let trace = row.route_trace.as_ref().unwrap();
    assert_eq!(trace["realCredentialRef"], "exact");
    assert_eq!(trace["routeAttemptCount"], 1);
    assert_eq!(
        (row.prompt_tokens, row.completion_tokens, row.total_tokens),
        (None, None, None)
    );
}
pub fn wire(peer: &Peer) {
    let calls = peer.calls();
    assert_eq!(
        calls.len(),
        1,
        "no account switch or fallback may repeat generation"
    );
    assert_eq!(
        calls[0].0["authorization"],
        "Bearer synthetic-executor-token"
    );
    assert!(!calls[0].0.contains_key("cookie") && !calls[0].0.contains_key("origin"));
    let call = &calls[0].1;
    assert_eq!(call["provider"], "udio");
    assert_eq!(call["providerAccountId"], "udio-pipeline");
    assert_eq!(call["endpointKind"], "music_generations");
    assert_eq!(
        call["input"]["requestBody"]["gen_params"]["model_type"],
        NATIVE_MODEL
    );
    assert_eq!(
        call["input"]["cookieHeader"],
        format!("sb-ssr-production-auth-token={SESSION}")
    );
    assert!(!call.to_string().contains("caller-secret"));
}
