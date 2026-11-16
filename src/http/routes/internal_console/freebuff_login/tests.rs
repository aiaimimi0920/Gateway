//! Real management router, synthetic fixed-origin peer, normal secret resolution and durable revision archive.
use super::*;
use crate::{
    console::freebuff_login::{test_support::*, Session, SessionStatus},
    http::router::build_router,
    protocol::freebuff_login::TEST_ORIGIN,
};
use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;
mod flow;
mod reauthorization;
mod safety;
#[path = "../../../../../tests/console_contract_support/mod.rs"]
mod support;
const ROOT: &str = "/v1/internal/gateway/console/freebuff-auth-sessions";
fn create_body(state: &Arc<AppState>) -> Value {
    json!({"providerId":"freebuff-pool", "expectedRevision":state.route_config.snapshot().revision().id(), "fingerprintId":FINGERPRINT})
}
async fn create(
    state: &Arc<AppState>,
    grant: &str,
    body: Value,
    mock: &Mock,
) -> (Value, Arc<Session>) {
    let response = TEST_ORIGIN
        .scope(
            mock.origin.clone(),
            support::send_console_json(state, Method::POST, ROOT, body, Some(grant), None),
        )
        .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        support::parse_json(response).await
    );
    let view = support::parse_json(response).await["session"].clone();
    let (owner, _) = authorize(
        state,
        &HeaderMap::new(),
        Some(support::MANAGEMENT_TOKEN),
        None,
        false,
    )
    .unwrap();
    let session = state
        .lifecycle
        .freebuff_login
        .get_owned(view["id"].as_str().unwrap(), &owner)
        .unwrap();
    wait(&session, SessionStatus::WaitingUser).await;
    (view, session)
}
async fn action(
    state: &Arc<AppState>,
    view: &Value,
    grant: Option<&str>,
    action: &str,
) -> Response {
    support::send_console_json(
        state,
        Method::POST,
        &format!("{ROOT}/{}/{action}", view["id"].as_str().unwrap()),
        json!({"expectedRevision":view["expectedRevision"]}),
        grant,
        None,
    )
    .await
}
async fn make_ready(
    state: &Arc<AppState>,
    view: &Value,
    session: &Arc<Session>,
    grant: &str,
    mock: &Mock,
) {
    let response = TEST_ORIGIN
        .scope(
            mock.origin.clone(),
            action(state, view, Some(grant), "poll"),
        )
        .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        support::parse_json(response).await
    );
    wait(session, SessionStatus::Ready).await;
}
