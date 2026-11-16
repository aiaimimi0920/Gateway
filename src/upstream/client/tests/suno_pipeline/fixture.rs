//! Normal music route and Key management against isolated SQLite, not a provider-private settle.
use super::peer::Server;
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

pub const MODEL: &str = "public-suno-music";
pub const NATIVE: &str = "chirp-v3-5"; // Existing wire fixture identity, not a current model claim.
pub const SESSION: &str = "suno-synthetic-exact";
pub struct Fixture {
    pub state: Arc<AppState>,
    id: String,
    token: String,
}
impl Fixture {
    pub async fn new(peer: &Server) -> Self {
        Self::with_client(peer, |_| {}).await
    }
    // Customize before router-owned audits can borrow AppState; never mutate a live shared Arc.
    pub async fn with_client(
        peer: &Server,
        configure: impl FnOnce(&mut crate::upstream::client::UpstreamClient),
    ) -> Self {
        let root = std::env::temp_dir().join(format!("suno-pipeline-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let mut config = support::test_config();
        config.storage_mode = crate::config::GatewayStorageMode::Local;
        config.gateway_api_key = None;
        config.gateway_api_key_secret = None;
        config.console.management_ui_enabled = false;
        config.console.state_dir = root.join("state");
        config.console.routes_file = root.join("routes.yaml");
        config.upstream_timeout_secs = 5;
        let mut provider = json!({"id":"suno-pipeline","preset":"suno","base_url":peer.origin,
            "explicit_credentials_only":true,"request_wait":{"mode":"disabled"},"priority":10,
            "execution_mode":"direct_http","supported_models":[NATIVE],"model_map":{MODEL:NATIVE},
            "call_flow":{"call_adapter":"auto","fingerprint_adapter":"session","special_adapter":"suno_web"},
            "credentials":[{"id":"unrelated","api_key":"suno-unrelated","supported_models":["unrelated"]},
                {"id":"exact","api_key":SESSION,"headers":{"Cookie":format!("__session={SESSION}"),"device-id":"fixture-device"},"supported_models":[NATIVE]},
                {"id":"disabled","enabled":false,"api_key":"suno-disabled","supported_models":[NATIVE]}]});
        let primary = provider.clone();
        provider["id"] = json!("suno-backup");
        provider["priority"] = json!(0);
        provider["credentials"] = json!([{"id":"backup","api_key":"suno-backup","headers":{"Cookie":"__session=backup"},"supported_models":[NATIVE]}]);
        let document = json!({"providers":[primary,provider],"model_routes":[{"pattern":MODEL,"provider_ids":["suno-pipeline","suno-backup"]}]});
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
        configure(&mut Arc::get_mut(&mut state).unwrap().upstream_client);
        let (status,text) = request(&state,"/v1/internal/gateway/access/keys",support::MANAGEMENT_TOKEN,
            json!({"ownerType":"user","ownerId":"fixture","resolvedProjectId":"local","resolvedTenantId":"local","keyKind":"normal","publicKeyPrefix":"sk-gw","displayName":"Suno prepaid contract","metadata":{"models":[MODEL]}})).await;
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
        let body = json!({"model":MODEL,"prompt":"合成音频公共合同测试","waitCompletion":wait,"pollIntervalMs":1000});
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
        crate::runtime::mark_runtime_draining(&self.state, "suno_pipeline_finish");
        self.state.shutdown.request("suno_pipeline_finish");
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
    assert_eq!(row.resolved_model.as_deref(), Some(NATIVE));
    assert_eq!(row.provider_account_id.as_deref(), Some("suno-pipeline"));
    let trace = row.route_trace.as_ref().unwrap();
    assert_eq!(trace["realCredentialRef"], "exact");
    assert_eq!(trace["routeAttemptCount"], 1);
    assert_eq!(
        (row.prompt_tokens, row.completion_tokens, row.total_tokens),
        (None, None, None)
    );
}
pub fn wire(peer: &Server, assets: usize) {
    let calls = peer.calls();
    assert_eq!(
        calls.len(),
        2 + assets,
        "no account switch/fallback or extra billed operation"
    );
    assert_eq!(calls[0].path, "/api/c/check");
    assert_eq!(calls[1].path, "/api/generate/v2-web/");
    assert_eq!(
        calls[1].body["mv"],
        crate::protocol::suno::SUNO_DEFAULT_UPSTREAM_MODEL
    );
    for call in &calls[..2] {
        assert_eq!(call.headers["authorization"], format!("Bearer {SESSION}"));
        assert_eq!(call.headers["cookie"], format!("__session={SESSION}"));
        assert!(!call.body.to_string().contains("caller-secret"));
    }
    for call in &calls[2..] {
        assert_eq!(call.method, axum::http::Method::GET);
        for name in [
            "cookie",
            "authorization",
            "origin",
            "referer",
            "device-id",
            "browser-token",
        ] {
            assert!(!call.headers.contains_key(name), "asset received {name}");
        }
    }
}
