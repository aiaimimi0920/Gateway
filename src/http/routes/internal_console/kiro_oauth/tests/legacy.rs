//! Explicit legacy reauthorization preserves the real prior device; no speculative context migration.
use super::*;

fn legacy_document(extra: Value) -> crate::routing::config::RouteConfigYaml {
    let mut doc = document();
    doc.providers[0].credentials = serde_json::from_value(json!([
        {"id":"legacy-stable","api_key":"synthetic-legacy-access","enabled":false,
            "account_name":"legacy-label","supported_models":["legacy-model"],
            "refresh_token":"synthetic-root-refresh","refresh_endpoint":"https://oidc.us-east-1.amazonaws.com/token",
            "refresh_client_id":"old-client","expires_at":"2099-01-01T00:00:00Z","extra_body":extra},
        {"id":"sibling","api_key":"synthetic-sibling","extra_body":{"operatorTag":"sibling-keep"}}
    ])).unwrap();
    doc
}

fn reauth_body(state: &Arc<AppState>) -> Value {
    let mut body = create_body(state);
    body.as_object_mut().unwrap().remove("machineId");
    body["credentialId"] = json!("legacy-stable");
    body
}

#[tokio::test]
async fn legacy_reauthorization_keeps_uuid_device_id_and_replaces_only_target_auth() {
    let mut doc = legacy_document(json!({
        "machineId":"ABCDEF01-2345-6789-ABCD-EF0123456789", "operatorTag":"keep",
        "kiroRefreshToken":"synthetic-extra-refresh", "kiroClientId":"old-client",
        "kiroClientSecret":"synthetic-old-client-secret","kiroAuthMethod":"social",
        "clientSecretExpiresAt":4102444800i64,"scopes":["codewhisperer:conversations"],
        "rawSource":{"accessToken":"synthetic-archive-access","entries":[{"refreshToken":"synthetic-archive-refresh"}]}}));
    doc.providers[0]
        .extra_body
        .insert("kiroVersion".into(), json!("0.7.1"));
    let fixture = support::ConsoleStateFixture::new(doc, true);
    let state = &fixture.state;
    // The fixture's bootstrap is only an in-memory snapshot. Establish a normal
    // archived revision before testing that reauthorization preserves its bytes.
    let mut persisted = state.route_config.snapshot().document().clone();
    persisted.providers[1].default_model = Some("synthetic-persisted-model".into());
    fixture.commit_route_document(persisted, None).await;
    let before = state.route_config.snapshot();
    let previous = serde_json::to_value(before.document()).unwrap();
    let old_headers = crate::upstream::headers::build_upstream_headers(
        &before.get_providers()[0].credential_pool[0].payload,
    );
    let grant = support::grant_console_secret_access(state).await;
    let mock = Mock::new(vec![registration(), device(), tokens(false, false)]).await;
    let mut wrong_machine = reauth_body(state);
    wrong_machine["machineId"] = json!("c".repeat(64));
    assert_eq!(
        support::send_console_json(state, Method::POST, ROOT, wrong_machine, Some(&grant), None)
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(mock.count(), 0);
    let (view, session) = create(state, &grant, reauth_body(state), &mock).await;
    ready(state, &view, &session, &grant, &mock).await;
    assert_eq!(
        serde_json::to_value(state.route_config.snapshot().document()).unwrap(),
        previous
    );
    let response = action(state, &view, Some(&grant), "import").await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        support::parse_json(response).await
    );
    state.lifecycle.kiro_oauth.join().await;
    assert_eq!(session.view().status, SessionStatus::Succeeded);
    let saved = state.route_config.snapshot();
    let target = &saved.document().providers[0].credentials[0];
    assert_eq!(target.id.as_deref(), Some("legacy-stable"));
    assert_eq!(target.enabled, Some(false));
    assert_eq!(target.account_name.as_deref(), Some("legacy-label"));
    assert_eq!(target.supported_models, ["legacy-model"]);
    assert_eq!(target.extra_body["operatorTag"], "keep");
    assert_eq!(target.api_key.as_deref(), Some("synthetic-console-access"));
    assert!(target.refresh_token.is_none() && target.expires_at.is_none());
    assert!(target
        .extra_body
        .keys()
        .all(|key| !crate::protocol::kiro_oauth::business::auth_extra(key)));
    assert!(target.extra_body.get("clientSecretExpiresAt").is_none());
    assert!(target.extra_body.get("scopes").is_none());
    assert_eq!(
        target.kiro_oauth.as_ref().unwrap().machine_id(),
        "abcdef0123456789abcdef0123456789".repeat(2)
    );
    let new_headers = crate::upstream::headers::build_upstream_headers(
        &saved.get_providers()[0].credential_pool[0].payload,
    );
    assert_eq!(old_headers["user-agent"], new_headers["user-agent"]);
    let current = serde_json::to_value(saved.document()).unwrap();
    assert_eq!(
        current["providers"][0]["credentials"][1],
        previous["providers"][0]["credentials"][1]
    );
    assert_eq!(current["providers"][1], previous["providers"][1]);
    let archive = state
        .route_config_runtime
        .as_ref()
        .unwrap()
        .coordinator()
        .unwrap()
        .load_revision(before.revision().id())
        .unwrap()
        .unwrap();
    assert_eq!(serde_json::to_value(archive.document()).unwrap(), previous);
    mock.stop().await;
}

