//! Build real local storage and create the ordinary prepaid Key through authenticated management.
use super::*;
pub(super) struct Fixture {
    pub state: Arc<AppState>,
    id: String,
    token: String,
}
impl Fixture {
    pub async fn new(peer: &peer::Peer, family: &str) -> Self {
        let root = std::env::temp_dir().join(format!("dim-public-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let mut config = support::test_config();
        config.storage_mode = crate::config::GatewayStorageMode::Local;
        config.gateway_api_key = None;
        config.gateway_api_key_secret = None;
        config.console.management_ui_enabled = false;
        config.console.state_dir = root.join("state");
        config.console.routes_file = root.join("routes.yaml");
        config.upstream_timeout_secs = 5;
        let context: super::super::AuthorizationContext =
            serde_json::from_value(super::super::test_support::context()).unwrap();
        let declaration = match family {
            "chat" => "openai",
            "responses" => "openai-response",
            "messages" => "anthropic",
            _ => "gemini",
        };
        let models = super::super::parse_catalog(&serde_json::to_vec(&json!({"data":[{"id":"opaque-model","dim":{"endpoint_type":declaration,"tool_call":true}}]})).unwrap()).unwrap();
        let catalog = super::super::ModelCatalog::new(&context, models).unwrap();
        let document = json!({"providers":[{"id":"dim-public","preset":"dim-cn-oauth","auth_mode":"dim_user_oauth",
            "base_url":"https://dimagent.cn/v1","explicit_credentials_only":true,"request_wait":{"mode":"disabled"},
            "supported_models":[MODEL],"model_map":{MODEL:"opaque-model"},
            "credentials":[{"id":"unrelated","api_key":"synthetic-unrelated","refresh_token":"synthetic-unrelated-refresh","supported_models":["other-model"],"dim_oauth":context,"dim_catalog":catalog,"expires_at":"2099-01-01T00:00:00Z"},
            {"id":"exact","api_key":ACCESS,"refresh_token":"synthetic-dim-refresh","supported_models":[MODEL],"dim_oauth":context,"dim_catalog":catalog,"expires_at":"2099-01-01T00:00:00Z"}]}],"model_routes":[]});
        std::fs::write(
            &config.console.routes_file,
            serde_yaml::to_string(&document).unwrap(),
        )
        .unwrap();
        let mut state = Box::pin(crate::runtime::build_app_state(config))
            .await
            .unwrap();
        assert!(state.pg_pool.is_none() && state.redis_pool.is_closed());
        Arc::get_mut(&mut state).unwrap().upstream_client.http = peer.client();
        let (status,text) = request(&state,"/v1/internal/gateway/access/keys",support::MANAGEMENT_TOKEN,
            json!({"ownerType":"user","ownerId":"fixture","resolvedProjectId":"local","resolvedTenantId":"local","keyKind":"normal","publicKeyPrefix":"sk-gw","displayName":"DIM prepaid contract","metadata":{"models":[MODEL]}})).await;
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
        let req = match endpoint {
            "responses" => crate::protocol::responses::normalize_responses(body.clone()),
            "messages" => crate::protocol::anthropic::normalize_messages(body.clone()),
            _ => crate::protocol::openai::normalize_chat_completions(body.clone()),
        }
        .unwrap();
        let candidate = self
            .state
            .route_config
            .snapshot()
            .resolve_candidates(Some(MODEL))
            .into_iter()
            .find(|c| c.provider_account_id == "dim-public")
            .unwrap();
        // Fail before dispatch if a regression would bypass the only loopback-scoped sender.
        assert!(!crate::upstream::client::UpstreamClient::uses_native_codex(
            &candidate.payload,
            &req,
            "opaque-model"
        ));
        assert!(
            !crate::upstream::client::UpstreamClient::uses_native_claude(
                &candidate.payload,
                &req,
                "opaque-model"
            )
        );
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
        .expect("DIM public request deadline")
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
    pub async fn audits(&self, count: usize) -> Vec<db::GatewayRequestAuditView> {
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
                if rows.len() == count
                    && rows
                        .iter()
                        .all(|r| !matches!(r.status.as_str(), "started" | "running"))
                {
                    while Arc::strong_count(&self.state) > 1 {
                        tokio::task::yield_now().await;
                    }
                    return rows;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap()
    }
    pub async fn finish(self) {
        crate::runtime::mark_runtime_draining(&self.state, "dim_public_finish");
        self.state.shutdown.request("dim_public_finish");
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
    (response.status(), drain(response.into_body()).await)
}
