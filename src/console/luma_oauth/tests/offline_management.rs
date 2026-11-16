//! Explicit offline consent uses the normal authenticated management path, never a live account.
use super::{fixture::*, support};
use crate::protocol::luma_oauth::test_support::ENDPOINT;
use axum::http::{Method, StatusCode};
use serde_json::{json, Value};
use std::time::Duration;

#[tokio::test]
async fn luma_offline_management_explicit_opt_in_prepares_consent_scope() {
    let redirect = redirect();
    let f = fixture(&redirect, false);
    let grant = support::grant_console_secret_access(&f.state).await;
    let revision = f.state.route_config.snapshot().revision().id().to_string();
    let response = support::send_console_json(
        &f.state,
        Method::POST,
        PATH,
        json!({"providerId":"luma-mcp","expectedRevision":revision,"clientId":"registered-client",
            "redirectUri":redirect,"offlineAccess":true}),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let view = support::parse_json(response).await["session"].clone();
    let url = url::Url::parse(view["authorizationUrl"].as_str().unwrap()).unwrap();
    let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(query["scope"], "openid offline_access");
    assert_eq!(query["prompt"], "consent");
    assert_eq!(view["offlineAccess"], true);
    f.state.lifecycle.luma_oauth.join().await;
    assert_port_free(&redirect);
}

#[tokio::test]
async fn luma_offline_management_saves_pair_then_plain_reauthorization_clears_refresh() {
    for existing in [false, true] {
        let redirect = redirect();
        let f = fixture(&redirect, existing);
        let peer = Peer::with_reply(
            Duration::ZERO,
            json!({"access_token":"synthetic-luma-access",
            "token_type":"Bearer","refresh_token":"synthetic-refresh","expires_in":3600}),
        )
        .await;
        ENDPOINT
            .scope(peer.endpoint.clone(), async {
                let grant = support::grant_console_secret_access(&f.state).await;
                let mut credential_id = existing.then(|| "saved-mcp".to_owned());
                for offline in [true, false] {
                    let revision = f.state.route_config.snapshot().revision().id().to_string();
                    let response = support::send_console_json(
                        &f.state,
                        Method::POST,
                        PATH,
                        json!({"providerId":"luma-mcp","expectedRevision":revision,
                        "clientId":"registered-client","redirectUri":redirect,
                        "credentialId":credential_id,"offlineAccess":offline}),
                        Some(&grant),
                        None,
                    )
                    .await;
                    assert_eq!(response.status(), StatusCode::OK);
                    let view = support::parse_json(response).await["session"].clone();
                    let id = view["id"].as_str().unwrap();
                    let response = support::send_console_json(
                        &f.state,
                        Method::POST,
                        &format!("{PATH}/{id}/complete"),
                        json!({"expectedRevision":revision,
                        "callbackUrl":callback_view(&view)}),
                        Some(&grant),
                        None,
                    )
                    .await;
                    assert_eq!(response.status(), StatusCode::OK);
                    let ready = support::parse_json(response).await;
                    assert_eq!(ready["session"]["status"], "ready");
                    assert!(!ready.to_string().contains("synthetic-"));
                    assert_eq!(f.state.route_config.snapshot().revision().id(), revision);
                    let response = support::send_console_json(
                        &f.state,
                        Method::POST,
                        &format!("{PATH}/{id}/import"),
                        json!({"expectedRevision":revision}),
                        Some(&grant),
                        None,
                    )
                    .await;
                    assert_eq!(response.status(), StatusCode::OK);
                    tokio::time::timeout(Duration::from_secs(5), async {
                        loop {
                            let response = support::send_console_json(
                                &f.state,
                                Method::GET,
                                &format!("{PATH}/{id}"),
                                Value::Null,
                                None,
                                None,
                            )
                            .await;
                            let saved = support::parse_json(response).await;
                            assert!(!saved.to_string().contains("synthetic-"));
                            if saved["session"]["status"] == "succeeded" {
                                break;
                            }
                            assert_eq!(saved["session"]["status"], "importing", "{saved}");
                            tokio::time::sleep(Duration::from_millis(10)).await;
                        }
                    })
                    .await
                    .unwrap();
                    let snapshot = f.state.route_config.snapshot();
                    let c = &snapshot.document().providers[0].credentials[0];
                    assert_eq!(c.api_key.as_deref(), Some("synthetic-luma-access"));
                    assert_eq!(
                        c.refresh_token.as_deref(),
                        offline.then_some("synthetic-refresh")
                    );
                    assert_eq!(c.enabled, Some(false));
                    assert_eq!(c.luma_oauth.as_ref().unwrap().offline_access(), offline);
                    if let Some(id) = &credential_id {
                        assert_eq!(c.id.as_ref(), Some(id));
                    }
                    if existing {
                        assert_eq!(c.account_name.as_deref(), Some("preserved account name"));
                    }
                    credential_id = c.id.clone();
                }
            })
            .await;
        assert_eq!(peer.count(), 2);
        f.state.lifecycle.luma_oauth.join().await;
        assert_port_free(&redirect);
        peer.stop().await;
    }
}

#[tokio::test]
async fn luma_offline_management_rejects_known_code_only_client_across_owners() {
    use crate::console::{luma_oauth::registration_storage, persistence::LumaRegistrationRecord};
    let redirect = redirect();
    let f = fixture(&redirect, false);
    let revision = f.state.route_config.snapshot().revision().id().to_string();
    let store = registration_storage::persistence(&f.state).unwrap();
    let record = LumaRegistrationRecord {
        offline_access: false,
        id: uuid::Uuid::new_v4().to_string(),
        owner: "different-owner".into(),
        provider_id: "luma-mcp".into(),
        expected_revision: revision.clone(),
        redirect_uri: redirect.clone(),
        client_id: None,
    };
    assert!(registration_storage::claim(store.clone(), record.clone())
        .await
        .unwrap());
    registration_storage::finish(store, record, "registered-client".into())
        .await
        .unwrap();
    let grant = support::grant_console_secret_access(&f.state).await;
    let response = support::send_console_json(
        &f.state,
        Method::POST,
        PATH,
        json!({"providerId":"luma-mcp","expectedRevision":revision,"clientId":"registered-client",
            "redirectUri":redirect,"offlineAccess":true}),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(support::parse_json(response)
        .await
        .to_string()
        .contains("luma_offline_client_required"));
    assert_port_free(&redirect);
    f.state.lifecycle.luma_oauth.join().await;
}
