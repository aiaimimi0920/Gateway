//! Actual management router -> synthetic OIDC -> ordinary SecretPatch/CAS and durable refresh context.
use super::*;
use crate::{
    console::kiro_oauth::{test_support::*, Session, SessionStatus},
    http::router::build_router,
    protocol::kiro_oauth::TEST_ORIGIN,
};
use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;
mod directory;
mod legacy;
mod safety;
#[path = "../../../../../tests/console_contract_support/mod.rs"]
mod support;
const ROOT: &str = "/v1/internal/gateway/console/kiro-auth-sessions";
fn create_body(state: &Arc<AppState>) -> Value {
    json!({"providerId":"kiro-pool","expectedRevision":state.route_config.snapshot().revision().id(),
        "directory":{"authMethod":"builder_id"},"scopes":["codewhisperer:conversations"],"machineId":"a".repeat(64)})
}
async fn create(
    state: &Arc<AppState>,
    grant: &str,
    body: Value,
    mock: &Mock,
) -> (Value, Arc<Session>) {
    let response = TEST_ORIGIN
        .scope(
            mock.origin.clone(),
            support::send_console_json(state, Method::POST, ROOT, body, Some(grant), None),
        )
        .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        support::parse_json(response).await
    );
    let view = support::parse_json(response).await["session"].clone();
    let (owner, _) = authorize(
        state,
        &HeaderMap::new(),
        Some(support::MANAGEMENT_TOKEN),
        None,
        false,
    )
    .unwrap();
    let session = state
        .lifecycle
        .kiro_oauth
        .get_owned(view["id"].as_str().unwrap(), &owner)
        .unwrap();
    wait(&session, SessionStatus::WaitingUser).await;
    (view, session)
}
async fn action(
    state: &Arc<AppState>,
    view: &Value,
    grant: Option<&str>,
    action: &str,
) -> Response {
    support::send_console_json(
        state,
        Method::POST,
        &format!("{ROOT}/{}/{action}", view["id"].as_str().unwrap()),
        json!({"expectedRevision":view["expectedRevision"]}),
        grant,
        None,
    )
    .await
}
async fn ready(
    state: &Arc<AppState>,
    view: &Value,
    session: &Arc<Session>,
    grant: &str,
    mock: &Mock,
) {
    let response = TEST_ORIGIN
        .scope(
            mock.origin.clone(),
            action(state, view, Some(grant), "poll"),
        )
        .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        support::parse_json(response).await
    );
    wait(session, SessionStatus::Ready).await;
}

#[tokio::test]
async fn normal_router_paces_single_steps_and_saves_registered_secret_without_waiter_or_duplicate_writes(
) {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let mock = Mock::new(vec![
        registration(),
        device(),
        ResponseSpec::json(400, json!({"error":"authorization_pending"})),
        ResponseSpec::json(400, json!({"error":"slow_down"})),
        tokens(true, true),
    ])
    .await;
    let (view, session) = create(state, &grant, create_body(state), &mock).await;
    for expected_interval in [1, 6] {
        let response = TEST_ORIGIN
            .scope(
                mock.origin.clone(),
                action(state, &view, Some(&grant), "poll"),
            )
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(session.view().status, SessionStatus::Polling);
        let duplicate = action(state, &view, Some(&grant), "poll").await;
        assert_eq!(duplicate.status(), StatusCode::OK);
        wait(&session, SessionStatus::WaitingUser).await;
        assert_eq!(session.view().poll_interval_secs, Some(expected_interval));
    }
    ready(state, &view, &session, &grant, &mock).await;
    let public = serde_json::to_string(&session.view()).unwrap();
    for secret in [
        "synthetic-console-access",
        "synthetic-console-refresh",
        "synthetic-console-secret",
        "synthetic-device-secret",
    ] {
        assert!(!public.contains(secret));
    }
    let lock = state.lifecycle.health_control.publication.lock().await;
    let response = action(state, &view, Some(&grant), "import").await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        support::parse_json(response).await
    );
    assert_eq!(response.headers()["cache-control"], "no-store");
    drop(response);
    let target = session.view().target_credential_id.unwrap();
    for action_name in ["import", "cancel"] {
        assert_eq!(
            action(state, &view, Some(&grant), action_name)
                .await
                .status(),
            StatusCode::OK
        );
        assert_eq!(session.view().status, SessionStatus::Importing);
        assert_eq!(
            session.view().target_credential_id.as_deref(),
            Some(target.as_str())
        );
    }
    drop(lock);
    state.lifecycle.kiro_oauth.join().await;
    assert_eq!(session.view().status, SessionStatus::Succeeded);
    let snapshot = state.route_config.snapshot();
    let saved = &snapshot.document().providers[0].credentials[0];
    assert_eq!(snapshot.document().providers[0].credentials.len(), 1);
    assert_eq!(saved.id.as_deref(), Some(target.as_str()));
    assert_eq!(saved.api_key.as_deref(), Some("synthetic-console-access"));
    assert_eq!(
        saved.refresh_token.as_deref(),
        Some("synthetic-console-refresh")
    );
    let context = saved.kiro_oauth.as_ref().unwrap();
    context.validate().unwrap();
    assert_eq!(context.client_secret(), Some("synthetic-console-secret"));
    assert_eq!(context.machine_id(), "a".repeat(64));
    assert_eq!(
        saved.refresh_endpoint.as_deref(),
        Some(context.token_endpoint().as_str())
    );
    assert_eq!(
        saved.refresh_client_id.as_deref(),
        Some(context.client_id())
    );
    assert!(saved.expires_at.is_some());
    assert_eq!(
        saved.session_auth.as_ref().unwrap().expires_at,
        saved.expires_at
    );
    assert!(saved.token_expires_in_secs.is_none());
    let attempt = snapshot.get_providers()[0].credential_pool[0]
        .refresh_config
        .as_ref()
        .unwrap()
        .begin_refresh(
            std::time::Duration::from_secs(7200),
            saved.api_key.as_deref().unwrap(),
        )
        .unwrap();
    assert_eq!(attempt.kiro_oauth.as_ref(), Some(context));
    drop(attempt);
    assert_eq!(
        session.view().saved_revision.as_deref(),
        Some(snapshot.revision().id())
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
        archive.document().providers[0].credentials[0]
            .kiro_oauth
            .as_ref(),
        Some(context)
    );
    assert_eq!(
        snapshot.document().providers[1].api_key,
        "synthetic-other-secret"
    );
    let redacted = serde_json::to_string(
        &crate::console::secrets::redact_route_document(snapshot.document()).unwrap(),
    )
    .unwrap();
    for secret in [
        "synthetic-console-access",
        "synthetic-console-refresh",
        "synthetic-console-secret",
        "synthetic-other-secret",
    ] {
        assert!(!redacted.contains(secret));
    }
    let calls = mock.calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 5);
    assert_eq!(
        calls.iter().map(|c| c.0.as_str()).collect::<Vec<_>>(),
        [
            "/client/register",
            "/device_authorization",
            "/token",
            "/token",
            "/token"
        ]
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&calls[0].2).unwrap()["scopes"],
        json!(["codewhisperer:conversations"])
    );
    mock.stop().await;
}

