//! Exercises real build_app_state, background spawn and run_gateway_server over loopback TCP.
use super::{
    server_fixture::Fixture,
    server_peer::{self, Peer},
};
use crate::state::AppState;
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use tokio::task::JoinHandle;
pub(super) struct Server {
    pub state: Arc<AppState>,
    pub origin: String,
    client: rquest::Client,
    task: Option<JoinHandle<anyhow::Result<()>>>,
}
impl Server {
    pub async fn start(f: &Fixture, peer: &Peer, refresh: bool) -> Self {
        assert_eq!(
            std::env::var("GATEWAY_BIND_HOST").as_deref(),
            Ok("127.0.0.1")
        );
        let mut config = f.config.clone();
        config.provider_credential_refresh_enabled = refresh;
        let mut state = Box::pin(crate::runtime::build_app_state(config))
            .await
            .unwrap();
        assert!(state.local_runtime.is_none() && state.pg_pool.is_some());
        // Missed HTTPS seams hit a rejecting, non-forwarding proxy rather than a supplier.
        let proxy = rquest::Proxy::all(&peer.origin)
            .unwrap()
            .no_proxy(rquest::NoProxy::from_string("127.0.0.1"));
        Arc::get_mut(&mut state).unwrap().upstream_client.http = rquest::Client::builder()
            .no_proxy()
            .proxy(proxy)
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        server_peer::within(peer.origin.clone(), async {
            crate::runtime::spawn_background_tasks(&state);
        })
        .await;
        assert_eq!(
            state.lifecycle.accio_db_refresh_task.lock().is_some(),
            refresh
        );
        let reserved = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = reserved.local_addr().unwrap().port();
        drop(reserved);
        let owned = state.clone();
        let origin = format!("http://127.0.0.1:{port}");
        let endpoint = peer.origin.clone();
        let task = tokio::spawn(async move {
            server_peer::within(
                endpoint,
                Box::pin(crate::runtime::run_gateway_server(owned, port)),
            )
            .await
        });
        let client = rquest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap();
        let server = Self {
            state,
            origin,
            client,
            task: Some(task),
        };
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                assert!(
                    !server.task.as_ref().unwrap().is_finished(),
                    "real server exited before readiness"
                );
                if server
                    .client
                    .get(format!("{}/healthz", server.origin))
                    .send()
                    .await
                    .is_ok()
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        server
    }
    pub async fn request(
        &self,
        method: &str,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (u16, String) {
        let mut request = self
            .client
            .request(method.parse().unwrap(), format!("{}{path}", self.origin));
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await.unwrap();
        let status = response.status().as_u16();
        (status, response.text().await.unwrap())
    }
    pub async fn manage(&self, method: &str, path: &str, body: Option<Value>) -> Value {
        let (status, text) = self
            .request(method, path, Some("server-management-synthetic"), body)
            .await;
        assert_eq!(status, 200, "management HTTP status {status}: {text}");
        serde_json::from_str(&text).unwrap()
    }
    pub async fn stop(mut self) {
        self.state.shutdown.request("accio-db-server-contract");
        tokio::time::timeout(Duration::from_secs(20), self.task.take().unwrap())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(self.state.lifecycle.accio_db_refresh_task.lock().is_none());
        self.state.pg_pool.as_ref().unwrap().close().await;
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            self.state.shutdown.request("accio-test-drop");
            task.abort();
        }
    }
}
