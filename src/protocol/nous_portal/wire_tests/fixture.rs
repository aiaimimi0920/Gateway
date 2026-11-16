//! Product-specific typed credentials use the real LocalRuntime and ordinary management-created Key.
use super::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
pub(super) fn access(subject: &str) -> String {
    format!(
        "{}.{}.{}",
        URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256"}"#),
        URL_SAFE_NO_PAD.encode(
            json!({"sub":subject,"exp":4070908800_u64,"scope":"inference:invoke"}).to_string()
        ),
        URL_SAFE_NO_PAD.encode(b"synthetic-signature")
    )
}
pub(super) fn credential(id: &str, model: &str, target: InferenceTarget) -> Value {
    let access = access(id);
    let receipt = super::super::ReceivedTokens::from_json(&serde_json::to_vec(&json!({
        "access_token":access,"refresh_token":format!("nous-refresh-{id}"),"scope":"inference:invoke",
        "inference_base_url":target.base_url()
    })).unwrap()).unwrap();
    let tokens = receipt
        .admit(
            &super::super::OAuthClient::new("permitted-gateway").unwrap(),
            None,
        )
        .unwrap();
    json!({"id":id,"api_key":access,"refresh_token":tokens.refresh_token,"nous_portal":tokens.context,
        "supported_models":[model],"expires_at":"2099-01-01T00:00:00Z"})
}
pub(super) struct Fixture {
    pub state: Arc<AppState>,
    id: String,
    token: String,
}
impl Fixture {
    pub async fn with_target(peer: &peer::Peer, target: InferenceTarget) -> Self {
        let root = std::env::temp_dir().join(format!("nous-public-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let mut config = support::test_config();
        config.storage_mode = crate::config::GatewayStorageMode::Local;
        config.gateway_api_key = None;
        config.gateway_api_key_secret = None;
        config.console.management_ui_enabled = false;
        config.console.state_dir = root.join("state");
        config.console.routes_file = root.join("routes.yaml");
        config.upstream_timeout_secs = 5;
        let document = json!({"providers":[{"id":"nous-public","preset":"nous-portal-oauth",
            "auth_mode":"nous_portal_oauth","base_url":target.base_url(),
            "explicit_credentials_only":true,"request_wait":{"mode":"disabled"},
            "supported_models":[MODEL],"model_map":{MODEL:UPSTREAM_MODEL},
            "credentials":[credential("unrelated","other-model",target),credential("exact",MODEL,target)]}],"model_routes":[]});
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
            json!({"ownerType":"user","ownerId":"fixture","resolvedProjectId":"local","resolvedTenantId":"local",
                "keyKind":"normal","publicKeyPrefix":"sk-gw","displayName":"Nous prepaid contract","metadata":{"models":[MODEL]}})).await;
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
            "completions" => crate::protocol::openai::normalize_legacy_completions(body.clone()),
            "embeddings" => crate::protocol::openai::normalize_embeddings(body.clone()),
            _ => crate::protocol::openai::normalize_chat_completions(body.clone()),
        }
        .unwrap();
        let candidate = self
            .state
            .route_config
            .snapshot()
            .resolve_candidates(Some(MODEL))
            .into_iter()
            .find(|c| c.provider_account_id == "nous-public")
            .unwrap();
        assert!(!crate::upstream::client::UpstreamClient::uses_native_codex(
            &candidate.payload,
            &req,
            UPSTREAM_MODEL
        ));
        assert!(
            !crate::upstream::client::UpstreamClient::uses_native_claude(
                &candidate.payload,
                &req,
                UPSTREAM_MODEL
            )
        );
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
        .expect("Nous public request deadline")
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
        crate::runtime::mark_runtime_draining(&self.state, "nous_public_finish");
        self.state.shutdown.request("nous_public_finish");
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
