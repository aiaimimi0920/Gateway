//! Actual management router, local durable configuration and synthetic non-forwarding protocol peer.
use crate::{routing::config::RouteConfigYaml, state::AppState};
use axum::http::{Method, StatusCode};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
mod fixture;
mod flow;
mod safety;
mod saving;
#[path = "../../../../../tests/console_contract_support/mod.rs"]
mod support;
const ROOT: &str = "/v1/internal/gateway/console/lobsterai-auth-sessions";
fn document() -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[{"id":"lobster","auth_mode":"lobsterai_session","adapter":"openai_compatible",
        "explicit_credentials_only":true,"base_url":"https://lobsterai-server.youdao.com/api/proxy/v1","credentials":[]},
        {"id":"other","base_url":"https://example.invalid/v1","api_key":"synthetic-unrelated-secret"}]})).unwrap()
}
async fn call(
    state: &Arc<AppState>,
    path: &str,
    body: Value,
    grant: Option<&str>,
) -> axum::response::Response {
    support::send_console_json(state, Method::POST, path, body, grant, None).await
}
