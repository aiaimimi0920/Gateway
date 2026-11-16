//! Production local bootstrap, real TCP requests and bounded graceful fixture ownership.
use super::{support, wire, MODEL};
use crate::{config::Config, http::router::build_router, state::AppState};
use axum::{body::Body, extract::State, http::Request, middleware::Next, response::Response};
use serde_json::{json, Value};
use std::{net::SocketAddr, sync::Arc, time::Duration};

pub(crate) struct LocalServer {
    pub state: Arc<AppState>,
    pub origin: String,
    pub port: u16,
    client: rquest::Client,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<()>>,
}

async fn loopback_transport(
    State((origin, oidc)): State<(String, Option<String>)>,
    request: Request<Body>,
    next: Next,
) -> Response {
    // The seam is cfg(test)-only and runs after normal product planning and final header validation.
    let request = wire::within(&origin, next.run(request));
    if let Some(oidc) = oidc {
        crate::protocol::kiro_oauth::TEST_ORIGIN
            .scope(oidc, request)
            .await
    } else {
        request.await
    }
}

impl LocalServer {
    pub async fn start(config: Config, upstream: &wire::Server, port: Option<u16>) -> Self {
        Self::start_with_oidc(config, upstream, None, port).await
    }
    pub async fn start_with_oidc(
        config: Config,
        upstream: &wire::Server,
        oidc: Option<String>,
        port: Option<u16>,
    ) -> Self {
        let mut state = Box::pin(crate::runtime::build_app_state(config))
            .await
            .unwrap();
        assert!(state.pg_pool.is_none() && state.redis_pool.is_closed());
        assert!(state.local_runtime.is_some() && state.auth_adapters.is_empty());
        Arc::get_mut(&mut state).unwrap().upstream_client.http = upstream.client();
        let listener =
            tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port.unwrap_or(0)))
                .await
                .unwrap();
        let address = listener.local_addr().unwrap();
        let app = build_router(state.clone()).layer(axum::middleware::from_fn_with_state(
            (upstream.origin.clone(), oidc),
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
            .json(&body);
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
        let response = self
            .open(
                "chat/completions",
                token,
                json!({"model":MODEL,"stream":stream,
            "messages":[{"role":"user","content":"local question"}]}),
            )
            .await;
        (response.status().as_u16(), response.text().await.unwrap())
    }

    pub async fn open(&self, endpoint: &str, token: &str, body: Value) -> rquest::Response {
        self.try_open(endpoint, token, body).await.unwrap()
    }
    pub async fn try_open(
        &self,
        endpoint: &str,
        token: &str,
        body: Value,
    ) -> Result<rquest::Response, rquest::Error> {
        self.client
            .post(format!("{}/v1/{endpoint}", self.origin))
            .bearer_auth(token)
            .header("cookie", "caller=must-not-forward")
            .header("user-agent", "caller-must-not-forward")
            .json(&body)
            .send()
            .await
    }

    pub async fn stop(mut self) {
        crate::runtime::mark_runtime_draining(&self.state, "kiro_key_fixture_stop");
        self.shutdown.take().unwrap().send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(10), self.task.as_mut().unwrap())
            .await
            .unwrap()
            .unwrap();
        self.task.take();
        self.state.shutdown.request("kiro_key_fixture_stop");
        self.state.lifecycle.kiro_oauth.join().await;
        self.state.local_runtime.as_ref().unwrap().close().await;
        self.state
            .upstream_client
            .freebuff
            .shutdown(Duration::from_secs(1))
            .await
            .unwrap();
    }
}

impl Drop for LocalServer {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
