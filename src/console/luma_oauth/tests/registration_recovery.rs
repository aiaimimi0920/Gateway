//! Reconstructed registries share durable state, not in-memory receipts or real upstream accounts.
use super::{
    fixture::{assert_port_free, fixture, redirect},
    registration::{peer, stop},
};
use crate::{
    console::luma_oauth::{registration_storage::persistence, RegistrationStatus, Registry},
    protocol::luma_oauth::test_support::REGISTRATION_ENDPOINT,
};
use std::{sync::atomic::Ordering, time::Duration};

async fn terminal(receipt: &super::super::registration::Registration) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while receipt.view().status == RegistrationStatus::Registering {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn luma_registration_success_is_not_published_when_receipt_write_fails() {
    use axum::{http::StatusCode, routing::post, Json, Router};
    use serde_json::{json, Value};
    let f = fixture(&redirect(), false);
    let store = persistence(&f.state).unwrap();
    let failing = store.clone();
    let app = Router::new().route(
        "/register",
        post(move |Json(body): Json<Value>| {
            failing.mark_degraded("synthetic_result_write_failure");
            async move {
                (
                    StatusCode::CREATED,
                    Json(json!({"client_id":"not-durably-saved",
            "redirect_uris":body["redirect_uris"], "grant_types":["authorization_code"],
            "response_types":["code"], "token_endpoint_auth_method":"none"})),
                )
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/register", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let revision = f.state.route_config.snapshot().revision().id().to_string();
    let registry = &f.state.lifecycle.luma_oauth;
    let receipt = registry
        .prepare_registration(&f.state, "owner", "luma-mcp", &revision)
        .await
        .unwrap();
    REGISTRATION_ENDPOINT
        .scope(endpoint, async {
            registry
                .start_registration(
                    &f.state,
                    &receipt.view().id,
                    "owner",
                    &revision,
                    rquest::Client::builder().no_proxy().build().unwrap(),
                )
                .unwrap();
            terminal(&receipt).await;
            assert!(receipt.view().status == RegistrationStatus::Unconfirmed);
            assert!(receipt.view().client_id.is_none());
            let guard = store.try_writer_lock().unwrap();
            assert!(store.load_luma_registrations_locked(&guard).unwrap()[0]
                .client_id
                .is_none());
        })
        .await;
    registry.join().await;
    stop(task).await;
}

#[tokio::test]
async fn luma_registration_restart_restores_public_client_and_owner_isolation() {
    let f = fixture(&redirect(), false);
    let revision = f.state.route_config.snapshot().revision().id().to_string();
    let registry = &f.state.lifecycle.luma_oauth;
    let receipt = registry
        .prepare_registration(&f.state, "owner", "luma-mcp", &revision)
        .await
        .unwrap();
    let original = receipt.view();
    let (endpoint, calls, task) = peer(Duration::ZERO, true).await;
    REGISTRATION_ENDPOINT
        .scope(endpoint, async {
            registry
                .start_registration(
                    &f.state,
                    &original.id,
                    "owner",
                    &revision,
                    rquest::Client::builder().no_proxy().build().unwrap(),
                )
                .unwrap();
            terminal(&receipt).await;
            assert!(receipt.view().status == RegistrationStatus::Registered);
            registry.join().await;
            let restarted = Registry::default();
            assert!(restarted
                .load_registration(&f.state, &original.id, "other")
                .await
                .is_err());
            let restored = restarted
                .load_registration(&f.state, &original.id, "owner")
                .await
                .unwrap()
                .view();
            assert_eq!(restored.id, original.id);
            assert_eq!(restored.redirect_uri, original.redirect_uri);
            assert_eq!(restored.client_id.as_deref(), Some("new-synthetic-client"));
            assert!(restored.status == RegistrationStatus::Registered);
            assert_port_free(&restored.redirect_uri);
            let prepared = restarted
                .prepare_registration(&f.state, "owner", "luma-mcp", &revision)
                .await
                .unwrap();
            assert_eq!(prepared.view().id, original.id);
            restarted
                .start_registration(
                    &f.state,
                    &original.id,
                    "owner",
                    &revision,
                    rquest::Client::builder().no_proxy().build().unwrap(),
                )
                .unwrap();
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            restarted.join().await;
        })
        .await;
    stop(task).await;
}

#[tokio::test]
async fn luma_registration_durable_marker_precedes_network_and_survives_shutdown() {
    let f = fixture(&redirect(), false);
    let revision = f.state.route_config.snapshot().revision().id().to_string();
    let registry = &f.state.lifecycle.luma_oauth;
    let receipt = registry
        .prepare_registration(&f.state, "owner", "luma-mcp", &revision)
        .await
        .unwrap();
    let (endpoint, calls, task) = peer(Duration::from_secs(60), true).await;
    REGISTRATION_ENDPOINT
        .scope(endpoint, async {
            registry
                .start_registration(
                    &f.state,
                    &receipt.view().id,
                    "owner",
                    &revision,
                    rquest::Client::builder().no_proxy().build().unwrap(),
                )
                .unwrap();
            tokio::time::timeout(Duration::from_secs(5), async {
                while calls.load(Ordering::SeqCst) == 0 {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            let store = persistence(&f.state).unwrap();
            {
                let guard = store.try_writer_lock().unwrap();
                let records = store.load_luma_registrations_locked(&guard).unwrap();
                assert_eq!(records.len(), 1);
                assert_eq!(records[0].id, receipt.view().id);
                assert!(records[0].client_id.is_none());
            }
            registry.join().await;
            let restarted = Registry::default();
            let restored = restarted
                .prepare_registration(&f.state, "owner", "luma-mcp", &revision)
                .await
                .unwrap();
            assert!(restored.view().status == RegistrationStatus::Unconfirmed);
            assert_eq!(restored.view().id, receipt.view().id);
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            restarted.join().await;
        })
        .await;
    stop(task).await;
}

#[tokio::test]
async fn luma_registration_storage_failure_prevents_remote_request() {
    let f = fixture(&redirect(), false);
    let revision = f.state.route_config.snapshot().revision().id().to_string();
    let registry = &f.state.lifecycle.luma_oauth;
    let receipt = registry
        .prepare_registration(&f.state, "owner", "luma-mcp", &revision)
        .await
        .unwrap();
    persistence(&f.state)
        .unwrap()
        .mark_degraded("synthetic_storage_failure");
    let (endpoint, calls, task) = peer(Duration::ZERO, true).await;
    REGISTRATION_ENDPOINT
        .scope(endpoint, async {
            registry
                .start_registration(
                    &f.state,
                    &receipt.view().id,
                    "owner",
                    &revision,
                    rquest::Client::builder().no_proxy().build().unwrap(),
                )
                .unwrap();
            terminal(&receipt).await;
            assert!(receipt.view().status == RegistrationStatus::Unconfirmed);
            assert_eq!(calls.load(Ordering::SeqCst), 0);
        })
        .await;
    registry.join().await;
    stop(task).await;
}

#[tokio::test]
async fn luma_registration_two_runtime_candidates_send_only_one_request() {
    let f = fixture(&redirect(), false);
    let revision = f.state.route_config.snapshot().revision().id().to_string();
    let first = Registry::default();
    let second = Registry::default();
    let a = first
        .prepare_registration(&f.state, "owner", "luma-mcp", &revision)
        .await
        .unwrap();
    let b = second
        .prepare_registration(&f.state, "owner", "luma-mcp", &revision)
        .await
        .unwrap();
    assert_ne!(a.view().id, b.view().id);
    let (endpoint, calls, task) = peer(Duration::from_millis(40), true).await;
    REGISTRATION_ENDPOINT
        .scope(endpoint, async {
            let http = rquest::Client::builder().no_proxy().build().unwrap();
            first
                .start_registration(&f.state, &a.view().id, "owner", &revision, http.clone())
                .unwrap();
            second
                .start_registration(&f.state, &b.view().id, "owner", &revision, http)
                .unwrap();
            terminal(&a).await;
            terminal(&b).await;
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            let recovered_a = first
                .prepare_registration(&f.state, "owner", "luma-mcp", &revision)
                .await
                .unwrap();
            let recovered_b = second
                .prepare_registration(&f.state, "owner", "luma-mcp", &revision)
                .await
                .unwrap();
            assert_eq!(recovered_a.view().id, recovered_b.view().id);
        })
        .await;
    first.join().await;
    second.join().await;
    stop(task).await;
}
