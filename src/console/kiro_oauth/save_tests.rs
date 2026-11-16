//! Save guard is armed before first poll; the 20s publication budget never renews expired material.
use super::test_support::*;
use super::*;
use crate::protocol::kiro_oauth::{Directory, Scope, TEST_ORIGIN};
use std::{sync::Arc, time::Duration};
#[path = "../../../tests/console_contract_support/mod.rs"]
mod support;
async fn ready() -> (support::ConsoleStateFixture, Arc<Session>) {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let binding = Binding::new(
        "synthetic-owner",
        "kiro-pool",
        state.route_config.snapshot().revision().id(),
    )
    .unwrap();
    let mock = Mock::new(vec![registration(), device(), tokens(true, true)]).await;
    let session = TEST_ORIGIN
        .scope(mock.origin.clone(), async {
            state
                .lifecycle
                .kiro_oauth
                .start(
                    binding,
                    http(),
                    Directory::builder_id(),
                    vec![Scope::Conversations],
                    "a".repeat(64),
                )
                .unwrap()
        })
        .await;
    wait(&session, SessionStatus::WaitingUser).await;
    TEST_ORIGIN
        .scope(mock.origin.clone(), async {
            state
                .lifecycle
                .kiro_oauth
                .start_poll(session.id(), session.binding(), http())
                .unwrap();
        })
        .await;
    wait(&session, SessionStatus::Ready).await;
    mock.stop().await;
    (fixture, session)
}
#[tokio::test]
async fn unpolled_save_drop_closes_material_and_preserves_unconfirmed_target() {
    let (fixture, session) = ready().await;
    let operation = session
        .begin_save(&fixture.state, session.binding())
        .unwrap()
        .unwrap();
    let target = session.view().target_credential_id.unwrap();
    drop(operation.run(fixture.state.clone(), session.clone()));
    assert_eq!(session.view().status, SessionStatus::SaveUnconfirmed);
    assert_eq!(
        session.view().target_credential_id.as_deref(),
        Some(target.as_str())
    );
    assert!(session
        .begin_save(&fixture.state, session.binding())
        .unwrap()
        .is_none());
    assert!(
        fixture.state.route_config.snapshot().document().providers[0]
            .credentials
            .is_empty()
    );
    fixture.state.lifecycle.kiro_oauth.join().await;
}
#[tokio::test]
async fn save_timeout_covers_publication_wait_and_does_not_retry_or_claim_success() {
    let (fixture, session) = ready().await;
    let lock = fixture
        .state
        .lifecycle
        .health_control
        .publication
        .lock()
        .await;
    let operation = session
        .begin_save(&fixture.state, session.binding())
        .unwrap()
        .unwrap();
    tokio::time::timeout(
        Duration::from_secs(25),
        operation.run(fixture.state.clone(), session.clone()),
    )
    .await
    .unwrap();
    assert_eq!(session.view().status, SessionStatus::SaveUnconfirmed);
    assert_eq!(
        session.view().error_code.as_deref(),
        Some("kiro_login_save_timeout")
    );
    assert!(session
        .begin_save(&fixture.state, session.binding())
        .unwrap()
        .is_none());
    drop(lock);
    assert!(
        fixture.state.route_config.snapshot().document().providers[0]
            .credentials
            .is_empty()
    );
    fixture.state.lifecycle.kiro_oauth.join().await;
}
