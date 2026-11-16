//! Owns the real HTTP/SQLite lifetime and the existing production refresh loop with explicit test peers.
use super::*;
use crate::{http::router::build_router, state::AppState};
use axum::{body::Body, extract::State, http::Request, middleware::Next, response::Response};
use std::{net::SocketAddr, sync::Arc};

#[derive(Clone)]
struct Peers {
    model: String,
    issuer: String,
}
async fn loopback_transport(
    State(peers): State<Peers>,
    request: Request<Body>,
    next: Next,
) -> Response {
    crate::protocol::vertex_oauth::transport_test_support::test_endpoint(
        peers.issuer,
        wire::within(&peers.model, next.run(request)),
    )
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
        upstream: &'a wire::Server,
        issuer: &'a Issuer,
        port: Option<u16>,
    ) -> std::pin::Pin<Box<impl std::future::Future<Output = Self> + 'a>> {
        Box::pin(async move {
            let mut state = Box::pin(crate::runtime::build_app_state(config))
                .await
                .unwrap();
            assert!(
                state.pg_pool.is_none()
                    && state.redis_pool.is_closed()
                    && state.local_runtime.is_some()
            );
            assert!(state.auth_adapters.is_empty());
            // Permit the two explicit loopback peers; any missed HTTPS seam still hits the rejecting proxy.
            let proxy = rquest::Proxy::all(&upstream.origin)
                .unwrap()
                .no_proxy(rquest::NoProxy::from_string("127.0.0.1"));
            Arc::get_mut(&mut state).unwrap().upstream_client.http = rquest::Client::builder()
                .no_proxy()
                .proxy(proxy)
                .timeout(Duration::from_secs(5))
                .redirect(rquest::redirect::Policy::limited(3))
                .build()
                .unwrap();
            state.lifecycle.vertex_oauth.spawn(state.shutdown.clone());
            let listener =
                tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port.unwrap_or(0)))
                    .await
                    .unwrap();
            let address = listener.local_addr().unwrap();
            let app = build_router(state.clone()).layer(axum::middleware::from_fn_with_state(
                Peers {
                    model: upstream.origin.clone(),
                    issuer: issuer.endpoint.clone(),
                },
                loopback_transport,
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
                    .timeout(Duration::from_secs(10))
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
    pub async fn call(&self, token: &str, stream: bool) -> (u16, String) {
        let response = self.client.post(format!("{}/v1/chat/completions", self.origin)).bearer_auth(token)
            .json(&json!({"model":MODEL,"stream":stream,"messages":[{"role":"user","content":"local question"}]}))
            .send().await.unwrap();
        (response.status().as_u16(), response.text().await.unwrap())
    }
    pub fn start_refresh(&self, issuer: &Issuer) {
        let mut slot = self.state.lifecycle.oauth_refresh_task.lock();
        assert!(slot.is_none(), "Only the original refresh owner may run");
        // Only the due margin changes for synthetic long-lived material; the production loop and cadence are retained.
        let task = crate::token_refresh::start_token_refresh_task(self.state.clone(), 60, 7200);
        *slot = Some(tokio::spawn(
            crate::protocol::vertex_oauth::transport_test_support::test_endpoint(
                issuer.endpoint.clone(),
                task,
            ),
        ));
    }
    pub async fn stop(mut self) {
        crate::runtime::mark_runtime_draining(&self.state, "vertex_local_fixture_stop");
        self.shutdown.take().unwrap().send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(10), self.task.as_mut().unwrap())
            .await
            .unwrap()
            .unwrap();
        self.task.take();
        self.state.shutdown.request("vertex_local_fixture_stop");
        tokio::time::timeout(Duration::from_secs(15), async {
            self.state.lifecycle.vertex_oauth.join().await;
            crate::token_refresh::join(&self.state).await;
            self.state.local_runtime.as_ref().unwrap().close().await;
            self.state
                .upstream_client
                .freebuff
                .shutdown(Duration::from_secs(1))
                .await
                .unwrap();
        })
        .await
        .unwrap();
    }
}
impl Drop for LocalServer {
    fn drop(&mut self) {
        self.state.shutdown.request("vertex_local_fixture_drop");
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(task) = &self.task {
            task.abort();
        }
        self.state.lifecycle.vertex_oauth.shutdown();
    }
}
