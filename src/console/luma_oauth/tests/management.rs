use super::{fixture::*, support};
use crate::protocol::luma_oauth::test_support::ENDPOINT;
use axum::http::{Method, StatusCode};
use serde_json::{json, Value};
use std::time::Duration;

#[tokio::test]
async fn luma_login_loopback_exchange_and_explicit_import_preserve_new_or_existing_account() {
    for existing in [false, true] {
        let redirect = redirect();
        let f = fixture(&redirect, existing);
        let peer = Peer::new(Duration::ZERO).await;
        ENDPOINT
            .scope(peer.endpoint.clone(), async {
                let grant = support::grant_console_secret_access(&f.state).await;
                let revision = f.state.route_config.snapshot().revision().id().to_string();
                let mut body = json!({"providerId":"luma-mcp","expectedRevision":revision,
                "clientId":"registered-client","redirectUri":redirect});
                if existing {
                    body["credentialId"] = json!("saved-mcp");
                }
                let response = support::send_console_json(
                    &f.state,
                    Method::POST,
                    PATH,
                    body,
                    Some(&grant),
                    None,
                )
                .await;
                assert_eq!(response.status(), StatusCode::OK);
                assert_eq!(response.headers()["cache-control"], "no-store");
                let view = support::parse_json(response).await["session"].clone();
                assert_eq!(view["status"], "waiting_user");
                let id = view["id"].as_str().unwrap();
                let callback = callback_view(&view);
                let http = rquest::Client::builder().no_proxy().build().unwrap();
                let wrong = callback.replace("state=", "state=wrong");
                assert_eq!(
                    http.get(&wrong).send().await.unwrap().status().as_u16(),
                    400
                );
                assert_eq!(peer.count(), 0);
                assert_eq!(
                    http.get(&callback).send().await.unwrap().status().as_u16(),
                    200
                );
                assert_eq!(peer.count(), 1);
                assert_eq!(f.state.route_config.snapshot().revision().id(), revision);
                let get = format!("{PATH}/{id}");
                let response = support::send_console_json(
                    &f.state,
                    Method::GET,
                    &get,
                    Value::Null,
                    None,
                    None,
                )
                .await;
                let ready = support::parse_json(response).await;
                assert_eq!(ready["session"]["status"], "ready");
                assert!(ready["session"]["authorizationUrl"].is_null());
                assert!(!ready.to_string().contains("synthetic-luma-access"));
                let import = format!("{PATH}/{id}/import");
                let response = support::send_console_json(
                    &f.state,
                    Method::POST,
                    &import,
                    json!({"expectedRevision":revision}),
                    None,
                    None,
                )
                .await;
                assert_eq!(response.status(), StatusCode::FORBIDDEN);
                let response = support::send_console_json(
                    &f.state,
                    Method::POST,
                    &import,
                    json!({"expectedRevision":revision}),
                    Some(&grant),
                    None,
                )
                .await;
                assert_eq!(response.status(), StatusCode::OK);
                let saved = tokio::time::timeout(Duration::from_secs(5), async {
                    loop {
                        let response = support::send_console_json(
                            &f.state,
                            Method::GET,
                            &get,
                            Value::Null,
                            None,
                            None,
                        )
                        .await;
                        let body = support::parse_json(response).await;
                        if body["session"]["status"] == "succeeded" {
                            break body;
                        }
                        assert_eq!(body["session"]["status"], "importing", "{body}");
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                })
                .await
                .unwrap();
                assert!(!saved.to_string().contains("synthetic-luma-access"));
                let snapshot = f.state.route_config.snapshot();
                let credential = &snapshot.document().providers[0].credentials[0];
                assert_eq!(credential.api_key.as_deref(), Some("synthetic-luma-access"));
                assert_eq!(credential.enabled, Some(false));
                assert_eq!(
                    credential.luma_oauth.as_ref().unwrap().client_id(),
                    "registered-client"
                );
                assert!(credential
                    .luma_oauth
                    .as_ref()
                    .unwrap()
                    .expires_at()
                    .is_some());
                if existing {
                    assert_eq!(credential.id.as_deref(), Some("saved-mcp"));
                    assert_eq!(
                        credential.account_name.as_deref(),
                        Some("preserved account name")
                    );
                }
                let saved_revision = snapshot.revision().id().to_string();
                let response = support::send_console_json(
                    &f.state,
                    Method::POST,
                    &import,
                    json!({"expectedRevision":revision}),
                    Some(&grant),
                    None,
                )
                .await;
                assert_eq!(response.status(), StatusCode::OK);
                assert_eq!(
                    f.state.route_config.snapshot().revision().id(),
                    saved_revision
                );
                assert_eq!(peer.count(), 1);
                f.state.lifecycle.luma_oauth.join().await;
                assert_port_free(&redirect);
            })
            .await;
        peer.stop().await;
    }
}

#[tokio::test]
async fn luma_login_import_rejects_target_revision_changed_after_authorization() {
    let redirect = redirect();
    let f = fixture(&redirect, false);
    let peer = Peer::new(Duration::ZERO).await;
    let registry = &f.state.lifecycle.luma_oauth;
    let b = binding(&f, &redirect);
    let session = registry
        .create(
            &f.state,
            b.clone(),
            rquest::Client::builder().no_proxy().build().unwrap(),
        )
        .await
        .unwrap();
    ENDPOINT
        .scope(
            peer.endpoint.clone(),
            session.complete(
                &rquest::Client::builder().no_proxy().build().unwrap(),
                &callback(&session, None),
            ),
        )
        .await
        .unwrap();
    let snapshot = f.state.route_config.snapshot();
    let mut changed = snapshot.document().clone();
    changed.providers[0].label = Some("changed after authorization".into());
    f.state
        .route_config_runtime
        .as_ref()
        .unwrap()
        .commit_document(snapshot.revision().id(), changed, None)
        .await
        .unwrap();
    let error = registry
        .start_import(f.state.clone(), session.id(), &b)
        .err()
        .unwrap();
    assert_eq!(error.code.as_deref(), Some("console_revision_conflict"));
    assert_eq!(peer.count(), 1);
    assert!(f.state.route_config.snapshot().document().providers[0]
        .credentials
        .is_empty());
    registry.join().await;
    peer.stop().await;
    assert_port_free(&redirect);
}
