//! The actual management router, durable local console owner and synthetic Cursor peer.
mod boundaries;
mod flow;
mod lifecycle;
mod manual_probes;
mod safety;
#[path = "../../../tests/console_contract_support/mod.rs"]
mod support;
use super::*;
use crate::{
    console::{ConsoleAuthRuntime, ConsoleConfig, ConsoleConfigValues, RouteConfigRuntime},
    local_runtime::LocalRuntime,
    protocol::cursor_auth::{
        tests::{access, Peer},
        TEST_ORIGIN,
    },
    routing::config::RouteConfigYaml,
};
use axum::http::{Method, StatusCode};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
const ROOT: &str = "/v1/internal/gateway/console/cursor-auth-sessions";

fn document() -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[
        {"id":"cursor","base_url":"","auth_mode":"cursor_remote","explicit_credentials_only":true,"credentials":[]},
        {"id":"unrelated","base_url":"https://example.invalid/v1","api_key":"synthetic-unrelated-secret"}]})).unwrap()
}
struct Fixture {
    state: Arc<AppState>,
    console: ConsoleConfig,
    grant: String,
}
impl Fixture {
    async fn new() -> Self {
        let root = std::env::temp_dir().join(format!("cursor-import-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let console = ConsoleConfig::from_values(ConsoleConfigValues {
            state_dir: Some(root.join("console")),
            routes_file: Some(root.join("routes.yaml")),
            ..Default::default()
        })
        .unwrap();
        std::fs::write(
            &console.routes_file,
            serde_yaml::to_string(&document()).unwrap(),
        )
        .unwrap();
        let runtime = Arc::new(RouteConfigRuntime::new_local(&console).unwrap());
        runtime.recover_startup().await.unwrap();
        let mut state = support::build_state(runtime.route_config().clone(), Some(runtime));
        let inner = Arc::get_mut(&mut state).unwrap();
        inner.local_runtime = Some(LocalRuntime::open(&root.join("state")).await.unwrap());
        inner.console_auth = Arc::new(
            ConsoleAuthRuntime::new(&console, Some(support::MANAGEMENT_TOKEN.into())).unwrap(),
        );
        let grant = support::grant_console_secret_access(&state).await;
        Self {
            state,
            console,
            grant,
        }
    }
    fn body(&self, manual: bool) -> Value {
        let mut value = json!({"attemptId":uuid::Uuid::new_v4().to_string(),"providerId":"cursor",
            "expectedRevision":self.state.route_config.snapshot().revision().id()});
        if manual {
            value["accessToken"] = json!(access());
        }
        value
    }
    async fn call(
        &self,
        method: Method,
        path: &str,
        body: Value,
        grant: Option<&str>,
    ) -> axum::response::Response {
        support::send_console_json(&self.state, method, path, body, grant, None).await
    }
    async fn create(&self, peer: &Peer, body: Value) -> Value {
        let response = TEST_ORIGIN
            .scope(
                peer.origin.clone(),
                self.call(Method::POST, ROOT, body, Some(&self.grant)),
            )
            .await;
        let status = response.status();
        let value = support::parse_json(response).await;
        assert_eq!(status, StatusCode::OK, "{value}");
        value["session"].clone()
    }
    async fn wait(&self, id: &str, expected: &str) -> Value {
        tokio::time::timeout(Duration::from_secs(8), async {
            loop {
                let response = self
                    .call(Method::GET, &format!("{ROOT}/{id}"), Value::Null, None)
                    .await;
                assert_eq!(response.status(), StatusCode::OK);
                let value = support::parse_json(response).await["session"].clone();
                if value["status"] == expected {
                    return value;
                }
                assert!(
                    !["failed", "cancelled", "expired", "save_unconfirmed"]
                        .iter()
                        .any(|s| value["status"] == *s),
                    "{value}"
                );
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("Cursor import terminal receipt")
    }
    async fn close(&self) {
        self.state.lifecycle.cursor_import.join().await;
        self.state.local_runtime.as_ref().unwrap().close().await;
    }
}
