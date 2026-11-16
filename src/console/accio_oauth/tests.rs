//! Session resource limits and cancellation do not require real credentials or an enabled wire leaf.
use super::*;
use crate::protocol::accio_oauth::{LoginSite, OAuthClient};
use std::time::Duration;

fn binding(owner: &str) -> Binding {
    Binding::new(owner, "saved-pool", "revision").unwrap()
}
fn create(registry: &Registry, owner: &str) -> std::sync::Arc<Session> {
    registry
        .create(
            binding(owner),
            OAuthClient::new("synthetic-client").unwrap(),
            LoginSite::China,
            34567,
            "synthetic-device".into(),
        )
        .unwrap()
}
#[tokio::test]
async fn owner_capacity_cancel_shutdown_and_instance_isolation() {
    let registry = Registry::default();
    let first = create(&registry, "owner");
    assert!(registry.get_owned(first.id(), "foreign").is_err());
    assert!(Registry::default().get_owned(first.id(), "owner").is_err());
    for _ in 1..16 {
        create(&registry, "owner");
    }
    assert!(matches!(
        registry.create(
            binding("owner"),
            OAuthClient::new("synthetic-client").unwrap(),
            LoginSite::Global,
            34567,
            "synthetic-device".into()
        ),
        Err(LoginError::Capacity)
    ));
    first.cancel();
    assert_eq!(first.view().status, SessionStatus::Cancelled);
    assert!(first.view().authorization_url.is_none());
    create(&registry, "owner");
    registry.join().await;
    assert!(matches!(
        registry.create(
            binding("owner"),
            OAuthClient::new("synthetic-client").unwrap(),
            LoginSite::China,
            34567,
            "synthetic-device".into()
        ),
        Err(LoginError::Stopped)
    ));
}
#[tokio::test]
async fn expired_session_is_pruned_without_sending_and_does_not_retain_device_or_pkce_view() {
    let registry = Registry::default();
    let session = registry
        .create_with_ttl(
            binding("owner"),
            OAuthClient::new("synthetic-client").unwrap(),
            LoginSite::China,
            34567,
            "synthetic-device".into(),
            Duration::ZERO,
        )
        .unwrap();
    assert_eq!(session.view().status, SessionStatus::Expired);
    assert!(session.view().authorization_url.is_none());
    assert!(registry.get_owned(session.id(), "owner").is_err());
    assert!(session
        .complete(&rquest::Client::new(), "not-a-callback")
        .await
        .is_err());
    registry.join().await;
}
#[tokio::test]
async fn invalid_callback_consumes_attempt_and_never_echoes_callback_in_error_or_debug() {
    let registry = Registry::default();
    let session = create(&registry, "synthetic-secret-owner");
    let error = session
        .complete(
            &rquest::Client::new(),
            "https://foreign.invalid/?code=synthetic-private-code",
        )
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "accio_oauth_invalid_callback");
    assert_eq!(session.view().status, SessionStatus::Failed);
    assert!(session.view().authorization_url.is_none());
    assert!(session
        .complete(&rquest::Client::new(), "synthetic-private-code")
        .await
        .is_err());
    for text in [
        format!("{session:?}"),
        format!("{registry:?}"),
        format!("{:?}", session.binding()),
    ] {
        assert!(!text.contains("synthetic-secret-owner"));
        assert!(!text.contains("synthetic-device"));
    }
    registry.join().await;
}
#[test]
fn explicit_device_and_identifiers_reject_unsafe_or_missing_input() {
    assert!(Binding::new("", "pool", "rev").is_err());
    assert!(binding("owner").with_credential("\n").is_err());
    for device in ["", " whitespace ", "bad\r\ndevice", "设备"] {
        assert!(Registry::default()
            .create(
                binding("owner"),
                OAuthClient::new("synthetic-client").unwrap(),
                LoginSite::China,
                34567,
                device.into()
            )
            .is_err());
    }
}

#[tokio::test]
async fn runtime_owned_expiry_worker_observes_shutdown_and_is_joined() {
    let registry = std::sync::Arc::new(Registry::default());
    let shutdown = crate::state::GatewayShutdownHandle::default();
    let session = create(&registry, "owner");
    registry.spawn(shutdown.clone());
    registry.spawn(shutdown.clone());
    shutdown.request("synthetic-test-shutdown");
    tokio::time::timeout(Duration::from_secs(2), registry.join())
        .await
        .unwrap();
    assert_eq!(session.view().status, SessionStatus::Cancelled);
    assert!(registry.get_owned(session.id(), "owner").is_err());
}
