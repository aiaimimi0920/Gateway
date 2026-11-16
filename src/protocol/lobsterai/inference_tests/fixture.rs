//! 实际 LocalRuntime、管理 API 创建的普通预付 Key；不读取真实 .ng。
use super::*;
pub(super) struct Fixture {
    pub state: Arc<AppState>,
    pub(super) id: String,
    token: String,
}
impl Fixture {
    pub async fn new(peer: &peer::Peer) -> Self {
        Self::with_http(peer.client(), peer.origin.clone()).await
    }
    pub async fn with_http(http: rquest::Client, origin: String) -> Self {
        Self::with_catalog(
            http,
            origin,
            UPSTREAM_MODEL,
            json!([{"modelId":UPSTREAM_MODEL,"apiFormat":"openai","supportsToolCalling":true}]),
        )
        .await
    }
    pub async fn with_catalog(
        http: rquest::Client,
        origin: String,
        upstream: &str,
        catalog: Value,
    ) -> Self {
        Self::with_quota(http, origin, upstream, catalog, false).await
    }
    pub async fn with_quota(
        http: rquest::Client,
        origin: String,
        upstream: &str,
        catalog: Value,
        cash: bool,
    ) -> Self {
        Self::with_probe_policy(http, origin, upstream, catalog, cash, None).await
    }
    pub async fn with_probe_policy(
        http: rquest::Client,
        origin: String,
        upstream: &str,
        catalog: Value,
        cash: bool,
        policy: Option<Value>,
    ) -> Self {
        let root = std::env::temp_dir().join(format!("lobsterai-public-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let mut config = support::test_config();
        config.storage_mode = crate::config::GatewayStorageMode::Local;
        config.gateway_api_key = None;
        config.gateway_api_key_secret = None;
        config.console.management_ui_enabled = false;
        config.console.state_dir = root.join("state");
        config.console.routes_file = root.join("routes.yaml");
        config.upstream_timeout_secs = 5;
        let material = super::super::ReceivedTokens::from_json(
            super::super::test_support::tokens().to_string().as_bytes(),
        )
        .unwrap()
        .admit_initial(uuid::Uuid::new_v4())
        .unwrap();
        let mut document = json!({"providers":[{"id":"lobsterai-public","adapter":"openai_compatible","auth_mode":super::super::AUTH_MODE,"base_url":super::super::management_boundary::BASE,
            "explicit_credentials_only":true,"request_wait":{"mode":"disabled"},"supported_models":[MODEL],"model_map":{MODEL:upstream},
            "credentials":[{"id":"exact","api_key":material.access_token,"refresh_token":material.refresh_token,"lobsterai":material.context,
                "lobsterai_catalog":{"models":catalog},"supported_models":[MODEL]}]}],"model_routes":[]});
        if let Some(policy) = policy {
            document["providers"][0]["credentials"][0]["test_policy"] = policy.clone();
            document["providers"][0]["test_plans"] = json!([{"id":"named","name":"Named",
                "scopes":[{"kind":"account","id":"exact"}],"policy":policy}]);
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
        Arc::get_mut(&mut state).unwrap().upstream_client.http = http.clone();
        Arc::get_mut(&mut state)
            .unwrap()
            .upstream_client
            .lobsterai_http = http;
        ready(&state, origin).await;
        let (status,text) = request(&state,"/v1/internal/gateway/access/keys",support::MANAGEMENT_TOKEN,
            json!({"ownerType":"user","ownerId":"fixture","resolvedProjectId":"local","resolvedTenantId":"local",
                "keyKind":"normal","publicKeyPrefix":"sk-gw","displayName":"LobsterAI prepaid contract","metadata":{"models":[MODEL]},"quota":if cash {json!({"mode":"cash_prepaid","limit":1_000_000,"currency":"USD"})} else {Value::Null}})).await;
        assert_eq!(status, StatusCode::OK, "{text}");
        let key: Value = serde_json::from_str(&text).unwrap();
        let id = key["id"].as_str().unwrap().to_owned();
        let token = key["token"].as_str().unwrap().to_owned();
        if !cash {
            let (status, text) = request(
                &state,
                &format!("/v1/internal/gateway/access/keys/{id}/balances/adjust"),
                support::MANAGEMENT_TOKEN,
                json!({"balanceMode":"message_prepaid","totalMessages":2,"remainingMessages":2}),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{text}");
        } else {
            sqlx::query("INSERT INTO provider_pricing(provider_id,payload) VALUES(?,?)")
                .bind("lobsterai-public").bind(json!({"staticInputMicrosPer1kTokens":1000,"staticOutputMicrosPer1kTokens":2000}).to_string())
                .execute(&state.local_runtime.as_ref().unwrap().pool).await.unwrap();
        }
        Self { state, id, token }
    }
    pub async fn restart(self, origin: String) -> Self {
        let config = self.state.config.clone();
        let http = self.state.upstream_client.lobsterai_http.clone();
        let id = self.id.clone();
        let token = self.token.clone();
        // 关闭旧 owner 后从原文件重建，不重写路由、Key、结果或到期槽。
        self.finish().await;
        let mut state = Box::pin(crate::runtime::build_app_state(config))
            .await
            .unwrap();
        assert!(state.pg_pool.is_none() && state.redis_pool.is_closed());
        Arc::get_mut(&mut state).unwrap().upstream_client.http = http.clone();
        Arc::get_mut(&mut state)
            .unwrap()
            .upstream_client
            .lobsterai_http = http;
        ready(&state, origin).await;
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
                    return rows;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap()
    }
    pub async fn finish(self) {
        crate::runtime::mark_runtime_draining(&self.state, "lobsterai_public_finish");
        self.state.shutdown.request("lobsterai_public_finish");
        crate::token_refresh::join(&self.state).await;
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
async fn ready(state: &Arc<AppState>, origin: String) {
    let worker = state.clone();
    *state.lifecycle.oauth_refresh_task.lock() = Some(tokio::spawn(
        super::super::transport::TEST_ORIGIN.scope(origin, async move {
            crate::token_refresh::start_token_refresh_task(worker, 3600, 300).await;
        }),
    ));
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if state
                .route_config
                .snapshot()
                .oauth_refresh_target("lobsterai-public", "exact")
                .unwrap()
                .1
                .refresh_config
                .as_ref()
                .unwrap()
                .dispatch_readiness()
                .is_ok()
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
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
