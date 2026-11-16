//! Actual management router against a synthetic fixed-authority peer, never real credentials.
use crate::{routing::config::RouteConfigYaml, state::AppState};
use axum::http::{Method, StatusCode};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
mod catalog;
mod fixture;
mod flow;
mod lifetime;
mod safety;
#[path = "../../../../../tests/console_contract_support/mod.rs"]
mod support;
const ROOT: &str = "/v1/internal/gateway/console/nous-auth-sessions";
fn document() -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[{"id":"nous","preset":"nous-portal-oauth",
        "auth_mode":"nous_portal_oauth","explicit_credentials_only":true,
        "base_url":"https://inference-api.nousresearch.com/v1","credentials":[]},
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
