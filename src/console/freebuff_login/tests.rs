//! Cancellation, TTL, dropped save ownership and commit uncertainty do not invent successful persistence.
use super::{test_support::*, *};
use crate::protocol::freebuff_login::TEST_ORIGIN;
use std::{sync::Arc, time::Duration};
#[path = "../../../tests/console_contract_support/mod.rs"]
mod support;

#[tokio::test]
async fn unpolled_freebuff_save_and_unpolled_future_close_admission_without_retry() {
    for make_future in [false, true] {
        let fixture = support::ConsoleStateFixture::new(document(), true);
        let session = ready(&fixture.state).await;
        let operation = session
            .begin_save(&fixture.state, session.binding())
            .unwrap()
            .unwrap();
        let target = session.view().target_credential_id.unwrap();
        if make_future {
            drop(operation.run(fixture.state.clone(), session.clone()));
        } else {
            drop(operation);
        }
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
        fixture.state.lifecycle.freebuff_login.join().await;
    }
}
#[tokio::test]
async fn freebuff_save_timeout_includes_publication_lock_and_never_reports_success() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let session = ready(&fixture.state).await;
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
    tokio::time::pause();
    operation.run(fixture.state.clone(), session.clone()).await;
    assert_eq!(session.view().status, SessionStatus::SaveUnconfirmed);
    assert_eq!(
        session.view().error_code.as_deref(),
        Some("freebuff_login_save_timeout")
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
    drop(lock);
    fixture.state.lifecycle.freebuff_login.join().await;
}
#[tokio::test]
async fn freebuff_cancel_during_body_read_is_joined_and_destroys_approval_material() {
    let mut spec = code();
    spec.body_delay = Duration::from_secs(30);
    let mock = Mock::new(vec![spec]).await;
    let registry = Arc::new(Registry::default());
    let session = TEST_ORIGIN
        .scope(mock.origin.clone(), async {
            registry
                .start(
                    Binding::new("owner", "pool", "revision").unwrap(),
                    http(),
                    FINGERPRINT.into(),
                )
                .unwrap()
        })
        .await;
    tokio::time::timeout(Duration::from_secs(3), async {
        while mock.count() != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    session.cancel();
    registry.join().await;
    assert_eq!(session.view().status, SessionStatus::Cancelled);
    assert!(session.view().login_url.is_none());
    assert!(session.inner.lock().material.is_none());
    assert!(registry
        .start(
            Binding::new("owner", "pool", "revision").unwrap(),
            http(),
            FINGERPRINT.into()
        )
        .is_err());
    mock.stop().await;
}
#[tokio::test]
async fn freebuff_expiry_and_registry_drop_clear_owned_material_without_an_external_owner() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let session = ready(&fixture.state).await;
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(601)).await;
    assert_eq!(session.view().status, SessionStatus::Expired);
    assert!(session.inner.lock().material.is_none());
    fixture.state.lifecycle.freebuff_login.join().await;
    let binding = Binding::new("owner", "pool", "revision").unwrap();
    let short = Session::new(binding, FINGERPRINT.into(), Duration::ZERO);
    assert_eq!(short.view().status, SessionStatus::Expired);
}
#[tokio::test]
async fn freebuff_registry_owner_revision_and_capacity_are_checked_before_another_send() {
    let mock = Mock::new((0..16).map(|_| code()).collect()).await;
    let registry = Registry::default();
    let binding = Binding::new("owner", "pool", "revision").unwrap();
    let mut sessions = Vec::new();
    for _ in 0..16 {
        sessions.push(
            TEST_ORIGIN
                .scope(mock.origin.clone(), async {
                    registry
                        .start(binding.clone(), http(), FINGERPRINT.into())
                        .unwrap()
                })
                .await,
        );
    }
    for session in &sessions {
        wait(session, SessionStatus::WaitingUser).await;
    }
    assert!(matches!(
        registry.start(binding.clone(), http(), FINGERPRINT.into()),
        Err(LoginError::Capacity)
    ));
    assert!(registry.get_owned(sessions[0].id(), "other-owner").is_err());
    let wrong = Binding::new("owner", "pool", "another-revision").unwrap();
    assert!(registry
        .start_poll(sessions[0].id(), &wrong, http())
        .is_err());
    registry.join().await;
    assert_eq!(mock.count(), 16);
    mock.stop().await;
}
