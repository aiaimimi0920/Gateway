//! Normal management entry, real callback TCP and original secret/revision publication.
use super::{fixture::*, *};
use crate::console::vertex_oauth::SessionStatus;
use std::collections::HashMap;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

#[tokio::test]
async fn vertex_oauth_management_listener_exchanges_once_and_import_uses_normal_secret_cas() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let mock = issuer().await;
    let view = create(state, &grant, create_body(state), &mock).await;
    let session = session(state, &view);
    let before = OffsetDateTime::now_utc();
    let reply = deliver(&view).await;
    let after = OffsetDateTime::now_utc();
    assert!(reply.starts_with("HTTP/1.1 200 OK"));
    assert!(reply.contains("Cache-Control: no-store"));
    assert!(!reply.contains("synthetic-"));
    assert_eq!(session.view().status, SessionStatus::Ready);
    assert!(state.route_config.snapshot().document().providers[0]
        .credentials
        .is_empty());
    // The browser callback is not a save capability, and a second completion cannot send again.
    assert_eq!(
        action(
            state,
            &view,
            Some(&grant),
            "complete",
            Some(&callback(&view))
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    let lock = state.lifecycle.health_control.publication.lock().await;
    let response = action(state, &view, Some(&grant), "import", None).await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        support::parse_json(response).await
    );
    drop(response);
    assert_eq!(session.view().status, SessionStatus::Importing);
    let id = session.view().target_credential_id.unwrap();
    for name in ["import", "cancel"] {
        assert_eq!(
            support::parse_json(action(state, &view, Some(&grant), name, None).await).await
                ["session"]["status"],
            "importing"
        );
    }
    drop(lock);
    wait_status(&session, SessionStatus::Succeeded).await;
    let repeat =
        support::parse_json(action(state, &view, Some(&grant), "import", None).await).await;
    assert_eq!(repeat["session"]["status"], "succeeded");
    assert_redacted(&repeat);
    let snapshot = state.route_config.snapshot();
    let saved = &snapshot.document().providers[0].credentials[0];
    assert_eq!(saved.id.as_deref(), Some(id.as_str()));
    assert_eq!(saved.api_key.as_deref(), Some("synthetic-new-access"));
    assert_eq!(
        saved.refresh_token.as_deref(),
        Some("synthetic-new-refresh")
    );
    assert_eq!(saved.refresh_endpoint.as_deref(), Some(TOKEN_ENDPOINT));
    assert_eq!(
        saved.refresh_client_id.as_deref(),
        Some("explicit-vertex-client")
    );
    assert_eq!(saved.token_expires_in_secs, Some(3600));
    let context = saved.vertex_oauth.as_ref().unwrap();
    assert_eq!(context.client_secret(), Some("synthetic-private-client"));
    let expiry = OffsetDateTime::parse(saved.expires_at.as_deref().unwrap(), &Rfc3339).unwrap();
    assert!(
        expiry >= before + time::Duration::seconds(3600)
            && expiry <= after + time::Duration::seconds(3600)
    );
    assert_eq!(
        context.refresh_expires_at().unwrap().unwrap() - expiry,
        time::Duration::seconds(3600)
    );
    assert_eq!(context.target().quota_project(), Some("quota-a"));
    assert_eq!(
        snapshot.document().providers[1].api_key,
        "synthetic-other-secret"
    );
    let archived = state
        .route_config_runtime
        .as_ref()
        .unwrap()
        .coordinator()
        .unwrap()
        .load_revision(snapshot.revision().id())
        .unwrap()
        .unwrap();
    assert_eq!(
        archived.document().providers[0].credentials[0]
            .vertex_oauth
            .as_ref()
            .unwrap(),
        context
    );
    assert_redacted(
        &serde_json::to_value(
            crate::console::secrets::redact_route_document(snapshot.document()).unwrap(),
        )
        .unwrap(),
    );
    assert_eq!(mock.count(), 1);
    {
        let calls = mock.calls.lock().unwrap();
        let form: HashMap<_, _> = url::form_urlencoded::parse(calls[0].1.as_bytes())
            .into_owned()
            .collect();
        assert_eq!(form["grant_type"], "authorization_code");
        assert_eq!(form["redirect_uri"], view["redirectUri"].as_str().unwrap());
        assert_eq!(form["code"], "synthetic-code");
        assert_eq!(form["client_id"], "explicit-vertex-client");
        assert_eq!(form["client_secret"], "synthetic-private-client");
        use base64::Engine;
        use sha2::Digest;
        let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(sha2::Sha256::digest(form["code_verifier"].as_bytes()));
        let auth = url::Url::parse(view["authorizationUrl"].as_str().unwrap()).unwrap();
        assert_eq!(
            auth.query_pairs()
                .find(|(k, _)| k == "code_challenge")
                .unwrap()
                .1,
            challenge
        );
        assert!(!calls[0].0.contains_key("authorization"));
        assert!(!calls[0].0.contains_key("cookie"));
    }
    state.lifecycle.vertex_oauth.join().await;
    mock.stop().await;
}

#[tokio::test]
async fn vertex_oauth_management_reauthorization_preserves_id_metadata_and_clears_old_client_secret(
) {
    let mut doc = document();
    doc.providers[0].credentials.push(serde_json::from_value(json!({"id":"stable-id","api_key":"synthetic-old-access",
        "refresh_token":"synthetic-old-refresh","enabled":false,"account_name":"operator-label","supported_models":["operator-model"],
        "refresh_endpoint":TOKEN_ENDPOINT,"refresh_client_id":"old-client","vertex_oauth":{"grant_type":"desktop_authorization_code",
            "client_id":"old-client","client_secret":"old-client-secret","requested_scope":CLOUD_PLATFORM_SCOPE,"scope_reported":false,
            "target":{"project":"project-a","location":"global","quota_project":"quota-a"}}})).unwrap());
    let fixture = support::ConsoleStateFixture::new(doc, true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let mock = issuer().await;
    let mut body = create_body(state);
    body["credentialId"] = json!("stable-id");
    body.as_object_mut().unwrap().remove("clientSecret");
    let mut wrong_quota = body.clone();
    wrong_quota["target"]["quota_project"] = json!("different-quota");
    assert_eq!(
        call(state, ROOT, wrong_quota, Some(&grant)).await.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(mock.count(), 0);
    let view = create(state, &grant, body, &mock).await;
    let session = session(state, &view);
    complete(state, &view, &grant, &mock).await;
    assert_eq!(
        action(state, &view, Some(&grant), "import", None)
            .await
            .status(),
        StatusCode::OK
    );
    wait_status(&session, SessionStatus::Succeeded).await;
    let snapshot = state.route_config.snapshot();
    let accounts = &snapshot.document().providers[0].credentials;
    assert_eq!(accounts.len(), 1);
    let saved = &accounts[0];
    assert_eq!(saved.id.as_deref(), Some("stable-id"));
    assert_eq!(saved.enabled, Some(false));
    assert_eq!(saved.account_name.as_deref(), Some("operator-label"));
    assert_eq!(saved.supported_models, ["operator-model"]);
    assert_eq!(saved.vertex_oauth.as_ref().unwrap().client_secret(), None);
    assert_eq!(saved.api_key.as_deref(), Some("synthetic-new-access"));
    assert_eq!(
        saved.refresh_token.as_deref(),
        Some("synthetic-new-refresh")
    );
    assert!(!mock.calls.lock().unwrap()[0].1.contains("client_secret"));
    state.lifecycle.vertex_oauth.join().await;
    mock.stop().await;
}
