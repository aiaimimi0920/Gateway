//! Production management router, real Local SQLite and synthetic signed upstream only.
use super::*;
use axum::http::{Method, StatusCode};
use serde_json::json;
use std::path::PathBuf;
#[path = "../../../../../tests/console_contract_support/mod.rs"]
mod support;
#[path = "../freebuff_catalog_fixture.rs"]
mod wire;
use support::{grant_console_secret_access, parse_json, send_console_json, ConsoleStateFixture};
mod browser;
mod finish;
mod history;
mod preflight;
mod tcp;
const ENDPOINT: &str = "/v1/internal/gateway/console/freebuff-session-claim";
async fn fixture() -> (PathBuf, ConsoleStateFixture) {
    let root = std::env::temp_dir().join(format!(
        "gateway-local-freebuff-managed-end-{}",
        uuid::Uuid::new_v4()
    ));
    let local = LocalRuntime::open(&root).await.unwrap();
    let mut f = state_fixture();
    Arc::get_mut(&mut f.state).unwrap().local_runtime = Some(local);
    (root, f)
}
fn state_fixture() -> ConsoleStateFixture {
    ConsoleStateFixture::new(serde_json::from_value(json!({"providers":[{"id":"pool","preset":"freebuff","base_url":"https://freebuff.com",
        "explicit_credentials_only":true,"credentials":[{"id":"a","api_key":"synthetic-a"},{"id":"b","api_key":"synthetic-b","enabled":false,
        "supported_models":["open"],"extra_body":{"freebuffBusiness":{"origin":"https://codebuff.com","client":"cli"}}}]}]})).unwrap(), true)
}
async fn cleanup(root: &std::path::Path, f: ConsoleStateFixture) {
    f.state.local_runtime.as_ref().unwrap().close().await;
    drop(f);
    crate::local_runtime::test_support::remove_test_root(root).await;
}
fn body(f: &ConsoleStateFixture) -> Value {
    json!({"providerId":"pool","credentialId":"b","expectedRevision":f.state.route_config.snapshot().revision().id(),"cursor":null})
}
fn end_body(f: &ConsoleStateFixture, attempt: &str) -> Value {
    let mut input = body(f);
    input.as_object_mut().unwrap().remove("cursor");
    input["attemptId"] = json!(attempt);
    input
}
async fn call(
    f: &ConsoleStateFixture,
    peer: &wire::Peer,
    path: &str,
    body: Value,
    grant: &str,
) -> Response {
    crate::protocol::freebuff::catalog::with_test_origin(
        peer.origin.clone(),
        send_console_json(&f.state, Method::POST, path, body, Some(grant), None),
    )
    .await
}
async fn seed(f: &ConsoleStateFixture, peer: &wire::Peer, grant: &str) -> (String, String) {
    let mut input = body(f);
    input.as_object_mut().unwrap().remove("cursor");
    input["businessOrigin"] = json!("https://codebuff.com");
    input["client"] = json!("cli");
    let response = call(
        f,
        peer,
        "/v1/internal/gateway/console/freebuff-device-registration",
        input,
        grant,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let public = peer
        .hits
        .lock()
        .unwrap()
        .iter()
        .find(|hit| hit.0 == "POST")
        .map(|hit| {
            serde_json::from_slice::<Value>(&hit.3).unwrap()["publicKey"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .unwrap_or_default();
    let local = f.state.local_runtime.as_ref().unwrap();
    let mut session =
        CatalogSession::new(BusinessOrigin::Codebuff, "synthetic-b", ClientSurface::Cli).unwrap();
    let device = local
        .freebuff_registration("pool", "b", &session)
        .await
        .unwrap()
        .unwrap();
    crate::protocol::freebuff::catalog::with_test_origin(
        peer.origin.clone(),
        session.refresh(Some(&device), std::time::Duration::from_secs(3)),
    )
    .await
    .unwrap();
    let requests = session.business_requests(&device, "open").unwrap();
    let crate::local_runtime::freebuff_sessions::AdmissionPreparation::Send(pending) = local
        .prepare_freebuff_admission(
            "pool",
            "b",
            &requests,
            crate::protocol::freebuff::catalog::SessionChargeConsent::MaySpendFreebucks,
        )
        .await
        .unwrap()
    else {
        panic!("test seeds one original pending claim");
    };
    let attempt = pending.claim().attempt_id().to_owned();
    drop(pending);
    peer.hits.lock().unwrap().clear();
    (attempt, public)
}
async fn counts(f: &ConsoleStateFixture) -> (i64, i64) {
    let pool = &f.state.local_runtime.as_ref().unwrap().pool;
    let active = sqlx::query_scalar("SELECT count(*) FROM local_freebuff_session_claims")
        .fetch_one(pool)
        .await
        .unwrap();
    let history = sqlx::query_scalar("SELECT count(*) FROM local_freebuff_session_closeouts")
        .fetch_one(pool)
        .await
        .unwrap();
    (active, history)
}
