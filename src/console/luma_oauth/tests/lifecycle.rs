use super::fixture::*;
use crate::{console::luma_oauth::SessionStatus, protocol::luma_oauth::test_support::ENDPOINT};
use std::time::Duration;

fn http() -> rquest::Client {
    rquest::Client::builder().no_proxy().build().unwrap()
}

#[tokio::test]
async fn luma_login_owner_expiry_cancellation_and_shutdown_close_listeners() {
    let redirect = redirect();
    let f = fixture(&redirect, false);
    let registry = &f.state.lifecycle.luma_oauth;
    registry.spawn(f.state.shutdown.clone());
    let session = registry
        .create_with_ttl(
            &f.state,
            binding(&f, &redirect),
            http(),
            Duration::from_millis(50),
        )
        .await
        .unwrap();
    assert!(registry.get_owned(session.id(), "different-owner").is_err());
    await_status(&session, SessionStatus::Expired).await;
    assert_eq!(
        registry
            .get_owned(session.id(), "owner")
            .unwrap()
            .view()
            .status,
        SessionStatus::Expired
    );
    registry.join().await;
    assert_port_free(&redirect);
    assert!(registry
        .create(&f.state, binding(&f, &redirect), http())
        .await
        .is_err());
    let f = fixture(&redirect, false);
    let registry = &f.state.lifecycle.luma_oauth;
    let session = registry
        .create(&f.state, binding(&f, &redirect), http())
        .await
        .unwrap();
    session.cancel();
    assert_eq!(session.view().status, SessionStatus::Cancelled);
    registry.join().await;
    assert_port_free(&redirect);
}

#[tokio::test]
async fn luma_login_aborted_exchange_is_terminal_and_cannot_replay_code() {
    let redirect = redirect();
    let f = fixture(&redirect, false);
    let peer = Peer::new(Duration::from_secs(2)).await;
    let session = f
        .state
        .lifecycle
        .luma_oauth
        .create(&f.state, binding(&f, &redirect), http())
        .await
        .unwrap();
    let callback = callback(&session, None);
    let exchanged = session.clone();
    let endpoint = peer.endpoint.clone();
    let task = tokio::spawn(async move {
        ENDPOINT
            .scope(endpoint, exchanged.complete(&http(), &callback))
            .await
    });
    peer.wait_call().await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(session.view().status, SessionStatus::Failed);
    assert_eq!(
        session.view().error_code,
        Some("luma_login_exchange_unconfirmed")
    );
    assert!(session
        .complete(&http(), "unusable callback")
        .await
        .is_err());
    assert_eq!(peer.count(), 1);
    f.state.lifecycle.luma_oauth.join().await;
    peer.stop().await;
    assert_port_free(&redirect);
}

#[tokio::test]
async fn luma_login_unpolled_save_is_unconfirmed_and_not_replayed() {
    let redirect = redirect();
    let f = fixture(&redirect, false);
    let peer = Peer::new(Duration::ZERO).await;
    let registry = &f.state.lifecycle.luma_oauth;
    let b = binding(&f, &redirect);
    let session = registry.create(&f.state, b.clone(), http()).await.unwrap();
    ENDPOINT
        .scope(
            peer.endpoint.clone(),
            session.complete(&http(), &callback(&session, None)),
        )
        .await
        .unwrap();
    let operation = session.begin_save(&f.state, &b).unwrap().unwrap();
    let future = operation.run(f.state.clone(), session.clone());
    assert_eq!(session.view().status, SessionStatus::Importing);
    drop(future);
    assert_eq!(session.view().status, SessionStatus::SaveUnconfirmed);
    assert_eq!(
        session.view().error_code,
        Some("luma_login_save_unconfirmed")
    );
    assert!(session.begin_save(&f.state, &b).unwrap().is_none());
    assert!(f.state.route_config.snapshot().document().providers[0]
        .credentials
        .is_empty());
    registry.join().await;
    peer.stop().await;
    assert_port_free(&redirect);
}

#[tokio::test]
async fn luma_login_save_waiting_on_publication_does_not_extend_authorization() {
    let redirect = redirect();
    let f = fixture(&redirect, false);
    let peer = Peer::new(Duration::ZERO).await;
    let registry = &f.state.lifecycle.luma_oauth;
    let b = binding(&f, &redirect);
    let session = registry
        .create_with_ttl(&f.state, b.clone(), http(), Duration::from_secs(1))
        .await
        .unwrap();
    ENDPOINT
        .scope(
            peer.endpoint.clone(),
            session.complete(&http(), &callback(&session, None)),
        )
        .await
        .unwrap();
    let publication = f.state.lifecycle.health_control.publication.lock().await;
    registry
        .start_import(f.state.clone(), session.id(), &b)
        .unwrap();
    tokio::time::sleep(Duration::from_millis(1100)).await;
    drop(publication);
    await_status(&session, SessionStatus::SaveUnconfirmed).await;
    assert_eq!(
        session.view().error_code,
        Some("luma_login_material_expired")
    );
    assert!(f.state.route_config.snapshot().document().providers[0]
        .credentials
        .is_empty());
    registry.join().await;
    peer.stop().await;
    assert_port_free(&redirect);
}

#[tokio::test]
async fn luma_login_capacity_and_occupied_registered_port_are_bounded() {
    let redirect = redirect();
    let f = fixture(&redirect, false);
    let registry = &f.state.lifecycle.luma_oauth;
    let port = url::Url::parse(&redirect).unwrap().port().unwrap();
    let occupied = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).unwrap();
    assert!(registry
        .create(&f.state, binding(&f, &redirect), http())
        .await
        .is_err());
    drop(occupied);
    let mut redirects = Vec::new();
    for _ in 0..16 {
        let redirect = super::fixture::redirect();
        registry
            .create(&f.state, binding(&f, &redirect), http())
            .await
            .unwrap();
        redirects.push(redirect);
    }
    assert!(registry
        .create(&f.state, binding(&f, &redirect), http())
        .await
        .is_err());
    registry.join().await;
    for redirect in redirects {
        assert_port_free(&redirect);
    }
}