#[tokio::test]
async fn reauthorization_preserves_stable_device_and_operator_metadata_but_never_old_refresh_or_expiry(
) {
    let mut doc = document();
    doc.providers[0].credentials.push(serde_json::from_value(json!({"id":"stable-id","api_key":"synthetic-old-access","enabled":false,
        "account_name":"operator-name","supported_models":["operator-model"],"refresh_token":"synthetic-old-refresh","expires_at":"2099-01-01T00:00:00Z",
        "extra_body":{"kiroMachineId":"b".repeat(64),"operatorTag":"keep"},
        "session_auth":{"transport":"bearer","expiresAt":"2000-01-01T00:00:00Z"},
        "kiro_oauth":{"auth_method":"builder_id","directory_region":"us-east-1","start_url":"https://view.awsapps.com/start","client_id":"old-client",
            "client_secret":"synthetic-old-secret","client_secret_expires_at":4102444800i64,"scopes":["codewhisperer:conversations"],"machine_id":"b".repeat(64)}})).unwrap());
    let fixture = support::ConsoleStateFixture::new(doc, true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let mut body = create_body(state);
    body.as_object_mut().unwrap().remove("machineId");
    body["credentialId"] = json!("stable-id");
    let mock = Mock::new(vec![registration(), device(), tokens(false, false)]).await;
    let mut wrong_machine = body.clone();
    wrong_machine["machineId"] = json!("c".repeat(64));
    assert_eq!(
        support::send_console_json(state, Method::POST, ROOT, wrong_machine, Some(&grant), None)
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(mock.count(), 0);
    let (view, session) = create(state, &grant, body, &mock).await;
    ready(state, &view, &session, &grant, &mock).await;
    let response = action(state, &view, Some(&grant), "import").await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        support::parse_json(response).await
    );
    state.lifecycle.kiro_oauth.join().await;
    assert_eq!(session.view().status, SessionStatus::Succeeded);
    let snapshot = state.route_config.snapshot();
    let saved = &snapshot.document().providers[0].credentials[0];
    assert_eq!(snapshot.document().providers[0].credentials.len(), 1);
    assert_eq!(saved.id.as_deref(), Some("stable-id"));
    assert_eq!(saved.enabled, Some(false));
    assert_eq!(saved.account_name.as_deref(), Some("operator-name"));
    assert_eq!(saved.supported_models, ["operator-model"]);
    assert_eq!(saved.extra_body["operatorTag"], "keep");
    assert!(saved.refresh_token.is_none());
    assert!(saved.expires_at.is_none());
    assert!(saved.token_expires_in_secs.is_none());
    assert!(saved.session_auth.as_ref().unwrap().expires_at.is_none());
    let context = saved.kiro_oauth.as_ref().unwrap();
    assert_eq!(context.machine_id(), "b".repeat(64));
    assert_eq!(context.client_id(), "synthetic-console-client");
    assert_eq!(context.client_secret(), Some("synthetic-console-secret"));
    assert_eq!(saved.api_key.as_deref(), Some("synthetic-console-access"));
    mock.stop().await;
}