#[tokio::test]
async fn legacy_reauthorization_freezes_effective_extra_refresh_hash_and_root_only_zero_device() {
    for extra in [
        json!({"kiroRefreshToken":"synthetic-extra-refresh"}),
        json!({}),
    ] {
        let fixture = support::ConsoleStateFixture::new(legacy_document(extra.clone()), true);
        let state = &fixture.state;
        let expected =
            crate::protocol::kiro::generate_machine_id(None, extra["kiroRefreshToken"].as_str())
                .unwrap_or_else(|| "0".repeat(64));
        let grant = support::grant_console_secret_access(state).await;
        let mock = Mock::new(vec![registration(), device(), tokens(true, true)]).await;
        let (view, session) = create(state, &grant, reauth_body(state), &mock).await;
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
        let saved = state.route_config.snapshot();
        let target = &saved.document().providers[0].credentials[0];
        assert_eq!(target.kiro_oauth.as_ref().unwrap().machine_id(), expected);
        assert_eq!(
            target.refresh_token.as_deref(),
            Some("synthetic-console-refresh")
        );
        assert_eq!(target.extra_body["kiroMachineId"], expected);
        mock.stop().await;
    }
}

#[tokio::test]
async fn legacy_reauthorization_rejects_ambiguous_machine_and_inherited_auth_before_network() {
    for (extra, inherited) in [
        (
            json!({"machineId":"a".repeat(64),"kiroMachineId":"b".repeat(64)}),
            json!({}),
        ),
        (
            json!({"kiroRefreshToken":"one","refreshToken":"two"}),
            json!({}),
        ),
        (json!({"machineId":"invalid"}), json!({})),
        (
            json!({}),
            json!({"kiroClientSecret":"synthetic-shared-secret"}),
        ),
        (json!({"kiroApiRegion":"eu-west-1"}), json!({})),
    ] {
        let mut doc = legacy_document(extra);
        doc.providers[0].extra_body = serde_json::from_value(inherited).unwrap();
        let fixture = support::ConsoleStateFixture::new(doc, true);
        let state = &fixture.state;
        let before = serde_json::to_value(state.route_config.snapshot().document()).unwrap();
        let grant = support::grant_console_secret_access(state).await;
        let mock = Mock::new(vec![]).await;
        let response = TEST_ORIGIN
            .scope(
                mock.origin.clone(),
                support::send_console_json(
                    state,
                    Method::POST,
                    ROOT,
                    reauth_body(state),
                    Some(&grant),
                    None,
                ),
            )
            .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(mock.count(), 0);
        assert_eq!(
            serde_json::to_value(state.route_config.snapshot().document()).unwrap(),
            before
        );
        state.lifecycle.kiro_oauth.join().await;
        mock.stop().await;
    }
}

#[tokio::test]
async fn legacy_authorization_cancel_does_not_upgrade_or_replace_old_material() {
    let fixture = support::ConsoleStateFixture::new(legacy_document(json!({})), true);
    let state = &fixture.state;
    let before = serde_json::to_value(state.route_config.snapshot().document()).unwrap();
    let grant = support::grant_console_secret_access(state).await;
    let mock = Mock::new(vec![registration(), device()]).await;
    let (view, session) = create(state, &grant, reauth_body(state), &mock).await;
    assert_eq!(
        action(state, &view, Some(&grant), "cancel").await.status(),
        StatusCode::OK
    );
    state.lifecycle.kiro_oauth.join().await;
    assert_eq!(session.view().status, SessionStatus::Cancelled);
    assert_eq!(
        serde_json::to_value(state.route_config.snapshot().document()).unwrap(),
        before
    );
    mock.stop().await;
}

#[tokio::test]
async fn legacy_reauthorization_normalizes_matching_inherited_and_credential_machine_aliases() {
    let uuid = "ABCDEF01-2345-6789-ABCD-EF0123456789";
    let machine = crate::protocol::kiro::normalize_machine_id(uuid).unwrap();
    let mut doc = legacy_document(json!({"machineId":machine.to_uppercase()}));
    doc.providers[0]
        .extra_body
        .insert("kiroMachineId".into(), json!(uuid));
    let fixture = support::ConsoleStateFixture::new(doc, true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let mock = Mock::new(vec![registration(), device(), tokens(true, true)]).await;
    let (view, session) = create(state, &grant, reauth_body(state), &mock).await;
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
    let saved = state.route_config.snapshot();
    assert_eq!(
        saved.document().providers[0].extra_body["kiroMachineId"],
        uuid
    );
    let target = &saved.document().providers[0].credentials[0];
    assert_eq!(target.extra_body["kiroMachineId"], machine);
    assert_eq!(target.extra_body["machineId"], machine);
    mock.stop().await;
}
