//! A code/state never bypasses operator, Secret Grant, product, full target or revision checks.
use super::{fixture::*, *};
use crate::{
    console::vertex_oauth::SessionStatus, http::router::build_router,
    protocol::vertex_oauth::transport_test_support::test_endpoint,
};
use axum::{body::Body, extract::ConnectInfo, http::Request};
use tower::ServiceExt;

#[tokio::test]
async fn vertex_oauth_management_new_account_rejects_raw_parent_quota_before_consuming_code() {
    let mut doc = document();
    doc.providers[0]
        .headers
        .insert("x-goog-user-project".into(), "raw-parent-quota".into());
    let fixture = support::ConsoleStateFixture::new(doc, true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let mock = issuer().await;
    let response = test_endpoint(
        mock.endpoint.clone(),
        call(state, ROOT, create_body(state), Some(&grant)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(mock.count(), 0);
    state.lifecycle.vertex_oauth.join().await;
    mock.stop().await;
}

#[tokio::test]
async fn vertex_oauth_management_owner_host_ip_origin_and_grant_are_independent_of_callback() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let mock = issuer().await;
    let view = create(state, &grant, create_body(state), &mock).await;
    let session = session(state, &view);
    let path = format!("{ROOT}/{}", session.id());
    let response =
        support::send_console_json(state, Method::GET, &path, json!(null), None, None).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    for (name, value) in [
        ("host", "different.invalid"),
        ("x-management-token", "synthetic-code"),
    ] {
        let mut request = Request::get(&path);
        if name != "x-management-token" {
            request = request.header("x-management-token", support::MANAGEMENT_TOKEN);
        }
        let response = build_router(state.clone())
            .oneshot(request.header(name, value).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_ne!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["cache-control"], "no-store");
    }
    let mut request = Request::get(&path)
        .header("x-management-token", support::MANAGEMENT_TOKEN)
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(
        "127.0.0.2:34567".parse::<std::net::SocketAddr>().unwrap(),
    ));
    assert_ne!(
        build_router(state.clone())
            .oneshot(request)
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    for name in ["complete", "import", "cancel"] {
        assert_eq!(
            action(state, &view, None, name, None).await.status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            action(state, &view, Some("wrong-grant"), name, None)
                .await
                .status(),
            StatusCode::FORBIDDEN
        );
        let response = support::send_console_json(
            state,
            Method::POST,
            &format!("{path}/{name}"),
            json!({"expectedRevision":view["expectedRevision"]}),
            Some(&grant),
            Some("https://untrusted.invalid"),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let mut wrong = view.clone();
        wrong["expectedRevision"] = json!("wrong-revision");
        assert_eq!(
            action(state, &wrong, Some(&grant), name, None)
                .await
                .status(),
            StatusCode::CONFLICT
        );
    }
    assert_eq!(session.view().status, SessionStatus::WaitingUser);
    assert_eq!(mock.count(), 0);
    state.lifecycle.vertex_oauth.join().await;
    mock.stop().await;
}

#[tokio::test]
async fn vertex_oauth_management_invalid_client_product_target_and_unknown_fields_reject_before_exchange(
) {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let mock = issuer().await;
    for (field, value) in [
        ("providerId", json!("unrelated")),
        ("credentialId", json!("missing")),
        ("clientId", json!("")),
        ("clientSecret", json!("bad\nsecret")),
        ("tokenEndpoint", json!("http://127.0.0.1:1")),
        (
            "target",
            json!({"project":"project-b","location":"global","quota_project":"quota-a"}),
        ),
        (
            "target",
            json!({"project":"project-a","location":"us","quota_project":"quota-a"}),
        ),
        (
            "target",
            json!({"project":"project-a","location":"global","baseUrl":"https://untrusted.invalid"}),
        ),
    ] {
        let mut body = create_body(state);
        body[field] = value;
        let response =
            test_endpoint(mock.endpoint.clone(), call(state, ROOT, body, Some(&grant))).await;
        assert!(
            matches!(
                response.status(),
                StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY
            ),
            "{field}"
        );
        assert_eq!(response.headers()["cache-control"], "no-store");
    }
    assert_eq!(mock.count(), 0);
    state.lifecycle.vertex_oauth.join().await;
    mock.stop().await;
    let readonly = support::ConsoleStateFixture::new(document(), false);
    let grant = support::grant_console_secret_access(&readonly.state).await;
    assert_eq!(
        call(
            &readonly.state,
            ROOT,
            create_body(&readonly.state),
            Some(&grant)
        )
        .await
        .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[tokio::test]
async fn vertex_oauth_management_stale_target_blocks_manual_and_listener_exchange_and_ready_save() {
    for listener in [false, true] {
        let fixture = support::ConsoleStateFixture::new(document(), true);
        let state = &fixture.state;
        let grant = support::grant_console_secret_access(state).await;
        let mock = issuer().await;
        let view = create(state, &grant, create_body(state), &mock).await;
        let mut changed = document();
        changed.providers[1].default_model = Some("new-model".into());
        fixture.commit_route_document(changed, None).await;
        if listener {
            assert!(deliver(&view).await.starts_with("HTTP/1.1 400"));
        } else {
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
                StatusCode::CONFLICT
            );
        }
        assert_eq!(mock.count(), 0);
        if !listener {
            assert_eq!(
                action(state, &view, Some(&grant), "cancel", None)
                    .await
                    .status(),
                StatusCode::OK
            );
        }
        state.lifecycle.vertex_oauth.join().await;
        mock.stop().await;
    }
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let mock = issuer().await;
    let view = create(state, &grant, create_body(state), &mock).await;
    let session = session(state, &view);
    complete(state, &view, &grant, &mock).await;
    let mut changed = document();
    changed.providers[1].default_model = Some("winner".into());
    fixture.commit_route_document(changed, None).await;
    assert_eq!(
        action(state, &view, Some(&grant), "import", None)
            .await
            .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(session.view().status, SessionStatus::Ready);
    assert!(state.route_config.snapshot().document().providers[0]
        .credentials
        .is_empty());
    state.lifecycle.vertex_oauth.join().await;
    mock.stop().await;
}

#[tokio::test]
async fn vertex_oauth_management_invalid_callback_consumes_attempt_and_missing_refresh_cannot_be_saved(
) {
    for case in ["state", "duplicate", "denied", "missing-refresh"] {
        let fixture = support::ConsoleStateFixture::new(document(), true);
        let state = &fixture.state;
        let grant = support::grant_console_secret_access(state).await;
        let mut tokens = tokens();
        if case == "missing-refresh" {
            tokens.as_object_mut().unwrap().remove("refresh_token");
        }
        let mock = Mock::new(Reply {
            body: tokens.to_string(),
            ..Reply::default()
        })
        .await;
        let view = create(state, &grant, create_body(state), &mock).await;
        let session = session(state, &view);
        let mut cb = callback(&view);
        match case {
            "state" => cb.push('x'),
            "duplicate" => cb.push_str("&code=other"),
            "denied" => cb = cb.replace("code=synthetic-code", "error=access_denied"),
            _ => (),
        }
        let response = test_endpoint(
            mock.endpoint.clone(),
            action(state, &view, Some(&grant), "complete", Some(&cb)),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(session.view().status, SessionStatus::Failed);
        assert!(session.view().authorization_url.is_none());
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
        assert_eq!(
            action(state, &view, Some(&grant), "import", None)
                .await
                .status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(mock.count(), usize::from(case == "missing-refresh"));
        state.lifecycle.vertex_oauth.join().await;
        let url = url::Url::parse(view["redirectUri"].as_str().unwrap()).unwrap();
        assert!(tokio::net::TcpStream::connect((
            std::net::Ipv4Addr::LOCALHOST,
            url.port().unwrap()
        ))
        .await
        .is_err());
        mock.stop().await;
    }
}
