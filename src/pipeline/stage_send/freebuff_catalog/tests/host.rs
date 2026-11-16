//! Production Local bootstrap, management-created normal Key and real HTTP router.
use super::*;
use crate::{config::GatewayStorageMode, http::router::build_router, state::AppState};
use axum::{body::Body, extract::State, http::Request, middleware::Next, response::Response};
use std::{path::PathBuf, sync::Arc};
pub(super) const PENDING: &str = "/v1/internal/gateway/console/freebuff-pending-purchases";
pub(super) struct Host {
    pub state: Arc<AppState>,
    pub server: peer::Server,
    pub client: rquest::Client,
    pub token: String,
    pub key_id: String,
    pub root: PathBuf,
    pub model_observed: Arc<tokio::sync::Notify>,
}
async fn loopback(
    State((origin, observed)): State<(String, Arc<tokio::sync::Notify>)>,
    request: Request<Body>,
    next: Next,
) -> Response {
    crate::protocol::freebuff::catalog::with_test_origin(
        origin,
        crate::protocol::freebuff::catalog::TEST_BODY_STARTED.scope(observed, next.run(request)),
    )
    .await
}
impl Host {
    pub async fn new(peer: &peer::Peer) -> Self {
        let root = std::env::temp_dir().join(format!(
            "gateway-local-freebuff-public-{}",
            uuid::Uuid::new_v4()
        ));
        Self::new_at(peer, root, Duration::from_secs(15)).await
    }
    pub async fn new_at(peer: &peer::Peer, root: PathBuf, timeout: Duration) -> Self {
        assert!(
            root.is_absolute() && !root.exists(),
            "Explicit new fixture state root required"
        );
        std::fs::create_dir_all(&root).unwrap();
        let mut config = support::test_config();
        config.storage_mode = GatewayStorageMode::Local;
        config.console.management_ui_enabled = false;
        config.console.state_dir = root.join("state");
        config.console.routes_file = root.join("routes.yaml");
        let routes = json!({"providers":[{"id":"pool","preset":"freebuff","base_url":"https://freebuff.com","explicit_credentials_only":true,
            "credentials":[{"id":"account","account_name":"FreeBuff public purchase","api_key":"synthetic-account","supported_models":["open"],"extra_body":{"freebuffBusiness":{"origin":"https://codebuff.com","client":"cli"}}}]}],
            "model_routes":[{"pattern":"open","provider_ids":["pool"]}]});
        std::fs::write(
            &config.console.routes_file,
            serde_yaml::to_string(&routes).unwrap(),
        )
        .unwrap();
        let state = Box::pin(crate::runtime::build_app_state(config))
            .await
            .unwrap();
        assert!(
            state.local_runtime.is_some()
                && state.redis_pool.is_closed()
                && state.pg_pool.is_none()
        );
        let model_observed = Arc::new(tokio::sync::Notify::new());
        let server = peer::Server::start(build_router(state.clone()).layer(
            axum::middleware::from_fn_with_state(
                (peer.server.origin.clone(), model_observed.clone()),
                loopback,
            ),
        ))
        .await;
        let client = rquest::Client::builder()
            .no_proxy()
            .timeout(timeout)
            .build()
            .unwrap();
        let created: Value = client.post(format!("{}/v1/internal/gateway/access/keys",server.origin))
            .header("x-management-token", support::MANAGEMENT_TOKEN)
            .json(&json!({"ownerType":"user","ownerId":"fixture","resolvedProjectId":"local","resolvedTenantId":"local","keyKind":"normal",
                "publicKeyPrefix":"sk-gw","displayName":"freebuff-public","metadata":{"models":["open"]}}))
            .send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
        let key_id = created["id"].as_str().unwrap().to_owned();
        client
            .post(format!(
                "{}/v1/internal/gateway/access/keys/{key_id}/balances/adjust",
                server.origin
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
            server,
            client,
            key_id,
            token: created["token"].as_str().unwrap().into(),
            root,
            model_observed,
        }
    }
    pub async fn seed_device(&self, peer: &peer::Peer) {
        use crate::{
            local_runtime::freebuff_devices::{RegistrationIntent, RegistrationPreparation},
            protocol::freebuff::catalog::{BusinessOrigin, CatalogSession, ClientSurface},
        };
        let local = self.state.local_runtime.as_ref().unwrap();
        let mut session = CatalogSession::new(
            BusinessOrigin::Codebuff,
            "synthetic-account",
            ClientSurface::Cli,
        )
        .unwrap();
        crate::protocol::freebuff::catalog::with_test_origin(peer.server.origin.clone(), async {
            session.refresh(None, Duration::from_secs(3)).await.unwrap();
            let RegistrationPreparation::Send(attempt) = local
                .prepare_freebuff_registration(
                    "pool",
                    "account",
                    &session,
                    RegistrationIntent::Register,
                )
                .await
                .unwrap()
            else {
                panic!("new registration")
            };
            let device = session
                .register_device(attempt.key(), Duration::from_secs(3))
                .await
                .unwrap();
            attempt.complete(&session, &device).await.unwrap();
        })
        .await;
    }
    pub async fn management(&self, path: &str, body: Value, grant: Option<&str>) -> (u16, Value) {
        let mut request = self
            .client
            .post(format!("{}{path}", self.server.origin))
            .header("origin", &self.server.origin)
            .header("x-management-token", support::MANAGEMENT_TOKEN)
            .json(&body);
        if let Some(grant) = grant {
            request = request.header("x-secret-grant", grant);
        }
        let reply = request.send().await.unwrap();
        let status = reply.status().as_u16();
        (status, reply.json().await.unwrap())
    }
    pub async fn grant(&self) -> String {
        let (status, value) = self
            .management(
                "/v1/internal/gateway/console/session/confirm-secret-access",
                json!({"token":support::MANAGEMENT_TOKEN}),
                None,
            )
            .await;
        assert_eq!(status, 200, "{value}");
        value["grant"].as_str().unwrap().into()
    }
    pub fn input(&self) -> Value {
        json!({"providerId":"pool","credentialId":"account","expectedRevision":self.state.route_config.snapshot().revision().id()})
    }
    pub async fn list(&self, grant: &str) -> Value {
        let (status, value) = self.management(PENDING, self.input(), Some(grant)).await;
        assert_eq!(status, 200, "{value}");
        value["pending"].clone()
    }
    pub async fn decision(&self, grant: &str, view: &Value, action: &str) -> u16 {
        let mut body = self.input();
        body["pendingId"] = view["pendingId"].clone();
        body["quoteId"] = view["quoteId"].clone();
        body["decision"] = json!(action);
        self.management(&format!("{PENDING}/decision"), body, Some(grant))
            .await
            .0
    }
    pub fn public(&self, body: Value) -> tokio::task::JoinHandle<(u16, Value)> {
        self.public_at("/v1/chat/completions", body)
    }
    pub fn public_at(
        &self,
        path: &'static str,
        body: Value,
    ) -> tokio::task::JoinHandle<(u16, Value)> {
        let client = self.client.clone();
        let token = self.token.clone();
        let origin = self.server.origin.clone();
        tokio::spawn(async move {
            let response = client
                .post(format!("{origin}{path}"))
                .bearer_auth(token)
                .header("cookie", "forged-caller-cookie")
                .json(&body)
                .send()
                .await
                .unwrap();
            (response.status().as_u16(), response.json().await.unwrap())
        })
    }
    pub async fn pending<T>(&self, grant: &str, task: &tokio::task::JoinHandle<T>) -> Value {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let rows = self.list(grant).await;
                if let Some(view) = rows.as_array().unwrap().first() {
                    break view.clone();
                }
                assert!(
                    !task.is_finished(),
                    "public request ended before producing a pending purchase"
                );
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap()
    }
    pub async fn balance(&self) -> i64 {
        crate::access_balance::AccessBalanceStore::from_state(&self.state)
            .unwrap()
            .get(&self.key_id)
            .await
            .unwrap()
            .unwrap()
            .remaining_messages
            .unwrap()
    }
    pub async fn close(self) {
        let root = self.root.clone();
        self.stop().await;
        crate::local_runtime::test_support::remove_test_root(&root).await;
    }
    /// An explicit browser fixture keeps only its own synthetic database as evidence after join.
    pub async fn stop(self) {
        self.server.close().await;
        self.state
            .lifecycle
            .begin_drain("freebuff_public_test_done");
        self.state.shutdown.request("freebuff_public_test_done");
        self.state
            .upstream_client
            .freebuff
            .shutdown(Duration::from_secs(2))
            .await
            .unwrap();
        self.state.local_runtime.as_ref().unwrap().close().await;
    }
}
