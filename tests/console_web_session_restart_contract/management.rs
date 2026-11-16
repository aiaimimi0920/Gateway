//! Normal management TCP/Grant/CAS writes must reach the real local authority.
use super::*;
use neuro_gateway::console::secrets::redact_route_document;
use neuro_gateway::http::router::build_router;
use neuro_gateway::state::AppState;
use reqwest::{Client, Method, StatusCode};
use std::{net::SocketAddr, sync::Arc, time::Duration};
#[path = "../console_contract_support/mod.rs"]
mod console;

const ROUTES: &str = "/v1/internal/gateway/console/route-config";

struct Server {
    state: Arc<AppState>,
    origin: String,
    client: Client,
    shutdown: tokio::sync::oneshot::Sender<()>,
    task: tokio::task::JoinHandle<()>,
}

impl Server {
    async fn open(harness: &RuntimeHarness) -> Self {
        let runtime = Arc::new(RouteConfigRuntime::new_local(&config(harness)).unwrap());
        runtime.recover_startup().await.unwrap();
        let mut state = console::build_state(runtime.route_config().clone(), Some(runtime));
        Arc::get_mut(&mut state).unwrap().config.storage_mode =
            neuro_gateway::config::GatewayStorageMode::Local;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let app = build_router(state.clone());
        let (shutdown, stopped) = tokio::sync::oneshot::channel();
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
            origin,
            shutdown,
            task,
            client: Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap(),
        }
    }

    async fn request(
        &self,
        method: Method,
        path: &str,
        body: Value,
        token: bool,
        grant: Option<&str>,
    ) -> (StatusCode, Value) {
        let mut request = self
            .client
            .request(method, format!("{}{path}", self.origin))
            .json(&body);
        if token {
            request = request.header("x-management-token", console::MANAGEMENT_TOKEN);
        }
        if let Some(grant) = grant {
            request = request.header("x-secret-grant", grant);
        }
        let response = request.send().await.unwrap();
        let status = response.status();
        let bytes = response.bytes().await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    async fn grant(&self) -> String {
        let (status, body) = self
            .request(
                Method::POST,
                "/v1/internal/gateway/console/session/confirm-secret-access",
                json!({"token":console::MANAGEMENT_TOKEN}),
                true,
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        body["grant"].as_str().unwrap().to_owned()
    }

    async fn read(&self, expected: &RouteConfigYaml, revision: &str) {
        let (status, body) = self
            .request(Method::GET, ROUTES, Value::Null, true, None)
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["routeConfig"]["revision"]["id"], revision);
        assert_eq!(
            body["routeConfig"]["document"],
            value(&redact_route_document(expected).unwrap().document)
        );
        assert_eq!(body["routeConfig"]["mutationSupported"], true);
        let rendered = body.to_string();
        for secret in [
            "synthetic-new",
            "synthetic-old",
            "synthetic-other",
            "synthetic-clearance",
        ] {
            assert!(
                !rendered.contains(secret),
                "management response exposed full material"
            );
        }
        assert_routes(&self.state.route_config.snapshot(), expected);
    }

    async fn close(self) {
        self.shutdown.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(5), self.task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            Arc::strong_count(&self.state),
            1,
            "server retained AppState"
        );
        // Dropping the final state releases its runtime/store before the next listener starts.
    }
}

fn commit_body(document: &RouteConfigYaml, revision: &str) -> Value {
    let raw = value(document);
    let redacted = redact_route_document(document).unwrap();
    let patches: Vec<_> = redacted.secrets.iter().filter(|secret| secret.configured).map(|secret| {
        if secret.path.starts_with("/providers/0/credentials/0/") {
            json!({"path":secret.path,"operation":"replace","value":raw.pointer(&secret.path).unwrap()})
        } else {
            json!({"path":secret.path,"operation":"keep"})
        }
    }).collect();
    json!({"expectedRevision":revision,"document":redacted.document,"secretPatches":patches})
}

#[tokio::test]
async fn web_session_management_tcp_save_reopen_and_grant_cas_boundaries() {
    for grok in [true, false] {
        let original = document(grok, false);
        let rotated = document(grok, true);
        let harness = RuntimeHarness::new(
            "web-session-management",
            original.clone(),
            FakeRedisMode::Activated,
            true,
        );
        let server = Server::open(&harness).await;
        let revision = server
            .state
            .route_config
            .snapshot()
            .revision()
            .id()
            .to_owned();
        let initial_yaml = fs::read(&harness.routes).unwrap();
        let body = commit_body(&rotated, &revision);
        let (status, _) = server
            .request(Method::PUT, ROUTES, body.clone(), false, None)
            .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (status, denied) = server
            .request(Method::PUT, ROUTES, body.clone(), true, None)
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(denied["error"]["code"], "console_secret_access_required");
        assert_eq!(fs::read(&harness.routes).unwrap(), initial_yaml);
        server.read(&original, &revision).await;
        let grant = server.grant().await;
        let (status, saved) = server
            .request(Method::PUT, ROUTES, body, true, Some(&grant))
            .await;
        assert_eq!(status, StatusCode::OK, "{saved}");
        let next = saved["routeConfig"]["revision"]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        assert_ne!(next, revision);
        server.read(&rotated, &next).await;
        server.close().await;

        // No inherited AppState or fake Redis authority: fresh listener and disk-loaded owners.
        let server = Server::open(&harness).await;
        server.read(&rotated, &next).await;
        let yaml = fs::read(&harness.routes).unwrap();
        let (status, denied) = server
            .request(
                Method::PUT,
                ROUTES,
                commit_body(&original, &next),
                true,
                Some(&grant),
            )
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(denied["error"]["code"], "console_secret_access_required");
        let fresh_grant = server.grant().await;
        let (status, conflict) = server
            .request(
                Method::PUT,
                ROUTES,
                commit_body(&original, &revision),
                true,
                Some(&fresh_grant),
            )
            .await;
        assert_eq!(status, StatusCode::CONFLICT, "{conflict}");
        assert_eq!(fs::read(&harness.routes).unwrap(), yaml);
        server.read(&rotated, &next).await;
        server.close().await;
        reopen(&harness, &rotated, &next).await;
    }
}
