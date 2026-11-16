//! Real local bootstrap and ordinary management Key creation; no injected auth or balance bypass.
use super::*;
pub(super) struct Fixture {
    pub state: Arc<AppState>,
    pub id: String,
    token: String,
}
impl Fixture {
    pub async fn new(server: &wire::Server) -> Self {
        let root =
            std::env::temp_dir().join(format!("vertex-oauth-pipeline-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let mut config = support::test_config();
        config.storage_mode = crate::config::GatewayStorageMode::Local;
        config.gateway_api_key = None;
        config.gateway_api_key_secret = None;
        config.console.management_ui_enabled = false;
        config.console.state_dir = root.join("state");
        config.console.routes_file = root.join("routes.yaml");
        config.upstream_timeout_secs = 5;
        let context = json!({"grant_type":"desktop_authorization_code","client_id":"synthetic-client",
            "requested_scope":"https://www.googleapis.com/auth/cloud-platform","scope_reported":false,
            "target":{"project":"project-a","location":"global","quota_project":"quota-a"}});
        let document = json!({"providers":[{"id":"vertex-pipeline","preset":"google-agent-platform-official-api",
            "auth_mode":"vertex_user_oauth", "base_url":"https://aiplatform.googleapis.com/v1/projects/project-a/locations/global/publishers/google",
            "explicit_credentials_only":true,
            "request_wait":{"mode":"disabled"},"supported_models":[MODEL],"model_map":{MODEL:"gemini-contract"},
            "credentials":[{"id":"unrelated","api_key":"synthetic-unrelated","supported_models":["other-model"],
                "refresh_token":"synthetic-unrelated-refresh","vertex_oauth":context,"expires_at":"2099-01-01T00:00:00Z"},
                {"id":"exact","api_key":ACCESS,"supported_models":[MODEL],"refresh_token":"synthetic-refresh",
                "vertex_oauth":context,"expires_at":"2099-01-01T00:00:00Z"}]}],"model_routes":[]});
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
                "keyKind":"normal","publicKeyPrefix":"sk-gw","displayName":"Vertex prepaid contract","metadata":{"models":[MODEL]}})).await;
        assert_eq!(status, StatusCode::OK, "{text}");
        let key: Value = serde_json::from_str(&text).unwrap();
        let id = key["id"].as_str().unwrap().to_owned();
        let token = key["token"].as_str().unwrap().to_owned();
        let (status, text) = request(
            &state,
            &format!("/v1/internal/gateway/access/keys/{id}/balances/adjust"),
            support::MANAGEMENT_TOKEN,
            json!({"balanceMode":"message_prepaid","totalMessages":2,"remainingMessages":2}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{text}");
        Self { state, id, token }
    }
    pub async fn open(&self, endpoint: &str, body: Value) -> axum::response::Response {
        tokio::time::timeout(
            Duration::from_secs(10),
            Box::pin(
                build_router(self.state.clone()).oneshot(
                    Request::post(format!("/v1/{endpoint}"))
                        .header("authorization", format!("Bearer {}", self.token))
                        .header("cookie", "foreign=caller-secret")
                        .header("content-type", "application/json")
                        .body(Body::from(body.to_string()))
                        .unwrap(),
                ),
            ),
        )
        .await
        .expect("Full Vertex public fixture exceeded its local deadline")
        .unwrap()
    }
    pub async fn remaining(&self) -> i64 {
        AccessBalanceStore::from_state(&self.state)
            .unwrap()
            .get(&self.id)
            .await
            .unwrap()
            .unwrap()
            .remaining_messages
            .unwrap()
    }
    pub async fn token_balance(&self) -> (StatusCode, String) {
        request(
            &self.state,
            &format!(
                "/v1/internal/gateway/access/keys/{}/balances/adjust",
                self.id
            ),
            support::MANAGEMENT_TOKEN,
            json!({"balanceMode":"token_prepaid","totalTokens":100,"remainingTokens":100}),
        )
        .await
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
                    // The audit write precedes quota settlement. Wait for its real owner to release state.
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
        crate::runtime::mark_runtime_draining(&self.state, "vertex_pipeline_finish");
        self.state.shutdown.request("vertex_pipeline_finish");
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
pub(super) fn request<'a>(
    state: &'a Arc<AppState>,
    path: &'a str,
    token: &'a str,
    body: Value,
) -> std::pin::Pin<Box<impl std::future::Future<Output = (StatusCode, String)> + 'a>> {
    Box::pin(async move {
        let response = build_router(state.clone())
            .oneshot(
                Request::post(path)
                    .header("authorization", format!("Bearer {token}"))
                    .header("x-management-token", token)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        (status, drain(response.into_body()).await)
    })
}
