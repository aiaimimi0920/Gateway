//! Rejected admission retains material; accepted ambiguity is never an automatic resubmission.
use super::*;

#[tokio::test]
async fn actions_require_secret_grant_original_revision_and_same_management_owner() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let view = create(state, &grant, None, Some("synthetic-device")).await;
    let session = session(state, &view);
    for name in ["complete", "import", "cancel"] {
        assert_eq!(
            action(state, &view, None, name, Some(&callback(&view)))
                .await
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    let status = build_router(state.clone())
        .oneshot(
            Request::get(format!("{ROOT}/{}", session.id()))
                .header("x-management-token", support::MANAGEMENT_TOKEN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status.status(), StatusCode::OK);
    assert_eq!(status.headers()["cache-control"], "no-store");
    let foreign = build_router(state.clone())
        .oneshot(
            Request::get(format!("{ROOT}/{}", session.id()))
                .header("x-management-token", support::MANAGEMENT_TOKEN)
                .header("host", "foreign.invalid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(foreign.status(), StatusCode::OK);
    let mut wrong = view.clone();
    wrong["expectedRevision"] = json!("wrong-revision");
    assert_eq!(
        action(state, &wrong, Some(&grant), "cancel", None)
            .await
            .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(session.view().status, SessionStatus::WaitingUser);
    let issuer = Issuer::new(true, EXPIRY, None).await;
    complete(state, &view, &grant, &issuer).await;
    let original = state.route_config.snapshot().revision().id().to_owned();
    let mut changed = document();
    changed.providers[0].label = Some("updated-by-operator".into());
    state
        .route_config_runtime
        .as_ref()
        .unwrap()
        .commit_document(&original, changed, None)
        .await
        .unwrap();
    let response = action(state, &view, Some(&grant), "import", None).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(session.view().status, SessionStatus::Ready);
    assert_eq!(issuer.hits.load(std::sync::atomic::Ordering::SeqCst), 1);
    state.lifecycle.accio_oauth.join().await;
}

#[tokio::test]
async fn unknown_fields_foreign_product_overrides_and_missing_or_ambiguous_device_are_rejected() {
    for variant in [
        "missing-device",
        "ambiguous-device",
        "foreign-preset",
        "foreign-base",
        "foreign-path",
        "foreign-credential-base",
        "unknown-field",
        "no-such-id",
    ] {
        let mut doc = document();
        if variant == "foreign-preset" {
            doc.providers[0].preset = Some("xai-api".into());
            doc.providers[0].base_url = "https://api.x.ai/v1".into();
        }
        if variant == "foreign-base" {
            doc.providers[0].base_url = "https://foreign.invalid".into();
        }
        if variant == "foreign-path" {
            doc.providers[0].responses_path = Some("/foreign/path".into());
        }
        if matches!(variant, "ambiguous-device" | "foreign-credential-base") {
            let mut credential = json!({"id":"stable-id","api_key":"synthetic-old-access","headers":{"utdid":"one-device","x-utdid":"other-device"}});
            if variant == "foreign-credential-base" {
                credential["base_url"] = json!("https://foreign.invalid");
            }
            doc.providers[0]
                .credentials
                .push(serde_json::from_value(credential).unwrap());
        }
        let fixture = support::ConsoleStateFixture::new(doc, true);
        let state = &fixture.state;
        let grant = support::grant_console_secret_access(state).await;
        let mut body = json!({"providerId":"saved-pool","expectedRevision":state.route_config.snapshot().revision().id(),
            "clientId":"synthetic-explicit-client","loginSite":"china","callbackPort":34567});
        if variant != "missing-device" && variant != "ambiguous-device" {
            body["utdid"] = json!("synthetic-device");
        }
        if matches!(
            variant,
            "ambiguous-device" | "foreign-credential-base" | "no-such-id"
        ) {
            body["credentialId"] = json!("stable-id");
        }
        if variant == "unknown-field" {
            body["tokenEndpoint"] = json!("http://127.0.0.1:1");
        }
        let response =
            support::send_console_json(state, Method::POST, ROOT, body, Some(&grant), None).await;
        assert!(
            response.status().is_client_error(),
            "{variant}: {}",
            response.status()
        );
        state.lifecycle.accio_oauth.join().await;
    }
}

#[tokio::test]
async fn cancel_inflight_exchange_discards_late_material_and_dropping_waiter_fails_closed() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let pause = Arc::new(tokio::sync::Notify::new());
    let issuer = Issuer::new(true, EXPIRY, Some(pause.clone())).await;
    for cancelled in [true, false] {
        let view = create(state, &grant, None, Some("synthetic-device")).await;
        let session = session(state, &view);
        let hits = issuer.hits.load(std::sync::atomic::Ordering::SeqCst);
        let callback = callback(&view);
        let mut future = Box::pin(crate::protocol::accio_oauth::TEST_TOKEN_ENDPOINT.scope(
            issuer.endpoint.clone(),
            action(state, &view, Some(&grant), "complete", Some(&callback)),
        ));
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop { tokio::select! { _=&mut future=>panic!("exchange ended before controlled cancellation"), _=tokio::task::yield_now()=>{} }
                if issuer.hits.load(std::sync::atomic::Ordering::SeqCst) > hits { break; }
            }
        }).await.unwrap();
        if cancelled {
            assert_eq!(
                action(state, &view, Some(&grant), "cancel", None)
                    .await
                    .status(),
                StatusCode::OK
            );
            assert_eq!(future.await.status(), StatusCode::BAD_REQUEST);
            assert_eq!(session.view().status, SessionStatus::Cancelled);
        } else {
            drop(future);
            assert_eq!(session.view().status, SessionStatus::Failed);
        }
        pause.notify_one();
        assert!(session.view().credential_id.is_none());
    }
    assert!(state.route_config.snapshot().document().providers[0]
        .credentials
        .is_empty());
    state.lifecycle.accio_oauth.join().await;
}

#[tokio::test]
async fn accepted_save_uses_original_cas_and_reports_conflict_as_unconfirmed_without_second_commit()
{
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let view = create(state, &grant, None, Some("synthetic-device")).await;
    let session = session(state, &view);
    let issuer = Issuer::new(true, EXPIRY, None).await;
    complete(state, &view, &grant, &issuer).await;
    let lock = state.lifecycle.health_control.publication.lock().await;
    assert_eq!(
        action(state, &view, Some(&grant), "import", None)
            .await
            .status(),
        StatusCode::OK
    );
    let mut changed = document();
    changed.providers[0].label = Some("newer-revision".into());
    let revision = state
        .route_config_runtime
        .as_ref()
        .unwrap()
        .commit_document(view["expectedRevision"].as_str().unwrap(), changed, None)
        .await
        .unwrap()
        .revision()
        .id()
        .to_owned();
    drop(lock);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while session.view().status == SessionStatus::Importing {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(session.view().status, SessionStatus::SaveUnconfirmed);
    assert!(session.view().credential_id.is_none());
    assert!(session.view().saved_revision.is_none());
    assert!(session.view().target_credential_id.is_some());
    for _ in 0..2 {
        assert_eq!(
            support::parse_json(action(state, &view, Some(&grant), "import", None).await).await
                ["session"]["status"],
            "save_unconfirmed"
        );
    }
    assert_eq!(state.route_config.snapshot().revision().id(), revision);
    assert!(state.route_config.snapshot().document().providers[0]
        .credentials
        .is_empty());
    state.lifecycle.accio_oauth.join().await;
}

#[tokio::test]
async fn unknown_auxiliary_secret_is_not_silently_reused_or_cleared_when_refresh_context_changes() {
    let mut doc = document();
    doc.providers[0].credentials.push(serde_json::from_value(json!({"id":"stable-id","api_key":"synthetic-old-access",
        "refresh_endpoint":"https://phoenix-gw.alibaba.com/api/auth/refresh_token","refresh_client_id":"old-client",
        "headers":{"utdid":"synthetic-device","X-Custom-Token":"synthetic-unknown-auxiliary"}})).unwrap());
    let fixture = support::ConsoleStateFixture::new(doc, true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let original = state.route_config.snapshot().revision().id().to_owned();
    let view = create(state, &grant, Some("stable-id"), None).await;
    let session = session(state, &view);
    let issuer = Issuer::new(true, EXPIRY, None).await;
    complete(state, &view, &grant, &issuer).await;
    let response = action(state, &view, Some(&grant), "import", None).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        support::parse_json(response).await["error"]["code"],
        "accio_login_save_rejected"
    );
    assert_eq!(session.view().status, SessionStatus::Ready);
    assert_eq!(
        action(state, &view, Some(&grant), "import", None)
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(session.view().status, SessionStatus::Ready);
    let snapshot = state.route_config.snapshot();
    assert_eq!(snapshot.revision().id(), original);
    assert_eq!(
        snapshot.document().providers[0].credentials[0].headers["X-Custom-Token"],
        "synthetic-unknown-auxiliary"
    );
    state.lifecycle.accio_oauth.join().await;
}
