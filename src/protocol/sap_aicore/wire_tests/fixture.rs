//! 实际 LocalRuntime、管理 API 创建的普通预付 Key；不读取真实 .ng。
use super::*;
pub(in crate::protocol::sap_aicore) struct Fixture {
    pub state: Arc<AppState>,
    id: String,
    pub(in crate::protocol::sap_aicore) token: String,
}
impl Fixture {
    pub async fn new(peer: &peer::Peer) -> Self {
        Self::with_fallback(peer, false).await
    }
    pub async fn with_fallback(peer: &peer::Peer, fallback: bool) -> Self {
        Self::configured(
            peer,
            fallback,
            super::super::test_support::BASE,
            KEY,
            json!({"sap_api_version":"2024-10-21"}),
        )
        .await
    }
    pub async fn with_images(peer: &peer::Peer) -> Self {
        Self::configured(
            peer,
            false,
            super::super::test_support::BASE,
            KEY,
            json!({"sap_api_version":"2024-10-21","sap_image_input":true}),
        )
        .await
    }
    pub async fn with_reasoning(peer: &peer::Peer) -> Self {
        Self::configured(
            peer,
            false,
            super::super::test_support::BASE,
            KEY,
            json!({"sap_api_version":"2024-12-01-preview","sap_o3_mini":true}),
        )
        .await
    }
    pub async fn with_key(peer: &peer::Peer, service_key: &str) -> Self {
        Self::with_target(peer, super::super::test_support::BASE, service_key).await
    }
    pub async fn with_target(peer: &peer::Peer, base: &str, service_key: &str) -> Self {
        Self::configured(
            peer,
            false,
            base,
            service_key,
            json!({"sap_api_version":"2024-10-21"}),
        )
        .await
    }
    pub(in crate::protocol::sap_aicore) async fn configured(
        peer: &peer::Peer,
        fallback: bool,
        base: &str,
        service_key: &str,
        extra_body: Value,
    ) -> Self {
        Box::pin(Self::configured_with_http(
            peer.client(),
            fallback,
            base,
            service_key,
            extra_body,
        ))
        .await
    }

    pub(in crate::protocol::sap_aicore) async fn configured_with_http(
        http: rquest::Client,
        fallback: bool,
        base: &str,
        service_key: &str,
        extra_body: Value,
    ) -> Self {
        let root = std::env::temp_dir().join(format!("sap_aicore-public-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let mut config = support::test_config();
        config.storage_mode = crate::config::GatewayStorageMode::Local;
        config.gateway_api_key = None;
        config.gateway_api_key_secret = None;
        config.console.management_ui_enabled = false;
        config.console.state_dir = root.join("state");
        config.console.routes_file = root.join("routes.yaml");
        config.upstream_timeout_secs = 5;
        let document = json!({"providers":[{"id":"sap_aicore-public","preset":"sap-aicore-chat","base_url":base,
            "headers":{"AI-Resource-Group":"group-a"},"extra_body":extra_body,
            "explicit_credentials_only":true,"request_wait":{"mode":"disabled"},
            "supported_models":[MODEL],"model_map":{MODEL:UPSTREAM_MODEL},
            "credentials":[{"id":"unrelated","api_key":service_key,"supported_models":[if fallback { MODEL } else { "other-model" }]},
                {"id":"exact","api_key":service_key,"supported_models":[MODEL]}]}],"model_routes":[]});
        std::fs::write(
            &config.console.routes_file,
            serde_yaml::to_string(&document).unwrap(),
        )
        .unwrap();
        let mut state = Box::pin(crate::runtime::build_app_state(config))
            .await
            .unwrap();
        assert!(state.pg_pool.is_none() && state.redis_pool.is_closed());
        Arc::get_mut(&mut state).unwrap().upstream_client.http = http;
        let (status,text) = request(&state,"/v1/internal/gateway/access/keys",support::MANAGEMENT_TOKEN,
            json!({"ownerType":"user","ownerId":"fixture","resolvedProjectId":"local","resolvedTenantId":"local",
                "keyKind":"normal","publicKeyPrefix":"sk-gw","displayName":"SAP AI Core prepaid contract","metadata":{"models":[MODEL]}})).await;
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
        let result = tokio::time::timeout(Duration::from_secs(3), async {
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
        .await;
        match result {
            Ok(rows) => rows,
            Err(_) => {
                let rows = self
                    .state
                    .local_runtime
                    .as_ref()
                    .unwrap()
                    .list_audits(&db::RequestAuditFilters::default())
                    .await
                    .unwrap();
                let statuses: Vec<_> = rows.iter().map(|row| row.status.as_str()).collect();
                panic!(
                    "expected {count} settled audits; found {statuses:?} with {} AppState owners",
                    Arc::strong_count(&self.state)
                );
            }
        }
    }
    pub async fn finish(self) {
        crate::runtime::mark_runtime_draining(&self.state, "sap_aicore_public_finish");
        self.state.shutdown.request("sap_aicore_public_finish");
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
