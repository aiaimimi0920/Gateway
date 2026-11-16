//! Real loopback router with production local owners; no upstream override or background workers.
use super::{support, KEY};
use neuro_gateway::{config::Config, http::router::build_router, state::AppState};
use serde_json::{json, Value};
use std::{net::SocketAddr, sync::Arc, time::Duration};

pub(super) struct LocalServer {
    pub state: Arc<AppState>,
    pub origin: String,
    pub port: u16,
    client: rquest::Client,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl LocalServer {
    pub async fn start(config: Config, port: Option<u16>) -> Self {
        let state = Box::pin(neuro_gateway::runtime::build_app_state(config))
            .await
            .unwrap();
        assert!(state.pg_pool.is_none() && state.redis_pool.is_closed());
        assert!(state.local_runtime.is_some() && state.auth_adapters.is_empty());
        let listener =
            tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port.unwrap_or(0)))
                .await
                .unwrap();
        let address = listener.local_addr().unwrap();
        let app = build_router(state.clone());
        let (shutdown, stop) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(async {
                let _ = stop.await;
            })
            .await
            .unwrap();
        });
        Self {
            state,
            origin: format!("http://{address}"),
            port: address.port(),
            client: rquest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap(),
            shutdown: Some(shutdown),
            task: Some(task),
        }
    }
    pub fn revision(&self) -> String {
        self.state
            .route_config
            .snapshot()
            .revision()
            .id()
            .to_owned()
    }
    pub fn assert_account(&self, provider: &str, credential: &str, present: bool) {
        let providers = self.state.route_config.get_providers();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].id, provider);
        assert_eq!(providers[0].credential_pool.len(), 1);
        let account = &providers[0].credential_pool[0];
        assert_eq!(account.id, credential);
        assert_eq!(account.payload.adapter, "lumalabs_agents_compatible");
        assert_eq!(account.payload.auth_mode.as_deref(), Some("bearer"));
        assert_eq!(account.payload.base_url, "https://agents.lumalabs.ai/v1");
        assert_eq!(account.payload.api_key, if present { KEY } else { "" });
        assert!(account.refresh_config.is_none());
    }
    pub async fn manage(
        &self,
        method: &str,
        path: &str,
        body: Value,
        grant: Option<&str>,
    ) -> (u16, Value) {
        self.manage_with_origin(method, path, body, grant, &self.origin)
            .await
    }
    pub async fn manage_with_origin(
        &self,
        method: &str,
        path: &str,
        body: Value,
        grant: Option<&str>,
        origin: &str,
    ) -> (u16, Value) {
        let mut request = self
            .client
            .request(method.parse().unwrap(), format!("{}{path}", self.origin))
            .header("x-management-token", support::MANAGEMENT_TOKEN)
            .header("origin", origin)
            .json(&body);
        if let Some(grant) = grant {
            request = request.header("x-secret-grant", grant);
        }
        let response = request.send().await.unwrap();
        let status = response.status().as_u16();
        let text = response.text().await.unwrap();
        (
            status,
            serde_json::from_str(&text).unwrap_or_else(|_| {
                panic!("Non-JSON management {method} {path}: HTTP {status}; {text}")
            }),
        )
    }
    pub async fn grant(&self) -> String {
        let (status, body) = self
            .manage(
                "POST",
                "/v1/internal/gateway/console/session/confirm-secret-access",
                json!({"token":support::MANAGEMENT_TOKEN}),
                None,
            )
            .await;
        assert_eq!(status, 200, "{body}");
        body["grant"].as_str().unwrap().to_owned()
    }
    pub async fn stop(mut self) {
        neuro_gateway::runtime::mark_runtime_draining(&self.state, "luma_local_fixture_stop");
        self.shutdown.take().unwrap().send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(10), self.task.as_mut().unwrap())
            .await
            .unwrap()
            .unwrap();
        self.task.take();
        self.state.shutdown.request("luma_local_fixture_stop");
        tokio::time::timeout(
            Duration::from_secs(10),
            self.state.local_runtime.as_ref().unwrap().close(),
        )
        .await
        .unwrap();
        // No public call or worker is started: FreeBuff is an empty default owner and drops normally.
    }
}
impl Drop for LocalServer {
    fn drop(&mut self) {
        self.state.shutdown.request("luma_local_fixture_drop");
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
