//! Bounded ownership, cancellation and unpolled-operation cleanup independent of HTTP lifetime.
use super::test_support::*;
use super::*;
use crate::protocol::kiro_oauth::{Directory, Scope, TEST_ORIGIN};
use std::{sync::Arc, time::Duration};
fn binding() -> Binding {
    Binding::new("synthetic-owner", "pool", "r-test").unwrap()
}
fn start(registry: &Registry) -> Arc<Session> {
    registry
        .start(
            binding(),
            http(),
            Directory::builder_id(),
            vec![Scope::Conversations],
            "a".repeat(64),
        )
        .unwrap()
}

#[tokio::test]
async fn capacity_and_shutdown_admit_no_untracked_network_operations() {
    let mock = Mock::new(vec![]).await;
    let registry = Registry::default();
    TEST_ORIGIN
        .scope(mock.origin.clone(), async {
            for _ in 0..16 {
                start(&registry);
            }
            assert!(matches!(
                registry.start(
                    binding(),
                    http(),
                    Directory::builder_id(),
                    vec![Scope::Conversations],
                    "a".repeat(64)
                ),
                Err(LoginError::Capacity)
            ));
            registry.join().await;
            assert!(matches!(
                registry.start(
                    binding(),
                    http(),
                    Directory::builder_id(),
                    vec![Scope::Conversations],
                    "a".repeat(64)
                ),
                Err(LoginError::Stopped)
            ));
        })
        .await;
    assert_eq!(mock.count(), 0);
    mock.stop().await;
}

#[tokio::test]
async fn cancellation_during_registration_cannot_publish_a_late_device_or_material() {
    let mut delayed = registration();
    delayed.header_delay = Duration::from_millis(100);
    let mock = Mock::new(vec![delayed]).await;
    let registry = Registry::default();
    let session = TEST_ORIGIN
        .scope(mock.origin.clone(), async { start(&registry) })
        .await;
    mock.wait_count(1).await;
    session.cancel();
    registry.join().await;
    assert_eq!(session.view().status, SessionStatus::Cancelled);
    assert!(session.inner.lock().context.is_none());
    assert!(session.view().verification_uri.is_none());
    assert_eq!(mock.count(), 1);
    mock.stop().await;
}

#[tokio::test]
async fn unpolled_registration_and_poll_drop_fail_closed_without_reusing_device() {
    let session = Session::new(binding(), "a".repeat(64), Duration::from_secs(600));
    drop(session.initialize(http(), Directory::builder_id(), vec![Scope::Conversations]));
    assert_eq!(session.view().status, SessionStatus::Failed);
    let mock = Mock::new(vec![registration(), device()]).await;
    let registry = Registry::default();
    let session = TEST_ORIGIN
        .scope(mock.origin.clone(), async { start(&registry) })
        .await;
    wait(&session, SessionStatus::WaitingUser).await;
    drop(session.begin_poll(http()).unwrap());
    assert_eq!(session.view().status, SessionStatus::Failed);
    assert!(session.begin_poll(http()).is_err());
    assert!(session.inner.lock().context.is_none());
    registry.join().await;
    assert_eq!(mock.count(), 2);
    mock.stop().await;
}

#[tokio::test]
async fn cancellation_during_sent_poll_does_not_allow_replay_or_resurrection() {
    let mut token = tokens(true, true);
    token.body_delay = Duration::from_millis(100);
    let mock = Mock::new(vec![registration(), device(), token]).await;
    let registry = Registry::default();
    let session = TEST_ORIGIN
        .scope(mock.origin.clone(), async { start(&registry) })
        .await;
    wait(&session, SessionStatus::WaitingUser).await;
    TEST_ORIGIN
        .scope(mock.origin.clone(), async {
            registry
                .start_poll(session.id(), session.binding(), http())
                .unwrap();
        })
        .await;
    mock.wait_count(3).await;
    session.cancel();
    registry.join().await;
    assert_eq!(session.view().status, SessionStatus::Cancelled);
    assert!(session.inner.lock().material.is_none());
    assert!(session.inner.lock().context.is_none());
    assert!(session.begin_poll(http()).is_err());
    assert_eq!(mock.count(), 3);
    mock.stop().await;
}

#[tokio::test]
async fn ttl_and_runtime_shutdown_close_capabilities_and_join_expiry_owner() {
    let session = Session::new(binding(), "a".repeat(64), Duration::from_millis(1));
    tokio::time::sleep(Duration::from_millis(5)).await;
    assert_eq!(session.view().status, SessionStatus::Expired);
    assert!(!session.retain());
    let registry = Arc::new(Registry::default());
    let shutdown = crate::state::GatewayShutdownHandle::default();
    registry.spawn(shutdown.clone());
    let mock = Mock::new(vec![]).await;
    let session = TEST_ORIGIN
        .scope(mock.origin.clone(), async { start(&registry) })
        .await;
    shutdown.request("synthetic-test");
    registry.join().await;
    assert_eq!(session.view().status, SessionStatus::Cancelled);
    assert!(registry.get_owned(session.id(), "synthetic-owner").is_err());
    assert_eq!(mock.count(), 0);
    mock.stop().await;
}

#[tokio::test]
async fn terminal_poll_errors_clear_registered_material_and_cannot_be_replayed() {
    for (wire, expected) in [
        (
            serde_json::json!({"error":"access_denied","detail":"must-not-leak"}),
            "kiro_oauth_access_denied",
        ),
        (
            serde_json::json!({"error":"expired_token"}),
            "kiro_oauth_expired",
        ),
        (
            serde_json::json!({"error":"invalid_client"}),
            "kiro_oauth_reauthorization_required",
        ),
    ] {
        let mock = Mock::new(vec![
            registration(),
            device(),
            ResponseSpec::json(400, wire),
        ])
        .await;
        let registry = Registry::default();
        let session = TEST_ORIGIN
            .scope(mock.origin.clone(), async { start(&registry) })
            .await;
        wait(&session, SessionStatus::WaitingUser).await;
        TEST_ORIGIN
            .scope(mock.origin.clone(), async {
                registry
                    .start_poll(session.id(), session.binding(), http())
                    .unwrap();
            })
            .await;
        wait(&session, SessionStatus::Failed).await;
        assert_eq!(session.view().error_code.as_deref(), Some(expected));
        assert!(!serde_json::to_string(&session.view())
            .unwrap()
            .contains("must-not-leak"));
        assert!(session.inner.lock().context.is_none());
        assert!(session.inner.lock().material.is_none());
        assert!(registry
            .start_poll(session.id(), session.binding(), http())
            .is_err());
        registry.join().await;
        assert_eq!(mock.count(), 3);
        mock.stop().await;
    }
}
