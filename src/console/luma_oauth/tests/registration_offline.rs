//! Refresh-capable clients are independently confirmed; old receipts remain code-only on reopen.
use super::{
    fixture::{fixture, redirect},
    registration::stop,
};
use crate::{
    console::luma_oauth::{Binding, RegistrationStatus, Registry},
    protocol::luma_oauth::test_support::REGISTRATION_ENDPOINT,
};
use axum::{http::StatusCode, routing::post, Json, Router};
use serde_json::{json, Value};
use std::time::Duration;

#[tokio::test]
async fn luma_registration_offline_persists_capability_and_allows_normal_authorization() {
    let f = fixture(&redirect(), false);
    let revision = f.state.route_config.snapshot().revision().id().to_string();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/register", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, Router::new().route("/register", post(|Json(body): Json<Value>| async move {
            assert_eq!(body["grant_types"], json!(["authorization_code", "refresh_token"]));
            (StatusCode::CREATED, Json(json!({"client_id":"offline-client","redirect_uris":body["redirect_uris"],
                "grant_types":body["grant_types"],"response_types":["code"],"token_endpoint_auth_method":"none"})))
        }))).await.unwrap();
    });
    let registry = &f.state.lifecycle.luma_oauth;
    let prepared = registry
        .prepare_registration_with_offline(&f.state, "owner", "luma-mcp", &revision, true)
        .await
        .unwrap();
    assert!(prepared.view().offline_access);
    REGISTRATION_ENDPOINT
        .scope(endpoint, async {
            registry
                .start_registration(
                    &f.state,
                    &prepared.view().id,
                    "owner",
                    &revision,
                    rquest::Client::builder().no_proxy().build().unwrap(),
                )
                .unwrap();
            tokio::time::timeout(Duration::from_secs(5), async {
                while prepared.view().status != RegistrationStatus::Registered {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
        })
        .await;
    registry.join().await;
    let reopened = Registry::default();
    let recovered = reopened
        .prepare_registration_with_offline(&f.state, "owner", "luma-mcp", &revision, true)
        .await
        .unwrap();
    assert_eq!(recovered.view().id, prepared.view().id);
    assert!(recovered.view().offline_access);
    let plain = reopened
        .prepare_registration(&f.state, "owner", "luma-mcp", &revision)
        .await
        .unwrap();
    assert_ne!(plain.view().id, recovered.view().id);
    assert!(!plain.view().offline_access);
    let binding = Binding::new(
        "owner",
        "luma-mcp",
        &revision,
        "offline-client",
        &recovered.view().redirect_uri,
    )
    .unwrap()
    .with_offline_access();
    let session = reopened
        .create(
            &f.state,
            binding,
            rquest::Client::builder().no_proxy().build().unwrap(),
        )
        .await
        .unwrap();
    assert!(session.view().offline_access);
    reopened.join().await;
    stop(task).await;
}
