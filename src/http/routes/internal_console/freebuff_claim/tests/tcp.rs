//! Production Local bootstrap and TCP; only the external FreeBuff transport is synthetic.
use super::*;
use crate::{
    config::{Config, GatewayStorageMode},
    http::router::build_router,
};
use axum::{body::Body, http::Request, middleware::Next};
use std::{net::SocketAddr, path::Path, time::Duration};
mod roundtrip;
mod seed;
pub(super) const TOKEN: &str = "synthetic-claim-business-token";
pub(super) fn config(root: &Path) -> Config {
    assert!(
        root.is_absolute() && !root.exists(),
        "Explicit new fixture root required"
    );
    std::fs::create_dir_all(root).unwrap();
    let mut config = support::test_config();
    config.storage_mode = GatewayStorageMode::Local;
    config.gateway_api_key = None;
    config.gateway_api_key_secret = None;
    config.console.management_ui_enabled = false;
    config.console.state_dir = root.join("state");
    config.console.routes_file = root.join("routes.yaml");
    let document = json!({"providers":[{"id":"pool","preset":"freebuff","base_url":"https://freebuff.com",
        "explicit_credentials_only":true,"supported_models":[],"credentials":[
            {"id":"a","api_key":"synthetic-untouched-account","account_name":"Untouched account","enabled":false},
            {"id":"b","api_key":TOKEN,"account_name":"FreeBuff original claim","enabled":false,"supported_models":[],
             "extra_body":{"freebuffBusiness":{"origin":"https://codebuff.com","client":"cli"}}}]}],"model_routes":[]});
    std::fs::write(
        &config.console.routes_file,
        serde_yaml::to_string(&document).unwrap(),
    )
    .unwrap();
    config
}
async fn loopback(State(origin): State<String>, request: Request<Body>, next: Next) -> Response {
    // Scope both private test transports so an accidental login can never contact a real platform.
    crate::protocol::freebuff_login::TEST_ORIGIN
        .scope(
            origin.clone(),
            crate::protocol::freebuff::catalog::with_test_origin(origin, next.run(request)),
        )
        .await
}
pub(super) struct Host {
    pub state: Arc<AppState>,
    pub origin: String,
    pub port: u16,
    client: rquest::Client,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl Host {
    pub fn start<'a>(
        config: Config,
        peer: &'a wire::Peer,
        port: Option<u16>,
    ) -> std::pin::Pin<Box<impl std::future::Future<Output = Self> + 'a>> {
        Box::pin(async move {
            let state = Box::pin(crate::runtime::build_app_state(config))
                .await
                .unwrap();
            assert!(
                state.pg_pool.is_none()
                    && state.redis_pool.is_closed()
                    && state.local_runtime.is_some()
            );
            assert!(state.auth_adapters.is_empty());
            let listener =
                tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port.unwrap_or(0)))
                    .await
                    .unwrap();
            let address = listener.local_addr().unwrap();
            let app = build_router(state.clone()).layer(axum::middleware::from_fn_with_state(
                peer.origin.clone(),
                loopback,
            ));
            let (stop, stopped) = tokio::sync::oneshot::channel();
            let task = tokio::spawn(async move {
                axum::serve(
                    listener,
                    app.into_make_service_with_connect_info::<SocketAddr>(),
                )
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
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
                    .timeout(Duration::from_secs(25))
                    .build()
                    .unwrap(),
                stop: Some(stop),
                task: Some(task),
            }
        })
    }
    pub fn revision(&self) -> String {
        self.state
            .route_config
            .snapshot()
            .revision()
            .id()
            .to_owned()
    }
    pub fn input(&self) -> Value {
        json!({"providerId":"pool","credentialId":"b","expectedRevision":self.revision()})
    }
    pub async fn request(&self, path: &str, body: Value, grant: Option<&str>) -> (u16, Value) {
        let mut request = self
            .client
            .post(format!("{}{path}", self.origin))
            .header("origin", &self.origin)
            .header("x-management-token", support::MANAGEMENT_TOKEN)
            .json(&body);
        if let Some(grant) = grant {
            request = request.header("x-secret-grant", grant);
        }
        let response = request.send().await.unwrap();
        let status = response.status().as_u16();
        if path.starts_with(ENDPOINT) {
            assert_eq!(response.headers()["cache-control"], "no-store");
        }
        let text = response.text().await.unwrap();
        (
            status,
            serde_json::from_str(&text)
                .unwrap_or_else(|_| panic!("Non-JSON fixture management response: {text}")),
        )
    }
    pub async fn grant(&self) -> String {
        let (status, body) = self
            .request(
                "/v1/internal/gateway/console/session/confirm-secret-access",
                json!({"token":support::MANAGEMENT_TOKEN}),
                None,
            )
            .await;
        assert_eq!(status, 200, "{body}");
        body["grant"].as_str().unwrap().to_owned()
    }
    pub async fn seed_original_claim(&self, peer: &wire::Peer) -> String {
        seed::original(self, peer).await
    }
    pub async fn counts(&self) -> (i64, i64) {
        let pool = &self.state.local_runtime.as_ref().unwrap().pool;
        let active = sqlx::query_scalar("SELECT count(*) FROM local_freebuff_session_claims")
            .fetch_one(pool)
            .await
            .unwrap();
        let history = sqlx::query_scalar("SELECT count(*) FROM local_freebuff_session_closeouts")
            .fetch_one(pool)
            .await
            .unwrap();
        (active, history)
    }
    pub async fn stop(mut self) {
        crate::runtime::mark_runtime_draining(&self.state, "freebuff_claim_tcp_fixture_stop");
        self.stop.take().unwrap().send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(10), self.task.as_mut().unwrap())
            .await
            .unwrap()
            .unwrap();
        self.task.take();
        self.state
            .shutdown
            .request("freebuff_claim_tcp_fixture_stop");
        tokio::time::timeout(Duration::from_secs(25), async {
            self.state
                .upstream_client
                .freebuff
                .shutdown(Duration::from_secs(2))
                .await
                .unwrap();
            self.state.local_runtime.as_ref().unwrap().close().await;
        })
        .await
        .unwrap();
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        self.state
            .shutdown
            .request("freebuff_claim_tcp_fixture_drop");
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
pub(super) fn assert_end_wire(peer: &wire::Peer, id: &str) {
    let hits = peer.hits.lock().unwrap();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].0, "GET");
    assert_eq!(hits[0].1, "/api/v1/freebuff/models");
    let (method, path, headers, body) = &hits[1];
    assert_eq!(method, "DELETE");
    assert_eq!(path, "/api/v1/freebuff/session/attempt");
    assert!(body.is_empty());
    assert_eq!(headers["authorization"], format!("Bearer {TOKEN}"));
    assert_eq!(headers["x-freebuff-instance-id"], format!("cli:{id}"));
    assert_eq!(headers["x-freebuff-desktop-attempt-id"], id);
    assert!(headers.contains_key("x-freebuff-device-sig"));
}
