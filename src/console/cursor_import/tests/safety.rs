use super::*;
use axum::{body::Body, http::Request};
use std::sync::atomic::Ordering;
use tower::ServiceExt;

#[tokio::test]
async fn cursor_explicit_invalid_email_is_rejected_before_creating_an_authorization() {
    let fixture = Fixture::new().await;
    for email in ["", " ", "invalid", "owner @example.test"] {
        let mut input = fixture.body(false);
        input["expectedEmail"] = json!(email);
        let id = input["attemptId"].as_str().unwrap().to_owned();
        let response = fixture
            .call(Method::POST, ROOT, input, Some(&fixture.grant))
            .await;
        assert_eq!(
            support::parse_json(response).await["error"]["code"],
            "cursor_invalid_expected_email"
        );
        let response = fixture
            .call(Method::GET, &format!("{ROOT}/{id}"), Value::Null, None)
            .await;
        assert_eq!(
            support::parse_json(response).await["error"]["code"],
            "cursor_import_session_unavailable"
        );
    }
    fixture.close().await;
}

#[tokio::test]
async fn cursor_import_requires_management_secret_grant_origin_and_current_revision() {
    let fixture = Fixture::new().await;
    let input = fixture.body(true);
    let unauth = crate::http::router::build_router(fixture.state.clone())
        .oneshot(
            Request::post(ROOT)
                .header("content-type", "application/json")
                .body(Body::from(input.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(unauth.status(), StatusCode::OK);
    assert_eq!(unauth.headers()["cache-control"], "no-store");
    let no_grant = fixture.call(Method::POST, ROOT, input.clone(), None).await;
    assert_eq!(no_grant.status(), StatusCode::FORBIDDEN);
    let foreign = support::send_console_json(
        &fixture.state,
        Method::POST,
        ROOT,
        input.clone(),
        Some(&fixture.grant),
        Some("https://attacker.invalid"),
    )
    .await;
    assert_ne!(foreign.status(), StatusCode::OK);
    let mut stale = input;
    stale["expectedRevision"] = json!("old");
    let rejected = fixture
        .call(Method::POST, ROOT, stale, Some(&fixture.grant))
        .await;
    assert_eq!(
        support::parse_json(rejected).await["error"]["code"],
        "console_revision_conflict"
    );
    fixture.close().await;
}

#[tokio::test]
async fn cursor_immediate_cancel_and_shutdown_send_no_poll_or_identity_request() {
    let fixture = Fixture::new().await;
    let peer = Peer::new(false).await;
    for shutdown in [false, true] {
        // Direct synchronous create/cancel intentionally does not yield to the spawned task.
        let context = crate::console::ConsoleRequestContext::loopback();
        let actor = fixture
            .state
            .console_auth
            .authenticate_management_token(&context, support::MANAGEMENT_TOKEN)
            .unwrap();
        let grant = fixture
            .state
            .console_auth
            .confirm_secret_access(
                &context,
                support::MANAGEMENT_TOKEN,
                support::MANAGEMENT_TOKEN,
            )
            .unwrap();
        let consent = Consent {
            actor,
            context,
            grant: grant.grant,
        };
        TEST_ORIGIN
            .scope(peer.origin.clone(), async {
                let view = fixture
                    .state
                    .lifecycle
                    .cursor_import
                    .create(
                        fixture.state.clone(),
                        "owner".into(),
                        serde_json::from_value(fixture.body(false)).unwrap(),
                        consent,
                    )
                    .unwrap();
                if shutdown {
                    fixture.state.lifecycle.cursor_import.close();
                } else {
                    fixture
                        .state
                        .lifecycle
                        .cursor_import
                        .get(&view.id, "owner")
                        .unwrap()
                        .cancel();
                }
                assert!(fixture
                    .state
                    .lifecycle
                    .cursor_import
                    .get(&view.id, "foreign")
                    .is_err());
                tokio::task::yield_now().await;
                assert_eq!(
                    fixture
                        .state
                        .lifecycle
                        .cursor_import
                        .get(&view.id, "owner")
                        .unwrap()
                        .view()
                        .status,
                    "cancelled"
                );
            })
            .await;
    }
    assert_eq!(peer.polls.load(Ordering::SeqCst), 0);
    assert_eq!(peer.checks.load(Ordering::SeqCst), 0);
    fixture.close().await;
}

#[tokio::test]
async fn cursor_generic_secret_patch_cannot_bypass_verified_import() {
    let fixture = Fixture::new().await;
    let mut candidate = crate::console::secrets::redact_route_document(&document())
        .unwrap()
        .document;
    candidate.providers[0].credentials.push(serde_json::from_value(json!({
        "id":"injected","account_name":"owner@example.test","enabled":false,"expires_at":"2099-01-01T00:00:00Z"
    })).unwrap());
    let response = fixture.call(Method::PUT, "/v1/internal/gateway/console/route-config", json!({
        "expectedRevision":fixture.state.route_config.snapshot().revision().id(),"document":candidate,
        "secretPatches":[{"path":"/providers/0/credentials/0/auth_token","operation":"replace","value":access()},
            {"path":"/providers/1/api_key","operation":"keep"}]
    }), Some(&fixture.grant)).await;
    let value = support::parse_json(response).await;
    assert_eq!(value["error"]["code"], "cursor_import_required");
    assert!(!value.to_string().contains(&access()));
    assert!(
        fixture.state.route_config.snapshot().document().providers[0]
            .credentials
            .is_empty()
    );
    fixture.close().await;
}

#[test]
fn cursor_import_only_config_rejects_automation_and_preserves_expired_material_on_read() {
    use crate::routing::config::RouteConfigStore;
    let mut value = serde_json::to_value(document()).unwrap();
    value["providers"][0]["credentials"] = json!([{"id":"expired","enabled":false,
        "auth_token":crate::protocol::cursor_auth::tests::token("auth0|synthetic",1),"expires_at":"1970-01-01T00:00:01Z"}]);
    let saved: RouteConfigYaml = serde_json::from_value(value.clone()).unwrap();
    assert!(RouteConfigStore::from_document(saved.clone()).is_ok());
    assert!(validate_generic_update(&saved, &saved).is_ok());
    for (field, forbidden) in [
        ("base_url", json!("https://example.invalid")),
        ("supported_models", json!(["model"])),
        ("scheduled_probe_enabled", json!(true)),
        ("auto_refill_enabled", json!(true)),
    ] {
        let mut changed = value.clone();
        changed["providers"][0][field] = forbidden;
        assert!(
            RouteConfigStore::from_document(serde_json::from_value(changed).unwrap()).is_err(),
            "{field}"
        );
    }
    let mut routed = value;
    routed["model_routes"] = json!([{"pattern":"model","provider_ids":["cursor"]}]);
    assert!(RouteConfigStore::from_document(serde_json::from_value(routed).unwrap()).is_err());
}
