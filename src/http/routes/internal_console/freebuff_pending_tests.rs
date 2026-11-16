//! Real management authorization; no public purchase or replacement executor.
#[path = "../../../../tests/console_contract_support/mod.rs"]
mod support;
use crate::console::freebuff_purchase::{RequestBinding, WaitingRequest};
use axum::http::{Method, StatusCode};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use support::{grant_console_secret_access, parse_json, send_console_json, ConsoleStateFixture};
mod admission;
mod authority;
mod binding;
mod boundaries;
#[path = "freebuff_catalog_fixture.rs"]
mod catalog_fixture;
mod request;
mod wire;
use request::TestRequest;
const PATH: &str = "/v1/internal/gateway/console/freebuff-pending-purchases";
fn fixture() -> ConsoleStateFixture {
    ConsoleStateFixture::new(serde_json::from_value(json!({"providers":[{"id":"pool","preset":"freebuff","base_url":"https://freebuff.com",
        "explicit_credentials_only":true,"credentials":[{"id":"b","api_key":"synthetic-b","supported_models":["open","other"],
        "extra_body":{"freebuffBusiness":{"origin":"https://codebuff.com","client":"cli"}}},
        {"id":"a","api_key":"synthetic-a","supported_models":["open","other"],
        "extra_body":{"freebuffBusiness":{"origin":"https://codebuff.com","client":"cli"}}}]}]})).unwrap(),true)
}
async fn list(f: &ConsoleStateFixture, grant: &str) -> serde_json::Value {
    let reply = send_console_json(&f.state, Method::POST, PATH, body(f), Some(grant), None).await;
    assert_eq!(reply.status(), StatusCode::OK);
    assert_eq!(reply.headers()["cache-control"], "no-store");
    parse_json(reply).await["pending"].clone()
}
async fn decide(
    f: &ConsoleStateFixture,
    grant: &str,
    view: &serde_json::Value,
    action: &str,
) -> axum::response::Response {
    let mut input = body(f);
    input["pendingId"] = view["pendingId"].clone();
    input["quoteId"] = view["quoteId"].clone();
    input["decision"] = json!(action);
    send_console_json(
        &f.state,
        Method::POST,
        &format!("{PATH}/decision"),
        input,
        Some(grant),
        None,
    )
    .await
}
fn body(f: &ConsoleStateFixture) -> serde_json::Value {
    json!({"providerId":"pool","credentialId":"b","expectedRevision":f.state.route_config.snapshot().revision().id()})
}
#[tokio::test]
async fn freebuff_pending_management_read_never_creates_a_purchase() {
    let f = fixture();
    let grant = grant_console_secret_access(&f.state).await;
    let response =
        send_console_json(&f.state, Method::POST, PATH, body(&f), Some(&grant), None).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(parse_json(response).await["pending"], json!([]));
}
