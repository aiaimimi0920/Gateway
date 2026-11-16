use super::{fixture::*, *};
#[tokio::test]
async fn nous_management_explicit_device_poll_save_and_duplicate_import() {
    for welcome in [false, true] {
        let fixture = support::ConsoleStateFixture::new(document(), true);
        let state = &fixture.state;
        let grant = support::grant_console_secret_access(state).await;
        let peer = Peer::new(false, 600).await;
        if welcome {
            peer.reply.lock().unwrap()["inference_base_url"] =
                json!("https://welcome-api.nousresearch.com/v1");
        }
        let view = create(state, &grant, body(state), &peer).await;
        let session = session(state, &view);
        assert!(state
            .lifecycle
            .nous_oauth
            .get_owned(session.id(), "another-owner")
            .is_err());
        assert_eq!(peer.count(), 0);
        assert_eq!(view["deviceAuthorization"]["userCode"], "ABCD");
        let denied = action(state, &view, None, "poll", &peer).await;
        assert_ne!(denied.status(), StatusCode::OK);
        assert_eq!(denied.headers()["cache-control"], "no-store");
        ready(state, &view, &grant, &peer).await;
        assert!(session.view().receipt_received);
        assert!(state.route_config.snapshot().document().providers[0]
            .credentials
            .is_empty());
        assert_ne!(
            action(state, &view, None, "import", &peer).await.status(),
            StatusCode::OK
        );
        assert_eq!(
            action(state, &view, Some(&grant), "import", &peer)
                .await
                .status(),
            StatusCode::OK
        );
        wait_status(&session, SessionStatus::Succeeded).await;
        let saved = state.route_config.snapshot();
        let credential = &saved.document().providers[0].credentials[0];
        assert_eq!(
            credential.api_key.as_deref(),
            Some(access(Some("subject")).as_str())
        );
        assert_eq!(
            credential.refresh_token.as_deref(),
            Some("synthetic-nous-refresh")
        );
        assert_eq!(
            credential.base_url.as_deref(),
            Some(if welcome {
                "https://welcome-api.nousresearch.com/v1"
            } else {
                "https://inference-api.nousresearch.com/v1"
            })
        );
        assert_eq!(
            credential.refresh_client_id.as_deref(),
            Some("permitted-gateway")
        );
        assert!(credential.nous_portal.is_some());
        assert!(credential.token_expires_in_secs.is_none());
        assert_eq!(
            saved.document().providers[1].api_key,
            "synthetic-unrelated-secret"
        );
        let response = action(state, &view, Some(&grant), "import", &peer).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_redacted(&support::parse_json(response).await);
        assert_eq!(
            state.route_config.snapshot().revision().id(),
            saved.revision().id()
        );
        assert_eq!(peer.count(), 1);
        assert_eq!(
            peer.catalog_reads.load(std::sync::atomic::Ordering::SeqCst),
            0
        );
        state.lifecycle.nous_oauth.join().await;
        peer.stop().await;
    }
}
#[tokio::test]
async fn nous_management_reauthorization_preserves_id_metadata_and_requires_known_subject() {
    for subject in [Some("subject"), None, Some("other")] {
        let fixture = support::ConsoleStateFixture::new(document(), true);
        let state = &fixture.state;
        let grant = support::grant_console_secret_access(state).await;
        let peer = Peer::new(false, 600).await;
        let view = create(state, &grant, body(state), &peer).await;
        ready(state, &view, &grant, &peer).await;
        action(state, &view, Some(&grant), "import", &peer).await;
        wait_status(&session(state, &view), SessionStatus::Succeeded).await;
        let before = state.route_config.snapshot();
        let id = before.document().providers[0].credentials[0]
            .id
            .clone()
            .unwrap();
        let mut changed = before.document().clone();
        changed.providers[0].credentials[0].account_name = Some("Operator label".into());
        changed.providers[0].credentials[0].enabled = Some(false);
        state
            .route_config_runtime
            .as_ref()
            .unwrap()
            .commit_document(before.revision().id(), changed, None)
            .await
            .unwrap();
        peer.reply.lock().unwrap()["access_token"] = json!(access(subject));
        let mut input = body(state);
        input["credentialId"] = json!(id);
        let view = create(state, &grant, input, &peer).await;
        ready(state, &view, &grant, &peer).await;
        let response = action(state, &view, Some(&grant), "import", &peer).await;
        if subject == Some("subject") {
            assert_eq!(response.status(), StatusCode::OK);
            wait_status(&session(state, &view), SessionStatus::Succeeded).await;
        } else {
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            assert_eq!(
                support::parse_json(response).await["error"]["code"],
                "nous_login_identity_mismatch"
            );
        }
        let after = state.route_config.snapshot();
        let c = &after.document().providers[0].credentials[0];
        assert_eq!(c.id.as_deref(), Some(id.as_str()));
        assert_eq!(c.enabled, Some(false));
        assert_eq!(c.account_name.as_deref(), Some("Operator label"));
        assert_eq!(c.api_key.as_deref(), Some(access(Some("subject")).as_str()));
        state.lifecycle.nous_oauth.join().await;
        peer.stop().await;
    }
}
