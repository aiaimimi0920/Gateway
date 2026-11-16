//! Production local startup and normal management Key creation; no private settlement calls.
use super::super::*;
use crate::{access_balance::AccessBalanceStore, state::AppState};

pub(super) struct Prepaid {
    pub state: Arc<AppState>,
    pub id: String,
    pub(super) token: String,
}
impl Prepaid {
    pub async fn new(peer: &wire::Server) -> Self {
        Self::with_client(peer.client()).await
    }
    // Configure transport before management routers can share AppState.
    pub async fn with_client(client: rquest::Client) -> Self {
        let root = std::env::temp_dir().join(format!("qwen-prepaid-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let mut config = support::test_config();
        config.storage_mode = crate::config::GatewayStorageMode::Local;
        config.gateway_api_key = None;
        config.gateway_api_key_secret = None;
        config.console.management_ui_enabled = false;
        config.console.state_dir = root.join("state");
        config.console.routes_file = root.join("routes.yaml");
        config.upstream_timeout_secs = 5;
        std::fs::write(
            &config.console.routes_file,
            serde_yaml::to_string(&document()).unwrap(),
        )
        .unwrap();
        let mut state = Box::pin(crate::runtime::build_app_state(config))
            .await
            .unwrap();
        assert!(state.pg_pool.is_none() && state.redis_pool.is_closed());
        Arc::get_mut(&mut state).unwrap().upstream_client.http = client;
        let key = manage(&state, "/v1/internal/gateway/access/keys",
            json!({"ownerType":"user","ownerId":"fixture","resolvedProjectId":"local","resolvedTenantId":"local","keyKind":"normal","publicKeyPrefix":"sk-gw","displayName":"Qwen prepaid contract","metadata":{"models":["public-alias"]}})).await;
        let fixture = Self {
            state,
            id: key["id"].as_str().unwrap().into(),
            token: key["token"].as_str().unwrap().into(),
        };
        manage(
            &fixture.state,
            &format!(
                "/v1/internal/gateway/access/keys/{}/balances/adjust",
                fixture.id
            ),
            json!({"balanceMode":"message_prepaid","totalMessages":2,"remainingMessages":2}),
        )
        .await;
        fixture
    }
    pub async fn request(&self, peer: &wire::Server, endpoint: &str, stream: bool) -> Response {
        let request = Request::post(format!("/v1/{endpoint}"))
            .header("authorization", format!("Bearer {}", self.token))
            .header("cookie", "foreign=caller-secret")
            .header("user-agent", "caller-UA")
            .header("content-type", "application/json")
            .body(Body::from(body(endpoint, stream).to_string()))
            .unwrap();
        wire::within(
            &peer.origin,
            Box::pin(build_router(self.state.clone()).oneshot(request)),
        )
        .await
        .unwrap()
    }
    pub async fn token_mode(&self) {
        self.token_remaining(10000).await;
    }
    pub async fn token_remaining(&self, remaining: i64) {
        manage(
            &self.state,
            &format!(
                "/v1/internal/gateway/access/keys/{}/balances/adjust",
                self.id
            ),
            json!({"balanceMode":"token_prepaid","totalTokens":10000,"remainingTokens":remaining}),
        )
        .await;
    }
    pub async fn tokens(&self) -> i64 {
        let balance = AccessBalanceStore::from_state(&self.state)
            .unwrap()
            .get(&self.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(balance.balance_mode, "token_prepaid");
        assert_eq!(balance.total_tokens, Some(10000));
        balance.remaining_tokens.unwrap()
    }
    pub async fn top_up_tokens(&self, amount: i64) {
        manage(
            &self.state,
            &format!(
                "/v1/internal/gateway/access/keys/{}/balances/adjust",
                self.id
            ),
            json!({"tokenDelta":amount}),
        )
        .await;
    }
    pub async fn balance(&self, remaining: i64) {
        let balance = AccessBalanceStore::from_state(&self.state)
            .unwrap()
            .get(&self.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(balance.balance_mode, "message_prepaid");
        assert_eq!(balance.total_messages, Some(2));
        assert_eq!(balance.remaining_messages, Some(remaining));
    }
    pub async fn audit(&self, status: &str, reported: bool) {
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
                    let row = &rows[0];
                    assert_eq!(row.status, status);
                    assert_eq!(row.access_key_id.as_deref(), Some(self.id.as_str()));
                    assert_eq!(row.resolved_model.as_deref(), Some("qwen3.7-plus"));
                    assert_eq!(
                        row.route_trace.as_ref().unwrap()["realCredentialRef"],
                        "exact"
                    );
                    assert_eq!(
                        (row.prompt_tokens, row.completion_tokens, row.total_tokens),
                        (
                            reported.then_some(2),
                            reported.then_some(3),
                            reported.then_some(5)
                        )
                    );
                    let serialized = serde_json::to_string(row).unwrap();
                    for secret in [
                        &self.token,
                        "synthetic-exact",
                        "synthetic-clearance",
                        "caller-secret",
                    ] {
                        assert!(
                            !serialized.contains(secret),
                            "audit exposed credential material"
                        );
                    }
                    while Arc::strong_count(&self.state) > 1 {
                        tokio::task::yield_now().await;
                    }
                    return;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
    pub async fn finish(self) {
        crate::runtime::mark_runtime_draining(&self.state, "qwen_prepaid_finish");
        self.state.shutdown.request("qwen_prepaid_finish");
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
async fn manage(state: &Arc<AppState>, path: &str, payload: Value) -> Value {
    let response = Box::pin(
        build_router(state.clone()).oneshot(
            Request::post(path)
                .header(
                    "authorization",
                    format!("Bearer {}", support::MANAGEMENT_TOKEN),
                )
                .header("x-management-token", support::MANAGEMENT_TOKEN)
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        ),
    )
    .await
    .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    serde_json::from_slice(&bytes).unwrap()
}
