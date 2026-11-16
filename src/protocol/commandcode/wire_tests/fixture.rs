//! 实际 LocalRuntime、管理 API 创建的普通预付 Key；不读取真实 .ng。
use super::*;
pub(super) struct Fixture {
    pub state: Arc<AppState>,
    id: String,
    token: String,
}
impl Fixture {
    pub async fn new(peer: &peer::Peer) -> Self {
        Self::with_fallback(peer, false).await
    }
    pub async fn with_fallback(peer: &peer::Peer, fallback: bool) -> Self {
        Self::configured(peer, fallback, None).await
    }
    pub async fn native_responses(peer: &peer::Peer) -> Self {
        Self::configured(peer, false, Some("/responses")).await
    }
    pub async fn native_messages(peer: &peer::Peer) -> Self {
        Self::configured(peer, false, Some("/messages")).await
    }
    pub async fn systemone(peer: &peer::Peer) -> Self {
        Self::configured(peer, false, Some("/systemone")).await
    }
    pub async fn systemone_policy(peer: &peer::Peer, filter: bool, limit: Option<usize>) -> Self {
        Self::configured_policy(peer, false, Some("/systemone"), filter, limit).await
    }
    async fn configured(peer: &peer::Peer, fallback: bool, native: Option<&str>) -> Self {
        Self::configured_policy(peer, fallback, native, false, None).await
    }
    async fn configured_policy(
        peer: &peer::Peer,
        fallback: bool,
        native: Option<&str>,
        filter: bool,
        limit: Option<usize>,
    ) -> Self {
        let root =
            std::env::temp_dir().join(format!("commandcode-public-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let mut config = support::test_config();
        config.storage_mode = crate::config::GatewayStorageMode::Local;
        config.gateway_api_key = None;
        config.gateway_api_key_secret = None;
        config.console.management_ui_enabled = false;
        config.console.state_dir = root.join("state");
        config.console.routes_file = root.join("routes.yaml");
        config.upstream_timeout_secs = 5;
        if let Some(limit) = limit {
            config.max_body_chat_completions_bytes = limit;
        }
        let mut document = json!({"providers":[{"id":"commandcode-public","preset":"commandcode-api","base_url":super::super::BASE,
            "explicit_credentials_only":true,"request_wait":{"mode":"disabled"},
            "supported_models":[MODEL],"model_map":{MODEL:UPSTREAM_MODEL},
            "credentials":[{"id":"unrelated","api_key":if fallback { KEY } else { "other-key" },"supported_models":[if fallback { MODEL } else { "other-model" }]},
                {"id":"exact","api_key":KEY,"supported_models":[MODEL]}]}],"model_routes":[]});
        if let Some(endpoint) = native {
            let routes = vec![endpoint];
            document["providers"][0]["credentials"][1]["model_catalog"] = json!({
                "sourceUrl":super::super::SOURCE,"checkedAt":"2026-11-13T00:00:00Z",
                "models":[UPSTREAM_MODEL],"supportedEndpoints":{UPSTREAM_MODEL:routes}
            });
        }
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
        // 管理请求会发布状态的弱引用；测试策略必须在首次请求前完成初始化。
        if filter {
            Arc::get_mut(&mut state).unwrap().filter_config =
                Some(crate::filter::chain::build_default_filter_chain());
        }
        let (status,text) = request(&state,"/v1/internal/gateway/access/keys",support::MANAGEMENT_TOKEN,
            json!({"ownerType":"user","ownerId":"fixture","resolvedProjectId":"local","resolvedTenantId":"local",
                "keyKind":"normal","publicKeyPrefix":"sk-gw","displayName":"CommandCode prepaid contract","metadata":{"models":[MODEL]}})).await;
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
        self.open_headers(endpoint, body, &[]).await
    }
    pub async fn open_headers(
        &self,
        endpoint: &str,
        body: Value,
        zdr: &[&str],
    ) -> axum::response::Response {
        let mut request = Request::post(format!("/v1/{endpoint}"));
        for value in zdr {
            request = request.header("x-cmd-zdr", *value);
        }
        tokio::time::timeout(
            Duration::from_secs(10),
            Box::pin(
                build_router(self.state.clone()).oneshot(
                    request
                        .header("authorization", format!("Bearer {}", self.token))
                        .header("cookie", "foreign=caller-secret")
                        .header("originator", "codex_cli_rs")
                        .header("chatgpt-account-id", "foreign")
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
    pub async fn restart(self, peer: &peer::Peer) -> Self {
        let config = self.state.config.clone();
        let id = self.id.clone();
        let token = self.token.clone();
        self.finish().await;
        let mut state = Box::pin(crate::runtime::build_app_state(config))
            .await
            .unwrap();
        Arc::get_mut(&mut state).unwrap().upstream_client.http = peer.client();
        Self { state, id, token }
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
        crate::runtime::mark_runtime_draining(&self.state, "commandcode_public_finish");
        self.state.shutdown.request("commandcode_public_finish");
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
