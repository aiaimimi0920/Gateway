//! Admission, binding and revision failures cannot become OIDC sends or extra durable credentials.
use super::*;

#[tokio::test]
async fn management_owner_origin_and_grant_are_separate_from_the_device_challenge() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let mock = Mock::new(vec![registration(), device()]).await;
    let (view, session) = create(state, &grant, create_body(state), &mock).await;
    let response = build_router(state.clone())
        .oneshot(
            Request::get(format!("{ROOT}/{}", session.id()))
                .header("x-management-token", support::MANAGEMENT_TOKEN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(
        support::parse_json(response).await["session"]["userCode"],
        "ABCD-EFGH"
    );
    let mut request = Request::get(format!("{ROOT}/{}", session.id()))
        .header("x-management-token", support::MANAGEMENT_TOKEN)
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(
        "127.0.0.2:34567".parse::<SocketAddr>().unwrap(),
    ));
    assert_ne!(
        build_router(state.clone())
            .oneshot(request)
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    for name in ["poll", "cancel", "import"] {
        assert_eq!(
            action(state, &view, None, name).await.status(),
            StatusCode::FORBIDDEN
        );
    }
    for (name, value) in [
        ("host", "different.example"),
        ("x-management-token", "ABCD-EFGH"),
    ] {
        let mut request = Request::get(format!("{ROOT}/{}", session.id()));
        if name != "x-management-token" {
            request = request.header("x-management-token", support::MANAGEMENT_TOKEN);
        }
        let response = build_router(state.clone())
            .oneshot(request.header(name, value).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_ne!(response.status(), StatusCode::OK);
    }
    let response = support::send_console_json(
        state,
        Method::POST,
        &format!("{ROOT}/{}/poll", session.id()),
        json!({"expectedRevision":view["expectedRevision"]}),
        Some(&grant),
        Some("https://untrusted.example"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let response = action(state, &view, Some(&grant), "cancel").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(session.view().status, SessionStatus::Cancelled);
    assert_eq!(mock.count(), 2);
    state.lifecycle.kiro_oauth.join().await;
    mock.stop().await;
}

#[tokio::test]
async fn invalid_targets_machine_scope_directory_and_unknown_fields_are_rejected_before_network() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let mock = Mock::new(vec![]).await;
    let baseline = create_body(state);
    for (case, (field, value)) in [
        ("machineId", json!("invalid")),
        ("scopes", json!([])),
        (
            "scopes",
            json!(["codewhisperer:conversations", "codewhisperer:conversations"]),
        ),
        ("directory", json!({"authMethod":"social"})),
        (
            "directory",
            json!({"authMethod":"identity_center","directoryRegion":"invalid","startUrl":"https://example.invalid/start"}),
        ),
        ("providerId", json!("unrelated")),
        ("credentialId", json!("missing")),
        ("tokenEndpoint", json!("https://attacker.invalid/token")),
        (
            "directory",
            json!({"authMethod":"builder_id","startUrl":"https://attacker.invalid/start"}),
        ),
    ].into_iter().enumerate() {
        let mut body = baseline.clone();
        body[field] = value;
        let response = TEST_ORIGIN
            .scope(
                mock.origin.clone(),
                support::send_console_json(state, Method::POST, ROOT, body, Some(&grant), None),
            )
            .await;
        assert!(matches!(
            response.status(),
            StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY
        ), "case={case} field={field} status={}",response.status());
    }
    assert_eq!(mock.count(), 0);
    state.lifecycle.kiro_oauth.join().await;
    mock.stop().await;
    let readonly = support::ConsoleStateFixture::new(document(), false);
    let grant = support::grant_console_secret_access(&readonly.state).await;
    let response = support::send_console_json(
        &readonly.state,
        Method::POST,
        ROOT,
        create_body(&readonly.state),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[test]
fn builder_id_directory_rejects_extra_fields_instead_of_silently_ignoring_them() {
    assert!(serde_json::from_value::<LoginDirectory>(json!({"authMethod":"builder_id"})).is_ok());
    for field in ["startUrl", "directoryRegion", "tokenEndpoint"] {
        let mut value = json!({"authMethod":"builder_id"});
        value[field] = json!("unexpected");
        assert!(
            serde_json::from_value::<LoginDirectory>(value).is_err(),
            "{field}"
        );
    }
}

#[tokio::test]
async fn stale_revision_blocks_poll_and_save_before_new_sends_or_publication() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let mock = Mock::new(vec![registration(), device()]).await;
    let (view, session) = create(state, &grant, create_body(state), &mock).await;
    let mut newer = state.route_config.snapshot().document().clone();
    newer.providers[1].default_model = Some("changed-model".into());
    fixture.commit_route_document(newer, None).await;
    assert_eq!(
        action(state, &view, Some(&grant), "poll").await.status(),
        StatusCode::CONFLICT
    );
    assert_eq!(session.view().status, SessionStatus::WaitingUser);
    assert_eq!(mock.count(), 2);
    state.lifecycle.kiro_oauth.join().await;
    mock.stop().await;
}

#[tokio::test]
async fn admitted_save_revision_conflict_remains_unconfirmed_and_never_reopens_material() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let mock = Mock::new(vec![registration(), device(), tokens(true, true)]).await;
    let (view, session) = create(state, &grant, create_body(state), &mock).await;
    ready(state, &view, &session, &grant, &mock).await;
    let lock = state.lifecycle.health_control.publication.lock().await;
    assert_eq!(
        action(state, &view, Some(&grant), "import").await.status(),
        StatusCode::OK
    );
    let target = session.view().target_credential_id.unwrap();
    let mut newer = state.route_config.snapshot().document().clone();
    newer.providers[1].default_model = Some("winner".into());
    fixture.commit_route_document(newer, None).await;
    drop(lock);
    state.lifecycle.kiro_oauth.join().await;
    assert_eq!(session.view().status, SessionStatus::SaveUnconfirmed);
    assert_eq!(
        session.view().target_credential_id.as_deref(),
        Some(target.as_str())
    );
    assert!(session.view().credential_id.is_none());
    assert!(session.view().saved_revision.is_none());
    assert!(state.route_config.snapshot().document().providers[0]
        .credentials
        .is_empty());
    mock.stop().await;
}
