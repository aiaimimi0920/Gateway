//! Real management-created Key and Local SQLite are authoritative for balance/audit assertions.
use super::{
    support,
    upstream::{Mode, Server, Upstream, DEADLINE},
};
use neuro_gateway::{
    access_balance::AccessBalanceStore,
    config::GatewayStorageMode,
    db::{GatewayRequestAuditView, RequestAuditFilters},
    http::router::build_router,
    state::AppState,
};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
use tokio::time::timeout;

pub struct Fixture {
    pub state: Arc<AppState>,
    pub token: String,
    pub key_id: String,
    pub client: rquest::Client,
    pub local: Server,
    pub upstream: Upstream,
    remote: Server,
    limits: Vec<(String, usize)>,
}
impl Fixture {
    pub async fn start(mode: Mode) -> Self {
        let upstream = Upstream::new(mode);
        let remote = Server::start(upstream.router()).await;
        let root =
            std::env::temp_dir().join(format!("gateway-bedrock-tcp-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let mut config = support::test_config();
        config.upstream_timeout_secs = 30;
        config.storage_mode = GatewayStorageMode::Local;
        config.console.state_dir = root.join("state");
        config.console.routes_file = root.join("routes.yaml");
        let provider = |id: &str| {
            json!({"id":id,"adapter":"bedrock_converse_compatible","protocol_profile":"aws_bedrock",
            "base_url":remote.origin,"supported_models":["fixture-model"],"extra_body":{"system":[{"text":"policy-secret"}]},
            "credentials":[{"id":format!("{id}-account"),"api_key":"synthetic-only","supported_models":["fixture-model"]}]})
        };
        let routes = json!({"providers":[provider("primary"),provider("fallback")],"model_routes":[{"pattern":"fixture-model","provider_ids":["primary","fallback"]}],"aliases":{}});
        std::fs::write(
            &config.console.routes_file,
            serde_yaml::to_string(&routes).unwrap(),
        )
        .unwrap();
        let state = Box::pin(neuro_gateway::runtime::build_app_state(config))
            .await
            .unwrap();
        assert!(
            state.local_runtime.is_some()
                && state.redis_pool.is_closed()
                && state.pg_pool.is_none()
        );
        let limits = state
            .route_config
            .snapshot()
            .resolve_candidates(Some("fixture-model"))
            .iter()
            .map(|candidate| {
                let id = candidate.provider_account_id.clone();
                let limit = state
                    .concurrency_registry
                    .get_or_create(&id)
                    .snapshot()
                    .current_limit;
                (id, limit)
            })
            .collect();
        let local = Server::start(build_router(state.clone())).await;
        let client = rquest::Client::builder()
            .no_proxy()
            .timeout(DEADLINE)
            .build()
            .unwrap();
        let created: Value = client.post(format!("{}/v1/internal/gateway/access/keys", local.origin))
            .header("x-management-token", support::MANAGEMENT_TOKEN)
            .json(&json!({"ownerType":"user","ownerId":"fixture","resolvedProjectId":"local","resolvedTenantId":"local",
                "keyKind":"normal","publicKeyPrefix":"sk-gw","displayName":"bedrock-tcp","metadata":{"models":["fixture-model"]}}))
            .send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
        let key_id = created["id"].as_str().unwrap().to_owned();
        let token = created["token"].as_str().unwrap().to_owned();
        client
            .post(format!(
                "{}/v1/internal/gateway/access/keys/{key_id}/balances/adjust",
                local.origin
            ))
            .header("x-management-token", support::MANAGEMENT_TOKEN)
            .json(
                &json!({"balanceMode":"message_prepaid","totalMessages":10,"remainingMessages":10}),
            )
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
        Self {
            state,
            token,
            key_id,
            client,
            local,
            upstream,
            remote,
            limits,
        }
    }
    pub async fn post(&self, path: &str, body: Value) -> rquest::Response {
        self.client
            .post(format!("{}{path}", self.local.origin))
            .bearer_auth(&self.token)
            .header("cookie", "caller-must-not-forward")
            .json(&body)
            .send()
            .await
            .unwrap()
    }
    pub async fn balance(&self) -> i64 {
        AccessBalanceStore::from_state(&self.state)
            .unwrap()
            .get(&self.key_id)
            .await
            .unwrap()
            .unwrap()
            .remaining_messages
            .unwrap()
    }
    pub async fn settled(&self, count: usize, status: &str) -> Vec<GatewayRequestAuditView> {
        let local = self.state.local_runtime.as_ref().unwrap();
        let rows = timeout(DEADLINE, async {
            loop {
                let rows = local
                    .list_audits(&RequestAuditFilters::default())
                    .await
                    .unwrap();
                if rows.len() == count && rows.iter().all(|r| r.status != "running") {
                    break rows;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        for row in &rows {
            assert_eq!(row.status, status);
            assert_eq!(row.access_key_id.as_deref(), Some(self.key_id.as_str()));
            // A normal Local Key has no delegated platform/source key to attribute.
            assert!(row.source_access_key_id.is_none());
            assert!(self
                .limits
                .iter()
                .any(|(id, _)| row.provider_account_id.as_ref() == Some(id)));
            assert_eq!(row.resolved_model.as_deref(), Some("fixture-model"));
            assert_eq!(row.route_trace.as_ref().unwrap()["routeAttemptCount"], 1);
            assert!(row.completed_at.is_some());
            assert!(
                row.client_disconnected_at.is_none(),
                "Drop alone does not identify client intent"
            );
        }
        assert_eq!(
            self.upstream.calls.lock().unwrap().len(),
            count,
            "one send per request; no replay/fallback"
        );
        for (id, _) in &self.limits {
            assert_eq!(
                self.state
                    .concurrency_registry
                    .get_or_create(id)
                    .snapshot()
                    .active_count,
                0
            );
        }
        assert_eq!(
            self.balance().await,
            10 - if status == "completed" {
                count as i64
            } else {
                0
            }
        );
        if status == "completed" {
            assert_eq!(self.successes(), count);
        }
        rows
    }
    pub fn successes(&self) -> usize {
        self.limits
            .iter()
            .map(|(id, _)| {
                self.state
                    .concurrency_registry
                    .get_or_create(id)
                    .snapshot()
                    .consecutive_successes
            })
            .sum()
    }
    pub fn neutral(&self) {
        for (id, limit) in &self.limits {
            let snapshot = self.state.concurrency_registry.get_or_create(id).snapshot();
            assert_eq!(snapshot.current_limit, *limit);
            assert_eq!(snapshot.consecutive_successes, 0);
        }
    }
    pub async fn close(self) {
        self.local.close().await;
        self.remote.close().await;
        self.state.shutdown.request("bedrock_public_tcp_test");
        timeout(DEADLINE, self.state.local_runtime.as_ref().unwrap().close())
            .await
            .unwrap();
    }
}
