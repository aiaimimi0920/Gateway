//! A save future owns its guard before its first poll, including an immediate runtime abort.
use super::*;
use crate::routing::config::RouteConfigYaml;
use serde_json::json;
use std::sync::Arc;
#[path = "../../../tests/console_contract_support/mod.rs"]
mod support;

async fn ready() -> (support::ConsoleStateFixture, Arc<Session>) {
    let document:RouteConfigYaml=serde_json::from_value(json!({"providers":[{"id":"build","preset":"grok-oauth",
        "base_url":"https://cli-chat-proxy.grok.com/v1","explicit_credentials_only":true,"credentials":[]}]})).unwrap();
    let fixture = support::ConsoleStateFixture::new(document, true);
    let mut server = tests::Server::new(false).await;
    let binding = Binding::new(
        "synthetic-owner",
        "build",
        fixture.state.route_config.snapshot().revision().id(),
    )
    .unwrap();
    let session = fixture
        .state
        .lifecycle
        .grok_oauth
        .create(
            binding,
            server.client.clone(),
            "http://127.0.0.1:34567/callback",
            None,
        )
        .unwrap();
    session
        .complete(&tests::http(), &tests::callback(&session))
        .await
        .unwrap();
    assert_eq!(server.finish().await, 1);
    (fixture, session)
}
#[tokio::test]
async fn dropping_unpolled_save_future_is_unconfirmed_and_never_claims_success() {
    let (fixture, session) = ready().await;
    let operation = session
        .begin_save(&fixture.state, session.binding())
        .unwrap()
        .unwrap();
    let future = operation.run(fixture.state.clone(), session.clone());
    assert_eq!(session.view().status, SessionStatus::Importing);
    assert!(session.view().target_credential_id.is_some());
    drop(future);
    let view = session.view();
    assert_eq!(view.status, SessionStatus::SaveUnconfirmed);
    assert!(view.credential_id.is_none());
    assert!(view.saved_revision.is_none());
    assert!(session
        .begin_save(&fixture.state, session.binding())
        .unwrap()
        .is_none());
    assert!(
        fixture.state.route_config.snapshot().document().providers[0]
            .credentials
            .is_empty()
    );
    fixture.state.lifecycle.grok_oauth.join().await;
}
#[tokio::test]
async fn preparation_preserves_receipt_absolute_expiry_and_does_not_consume_material() {
    let (fixture, session) = ready().await;
    let inner = session.inner.lock();
    let receipt = inner.receipt.as_ref().unwrap();
    let expiry = receipt.expires_at_unix_secs().unwrap();
    let prepared = super::persistence::prepare(&fixture.state, session.binding(), receipt).unwrap();
    let actual = time::OffsetDateTime::parse(
        prepared.document.providers[0].credentials[0]
            .expires_at
            .as_ref()
            .unwrap(),
        &time::format_description::well_known::Rfc3339,
    )
    .unwrap()
    .unix_timestamp();
    assert_eq!(actual, expiry as i64);
    drop(inner);
    assert_eq!(session.view().status, SessionStatus::Ready);
    assert!(session.claim_receipt(session.binding()).is_ok());
    fixture.state.lifecycle.grok_oauth.join().await;
}
