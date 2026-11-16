//! Normal management routes use a synthetic registration peer; no live public-client creation.
use super::{
    fixture::{assert_port_free, fixture, redirect},
    support,
};
use crate::{
    console::luma_oauth::RegistrationStatus,
    protocol::luma_oauth::test_support::REGISTRATION_ENDPOINT,
};
use axum::{
    http::{Method, StatusCode},
    routing::post,
    Json, Router,
};
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
const PATH: &str = "/v1/internal/gateway/console/luma-client-registrations";

#[tokio::test]
async fn luma_registration_restart_recovers_unknown_receipt_without_new_registration() {
    let f = fixture(&redirect(), false);
    let revision = f.state.route_config.snapshot().revision().id().to_string();
    let registry = &f.state.lifecycle.luma_oauth;
    let receipt = registry
        .prepare_registration(&f.state, "owner", "luma-mcp", &revision)
        .await
        .unwrap();
    let original = receipt.view();
    let (endpoint, calls, task) = peer(Duration::ZERO, false).await;
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
            tokio::time::timeout(Duration::from_secs(3), async {
                while receipt.view().status != RegistrationStatus::Unconfirmed {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            registry.join().await;
            let restarted = crate::console::luma_oauth::Registry::default();
            let restored = restarted
                .prepare_registration(&f.state, "owner", "luma-mcp", &revision)
                .await
                .unwrap();
            assert_eq!(restored.view().id, original.id);
            assert!(restored.view().status == RegistrationStatus::Unconfirmed);
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            restarted.join().await;
        })
        .await;
    stop(task).await;
}

#[tokio::test(start_paused = true)]
async fn luma_registration_never_started_expired_receipt_can_be_prepared_again() {
    let f = fixture(&redirect(), false);
    let revision = f.state.route_config.snapshot().revision().id().to_string();
    let registry = &f.state.lifecycle.luma_oauth;
    let original = registry
        .prepare_registration(&f.state, "owner", "luma-mcp", &revision)
        .await
        .unwrap();
    tokio::time::advance(Duration::from_secs(1201)).await;
    assert!(original.view().status == RegistrationStatus::Expired);
    assert_port_free(&original.view().redirect_uri);
    let next = registry
        .prepare_registration(&f.state, "owner", "luma-mcp", &revision)
        .await
        .unwrap();
    assert_ne!(next.view().id, original.view().id);
    assert!(next.view().status == RegistrationStatus::Prepared);
    assert!(registry
        .start_registration(
            &f.state,
            &original.view().id,
            "owner",
            &revision,
            rquest::Client::builder().no_proxy().build().unwrap()
        )
        .is_err());
    registry.join().await;
    assert_port_free(&next.view().redirect_uri);
}

pub(super) async fn peer(
    delay: Duration,
    valid: bool,
) -> (String, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let router = Router::new().route("/register", post(move |Json(body): Json<Value>| {
        let calls = observed.clone(); async move {
            calls.fetch_add(1, Ordering::SeqCst); tokio::time::sleep(delay).await;
            let response = if valid { json!({"client_id":"new-synthetic-client", "redirect_uris":body["redirect_uris"],
                "grant_types":["authorization_code"],"response_types":["code"],"token_endpoint_auth_method":"none"}) }
                else { json!({"error":"private-upstream"}) };
            (StatusCode::CREATED, Json(response))
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/register", listener.local_addr().unwrap());
    (
        endpoint,
        calls,
        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        }),
    )
}
pub(super) async fn stop(task: tokio::task::JoinHandle<()>) {
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
}

#[tokio::test]
async fn luma_registration_management_requires_confirmation_and_retains_one_attempt() {
    let f = fixture(&redirect(), false);
    let grant = support::grant_console_secret_access(&f.state).await;
    let revision = f.state.route_config.snapshot().revision().id().to_string();
    let prepare = json!({"providerId":"luma-mcp","expectedRevision":revision});
    let denied =
        support::send_console_json(&f.state, Method::POST, PATH, prepare.clone(), None, None).await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let (endpoint, calls, task) = peer(Duration::from_millis(50), true).await;
    REGISTRATION_ENDPOINT
        .scope(endpoint, async {
            let response = support::send_console_json(
                &f.state,
                Method::POST,
                PATH,
                prepare.clone(),
                Some(&grant),
                None,
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()["cache-control"], "no-store");
            let view = support::parse_json(response).await["registration"].clone();
            assert_eq!(view["status"], "prepared");
            assert_eq!(calls.load(Ordering::SeqCst), 0);
            let port = url::Url::parse(view["redirectUri"].as_str().unwrap())
                .unwrap()
                .port()
                .unwrap();
            assert!(std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).is_err());
            let start = format!("{PATH}/{}/start", view["id"].as_str().unwrap());
            let denied = support::send_console_json(
                &f.state,
                Method::POST,
                &start,
                json!({"expectedRevision":revision,"confirmRegistration":false}),
                Some(&grant),
                None,
            )
            .await;
            assert_eq!(denied.status(), StatusCode::BAD_REQUEST);
            for _ in 0..3 {
                let response = support::send_console_json(
                    &f.state,
                    Method::POST,
                    &start,
                    json!({"expectedRevision":revision,"confirmRegistration":true}),
                    Some(&grant),
                    None,
                )
                .await;
                assert_eq!(response.status(), StatusCode::OK);
            }
            let get = format!("{PATH}/{}", view["id"].as_str().unwrap());
            let registered = tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    let r = support::send_console_json(
                        &f.state,
                        Method::GET,
                        &get,
                        Value::Null,
                        None,
                        None,
                    )
                    .await;
                    let value = support::parse_json(r).await["registration"].clone();
                    if value["status"] == "registered" {
                        break value;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            assert_eq!(registered["clientId"], "new-synthetic-client");
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            assert_port_free(registered["redirectUri"].as_str().unwrap());
            let again = support::send_console_json(
                &f.state,
                Method::POST,
                PATH,
                prepare,
                Some(&grant),
                None,
            )
            .await;
            assert_eq!(
                support::parse_json(again).await["registration"]["id"],
                view["id"]
            );
            assert_eq!(f.state.route_config.snapshot().revision().id(), revision);
        })
        .await;
    f.state.lifecycle.luma_oauth.join().await;
    stop(task).await;
}

#[tokio::test]
async fn luma_registration_unknown_result_and_owner_mismatch_cannot_replay() {
    let f = fixture(&redirect(), false);
    let revision = f.state.route_config.snapshot().revision().id().to_string();
    let registry = &f.state.lifecycle.luma_oauth;
    let receipt = registry
        .prepare_registration(&f.state, "owner", "luma-mcp", &revision)
        .await
        .unwrap();
    let view = receipt.view();
    assert!(registry
        .registration_owned(&view.id, "other-owner")
        .is_err());
    let (endpoint, calls, task) = peer(Duration::ZERO, false).await;
    REGISTRATION_ENDPOINT
        .scope(endpoint, async {
            let http = rquest::Client::builder().no_proxy().build().unwrap();
            registry
                .start_registration(&f.state, &view.id, "owner", &revision, http.clone())
                .unwrap();
            tokio::time::timeout(Duration::from_secs(3), async {
                while receipt.view().status != RegistrationStatus::Unconfirmed {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            registry
                .start_registration(&f.state, &view.id, "owner", &revision, http)
                .unwrap();
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            assert!(!serde_json::to_string(&receipt.view())
                .unwrap()
                .contains("private-upstream"));
        })
        .await;
    registry.join().await;
    assert_port_free(&view.redirect_uri);
    stop(task).await;
}

#[tokio::test]
async fn luma_registration_shutdown_joins_running_attempt_and_releases_prepared_ports() {
    let f = fixture(&redirect(), false);
    let revision = f.state.route_config.snapshot().revision().id().to_string();
    let registry = &f.state.lifecycle.luma_oauth;
    let running = registry
        .prepare_registration(&f.state, "owner", "luma-mcp", &revision)
        .await
        .unwrap();
    let prepared = registry
        .prepare_registration(&f.state, "second-owner", "luma-mcp", &revision)
        .await
        .unwrap();
    let (endpoint, calls, task) = peer(Duration::from_secs(60), true).await;
    REGISTRATION_ENDPOINT
        .scope(endpoint, async {
            registry
                .start_registration(
                    &f.state,
                    &running.view().id,
                    "owner",
                    &revision,
                    rquest::Client::builder().no_proxy().build().unwrap(),
                )
                .unwrap();
            tokio::time::timeout(Duration::from_secs(3), async {
                while calls.load(Ordering::SeqCst) == 0 {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            tokio::time::timeout(Duration::from_secs(3), registry.join())
                .await
                .unwrap();
        })
        .await;
    assert!(running.view().status == RegistrationStatus::Unconfirmed);
    assert!(prepared.view().status == RegistrationStatus::Cancelled);
    assert_port_free(&running.view().redirect_uri);
    assert_port_free(&prepared.view().redirect_uri);
    stop(task).await;
}
