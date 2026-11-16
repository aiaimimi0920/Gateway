//! Actual management router, synthetic fixed JSON exchange and ordinary configuration persistence.
use super::*;
use crate::{
    console::accio_oauth::SessionStatus, http::router::build_router,
    routing::config::RouteConfigYaml,
};
use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;
mod fixture;
#[path = "../../../../../tests/console_contract_support/mod.rs"]
mod support;
use fixture::*;
mod failures;

#[tokio::test]
async fn normal_router_exchanges_and_saves_complete_material_without_duplicate_or_waiter_ownership()
{
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let view = create(state, &grant, None, Some("synthetic-device")).await;
    let session = session(state, &view);
    let issuer = Issuer::new(true, EXPIRY, None).await;
    complete(state, &view, &grant, &issuer).await;
    let repeated = crate::protocol::accio_oauth::TEST_TOKEN_ENDPOINT
        .scope(
            issuer.endpoint.clone(),
            action(
                state,
                &view,
                Some(&grant),
                "complete",
                Some(&callback(&view)),
            ),
        )
        .await;
    assert_eq!(repeated.status(), StatusCode::BAD_REQUEST);
    let lock = state.lifecycle.health_control.publication.lock().await;
    let response = action(state, &view, Some(&grant), "import", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    drop(response); // Losing an HTTP response cannot cancel an admitted save.
    assert_eq!(session.view().status, SessionStatus::Importing);
    let target = session.view().target_credential_id.unwrap();
    for action_name in ["import", "cancel"] {
        let response = action(state, &view, Some(&grant), action_name, None).await;
        assert_eq!(
            support::parse_json(response).await["session"]["status"],
            "importing"
        );
        assert_eq!(
            session.view().target_credential_id.as_deref(),
            Some(target.as_str())
        );
    }
    drop(lock);
    state.lifecycle.accio_oauth.join().await;
    assert_eq!(session.view().status, SessionStatus::Succeeded);
    let snapshot = state.route_config.snapshot();
    let accounts = &snapshot.document().providers[0].credentials;
    assert_eq!(accounts.len(), 1);
    let saved = &accounts[0];
    assert_eq!(saved.id.as_deref(), Some(target.as_str()));
    assert_eq!(saved.api_key.as_deref(), Some("synthetic-new-access"));
    assert_eq!(
        saved.refresh_token.as_deref(),
        Some("synthetic-new-refresh")
    );
    assert_eq!(saved.accio_sgcookie.as_deref(), Some("synthetic-new-sg"));
    assert_eq!(
        saved.refresh_endpoint.as_deref(),
        Some(crate::protocol::accio_oauth::REFRESH_ENDPOINT)
    );
    assert_eq!(
        saved.refresh_client_id.as_deref(),
        Some("synthetic-explicit-client")
    );
    assert_eq!(saved.expires_at.as_deref(), Some(EXPIRY));
    assert!(saved.token_expires_in_secs.is_none());
    assert_eq!(saved.headers["utdid"], "synthetic-device");
    assert!(saved.extra_body.is_empty());
    assert_eq!(
        session.view().saved_revision.as_deref(),
        Some(snapshot.revision().id())
    );
    assert_eq!(
        snapshot.document().providers[1].api_key,
        "synthetic-other-secret"
    );
    let archive = state
        .route_config_runtime
        .as_ref()
        .unwrap()
        .coordinator()
        .unwrap()
        .load_revision(snapshot.revision().id())
        .unwrap()
        .unwrap();
    assert_eq!(
        archive.document().providers[0].credentials[0].accio_sgcookie,
        saved.accio_sgcookie
    );
    let public = serde_json::to_string(
        &crate::console::secrets::redact_route_document(snapshot.document()).unwrap(),
    )
    .unwrap();
    for secret in [
        "synthetic-new-access",
        "synthetic-new-refresh",
        "synthetic-new-sg",
        "synthetic-other-secret",
    ] {
        assert!(!public.contains(secret));
    }
    assert_eq!(issuer.hits.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[tokio::test]
async fn reauthorization_preserves_stable_id_metadata_and_device_but_replaces_authentication_copies(
) {
    let mut doc = document();
    doc.providers[0].credentials.push(serde_json::from_value(json!({"id":"stable-id","api_key":"synthetic-old-access",
        "enabled":false,"account_name":"operator-name","supported_models":["operator-model"],"refresh_token":"synthetic-old-refresh","accio_sgcookie":"synthetic-old-sg",
        "headers":{"utdid":"existing-device","x-utdid":"existing-device","Cookie":"synthetic-old-cookie","Authorization":"synthetic-old-auth","x-cna":"old-derived-cna"},
        "extra_body":{"token":"synthetic-old-access","accessToken":"synthetic-old-access","utdid":"existing-device",
            "empid":"old-empid","empId":"old-empId","accountId":"old-accountId","account_id":"old-account_id",
            "tenant":"old-tenant","tenantId":"old-tenantId","tenant_id":"old-tenant_id","iaiTag":"operator-tag"}})).unwrap());
    let fixture = support::ConsoleStateFixture::new(doc, true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let view = create(state, &grant, Some("stable-id"), None).await;
    let session = session(state, &view);
    let issuer = Issuer::new(false, EXPIRY, None).await;
    complete(state, &view, &grant, &issuer).await;
    let response = action(state, &view, Some(&grant), "import", None).await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        support::parse_json(response).await
    );
    state.lifecycle.accio_oauth.join().await;
    assert_eq!(session.view().status, SessionStatus::Succeeded);
    let snapshot = state.route_config.snapshot();
    let credentials = &snapshot.document().providers[0].credentials;
    assert_eq!(credentials.len(), 1);
    let saved = &credentials[0];
    assert_eq!(saved.id.as_deref(), Some("stable-id"));
    assert_eq!(saved.enabled, Some(false));
    assert_eq!(saved.account_name.as_deref(), Some("operator-name"));
    assert_eq!(saved.supported_models, ["operator-model"]);
    assert_eq!(saved.headers.len(), 1);
    assert_eq!(saved.headers["utdid"], "existing-device");
    assert_eq!(saved.extra_body.len(), 1);
    assert_eq!(saved.extra_body["iaiTag"], "operator-tag");
    assert!(saved.accio_sgcookie.is_none());
    assert!(saved.auth_token.is_none());
    assert_eq!(saved.api_key.as_deref(), Some("synthetic-new-access"));
    assert_eq!(
        saved.refresh_token.as_deref(),
        Some("synthetic-new-refresh")
    );
}

#[tokio::test]
async fn already_expired_access_is_saved_with_refresh_and_its_original_absolute_expiry() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let view = create(state, &grant, None, Some("synthetic-device")).await;
    let session = session(state, &view);
    let issuer = Issuer::new(true, "2000-01-01T00:00:00.123456789Z", None).await;
    complete(state, &view, &grant, &issuer).await;
    assert_eq!(
        action(state, &view, Some(&grant), "import", None)
            .await
            .status(),
        StatusCode::OK
    );
    state.lifecycle.accio_oauth.join().await;
    assert_eq!(session.view().status, SessionStatus::Succeeded);
    let snapshot = state.route_config.snapshot();
    let saved = &snapshot.document().providers[0].credentials[0];
    assert_eq!(
        saved.expires_at.as_deref(),
        Some("2000-01-01T00:00:00.123456789Z")
    );
    assert_eq!(
        saved.refresh_token.as_deref(),
        Some("synthetic-new-refresh")
    );
    assert!(snapshot.get_providers()[0].credential_pool[0]
        .refresh_config
        .as_ref()
        .unwrap()
        .dispatch_readiness()
        .is_err());
}
