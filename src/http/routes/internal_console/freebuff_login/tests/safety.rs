use super::*;

#[tokio::test]
async fn freebuff_management_requires_authority_grant_origin_and_no_store_even_on_extractor_failure(
) {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let mock = Mock::new(vec![]).await;
    for (token, grant_value, origin, body, content_type) in [
        (
            None,
            None,
            None,
            create_body(state).to_string(),
            "application/json",
        ),
        (
            Some(support::MANAGEMENT_TOKEN),
            None,
            None,
            create_body(state).to_string(),
            "application/json",
        ),
        (
            Some(support::MANAGEMENT_TOKEN),
            Some(grant.as_str()),
            Some("https://evil.invalid"),
            create_body(state).to_string(),
            "application/json",
        ),
        (
            Some(support::MANAGEMENT_TOKEN),
            Some(grant.as_str()),
            None,
            "{".into(),
            "application/json",
        ),
        (
            Some(support::MANAGEMENT_TOKEN),
            Some(grant.as_str()),
            None,
            "a".repeat(17 * 1024),
            "application/json",
        ),
        (
            Some(support::MANAGEMENT_TOKEN),
            Some(grant.as_str()),
            None,
            "{}".into(),
            "text/plain",
        ),
    ] {
        let mut request = Request::post(ROOT).header("content-type", content_type);
        if let Some(token) = token {
            request = request.header("x-management-token", token);
        }
        if let Some(grant) = grant_value {
            request = request.header("x-secret-grant", grant);
        }
        if let Some(origin) = origin {
            request = request.header("origin", origin);
        }
        let response = TEST_ORIGIN
            .scope(
                mock.origin.clone(),
                build_router(state.clone()).oneshot(request.body(Body::from(body)).unwrap()),
            )
            .await
            .unwrap();
        assert!(response.status().is_client_error());
        assert_eq!(response.headers()["cache-control"], "no-store");
    }
    assert_eq!(mock.count(), 0);
    state.lifecycle.freebuff_login.join().await;
    mock.stop().await;
}

#[tokio::test]
async fn freebuff_management_owns_session_and_revalidates_revision_before_any_poll_or_save() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let mock = Mock::new(vec![code()]).await;
    let (view, session) = create(state, &grant, create_body(state), &mock).await;
    let response = build_router(state.clone())
        .oneshot(
            Request::get(format!("{ROOT}/{}", session.id()))
                .header("x-management-token", support::MANAGEMENT_TOKEN)
                .header("host", "other-host.invalid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert!(action(state, &view, None, "poll")
        .await
        .status()
        .is_client_error());
    let mut changed = state.route_config.snapshot().document().clone();
    changed.providers[0].label = Some("edited".into());
    fixture
        .commit_route_document(changed, Some("concurrent edit"))
        .await;
    let response = TEST_ORIGIN
        .scope(
            mock.origin.clone(),
            action(state, &view, Some(&grant), "poll"),
        )
        .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert!(action(state, &view, Some(&grant), "import")
        .await
        .status()
        .is_client_error());
    assert_eq!(
        action(state, &view, Some(&grant), "cancel").await.status(),
        StatusCode::OK
    );
    assert_eq!(session.view().status, SessionStatus::Cancelled);
    state.lifecycle.freebuff_login.join().await;
    assert_eq!(mock.count(), 1);
    mock.stop().await;
}

#[tokio::test]
async fn freebuff_management_rejects_custom_business_origin_paths_and_shared_auth_before_sending() {
    for (field, value) in [
        ("base_url", json!("https://www.codebuff.com")),
        ("base_url", json!("https://freebuff.com/other")),
        ("chat_completions_path", json!("/custom")),
        (
            "extra_body",
            json!({"freebuffSessionPath":"https://evil.invalid"}),
        ),
        ("api_key", json!("shared-secret")),
        ("headers", json!({"authorization":"Bearer other"})),
    ] {
        let mut document = serde_json::to_value(document()).unwrap();
        document["providers"][0][field] = value;
        let fixture =
            support::ConsoleStateFixture::new(serde_json::from_value(document).unwrap(), true);
        let state = &fixture.state;
        let grant = support::grant_console_secret_access(state).await;
        let mock = Mock::new(vec![]).await;
        let response = TEST_ORIGIN
            .scope(
                mock.origin.clone(),
                support::send_console_json(
                    state,
                    Method::POST,
                    ROOT,
                    create_body(state),
                    Some(&grant),
                    None,
                ),
            )
            .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{field}");
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(mock.count(), 0);
        state.lifecycle.freebuff_login.join().await;
        mock.stop().await;
    }
}
