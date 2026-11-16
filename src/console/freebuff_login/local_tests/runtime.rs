//! Real production bootstrap/router and explicit awaited HTTP, login owner, executor and SQLite shutdown.
use super::*;
use crate::{http::router::build_router, state::AppState};
use axum::{body::Body, extract::State, http::Request, middleware::Next, response::Response};
use std::{net::SocketAddr, sync::Arc};

async fn loopback(State(origin): State<String>, request: Request<Body>, next: Next) -> Response {
    crate::protocol::freebuff_login::TEST_ORIGIN
        .scope(origin, next.run(request))
        .await
}
pub(super) struct LocalServer {
    pub state: Arc<AppState>,
    pub origin: String,
    pub port: u16,
    client: rquest::Client,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl LocalServer {
    pub fn start<'a>(
        config: Config,
        peer: &'a Peer,
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
            state.lifecycle.freebuff_login.spawn(state.shutdown.clone());
            let listener =
                tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port.unwrap_or(0)))
                    .await
                    .unwrap();
            let address = listener.local_addr().unwrap();
            let app = build_router(state.clone()).layer(axum::middleware::from_fn_with_state(
                peer.origin.clone(),
                loopback,
            ));
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
                    .timeout(Duration::from_secs(25))
                    .build()
                    .unwrap(),
                shutdown: Some(shutdown),
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
    pub async fn manage(
        &self,
        method: &str,
        path: &str,
        body: Value,
        grant: Option<&str>,
    ) -> (u16, Value) {
        let mut request = self
            .client
            .request(method.parse().unwrap(), format!("{}{path}", self.origin))
            .header("x-management-token", support::MANAGEMENT_TOKEN)
            .header("origin", &self.origin);
        if method != "GET" {
            request = request.json(&body);
        }
        if let Some(grant) = grant {
            request = request.header("x-secret-grant", grant);
        }
        let response = request.send().await.unwrap();
        let status = response.status().as_u16();
        if path.starts_with(AUTH) {
            assert_eq!(response.headers()["cache-control"], "no-store");
        }
        let text = response.text().await.unwrap();
        (
            status,
            serde_json::from_str(&text)
                .unwrap_or_else(|_| panic!("Non-JSON management response: {text}")),
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
        crate::runtime::mark_runtime_draining(&self.state, "freebuff_local_fixture_stop");
        self.shutdown.take().unwrap().send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(10), self.task.as_mut().unwrap())
            .await
            .unwrap()
            .unwrap();
        self.task.take();
        self.state.shutdown.request("freebuff_local_fixture_stop");
        tokio::time::timeout(Duration::from_secs(25), async {
            self.state.lifecycle.freebuff_login.join().await;
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
impl Drop for LocalServer {
    fn drop(&mut self) {
        self.state.shutdown.request("freebuff_local_fixture_drop");
        self.state.lifecycle.freebuff_login.shutdown();
        if let Some(stop) = self.shutdown.take() {
            let _ = stop.send(());
        }
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
