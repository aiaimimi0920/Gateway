//! Synthetic DIM authority, actual management router, loopback callback and normal durable save.
use crate::{routing::config::RouteConfigYaml, state::AppState};
use axum::http::{Method, StatusCode};
use serde_json::{json, Value};
use std::sync::Arc;
mod catalog;
mod fixture;
mod flow;
mod safety;
mod saving;
#[path = "../../../../../tests/console_contract_support/mod.rs"]
mod support;
mod wire;
const ROOT: &str = "/v1/internal/gateway/console/dim-auth-sessions";
fn document() -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[{"id":"dim", "auth_mode":"dim_user_oauth",
        "explicit_credentials_only":true,"base_url":"https://dimagent.cn/v1","credentials":[]},
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
