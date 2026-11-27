use super::*;
use std::sync::atomic::Ordering;

#[tokio::test]
async fn browser_and_manual_cursor_import_are_durable_redacted_and_never_routable() {
    for manual in [false, true] {
        let fixture = Fixture::new().await;
        let peer = Peer::new(false).await;
        peer.identity.lock()["email"] = json!(" Discovered@example.test ");
        let input = fixture.body(manual);
        assert!(input.get("expectedEmail").is_none());
        let initial = fixture.create(&peer, input.clone()).await;
        assert_eq!(initial["authorizationUrl"].is_string(), !manual);
        let receipt = fixture
            .wait(input["attemptId"].as_str().unwrap(), "succeeded")
            .await;
        assert_eq!(receipt["authReceived"], true);
        assert_eq!(receipt["accountMatched"], true);
        assert_eq!(receipt["credentialPersisted"], true);
        assert_eq!(receipt["inferenceStatus"], "not_checked");
        assert_eq!(peer.polls.load(Ordering::SeqCst), usize::from(!manual));
        assert_eq!(peer.checks.load(Ordering::SeqCst), 1);
        // Same attempt remains idempotent even after its original revision has advanced.
        let duplicate = fixture.create(&peer, input.clone()).await;
        assert_eq!(duplicate["savedRevision"], receipt["savedRevision"]);
        assert_eq!(peer.checks.load(Ordering::SeqCst), 1);
        let snapshot = fixture.state.route_config.snapshot();
        let id = receipt["targetCredentialId"].as_str().unwrap();
        let credential = &snapshot.document().providers[0].credentials[0];
        assert_eq!(credential.id.as_deref(), Some(id));
        assert_eq!(
            credential.account_name.as_deref(),
            Some("discovered@example.test")
        );
        assert_eq!(credential.auth_token.as_deref(), Some(access().as_str()));
        assert_eq!(
            snapshot.document().providers[1].api_key.as_str(),
            "synthetic-unrelated-secret"
        );
        assert!(snapshot.select_credential_probe_target(id).is_none());
        assert!(snapshot
            .credential_probe_targets_for_provider("cursor")
            .unwrap()
            .is_empty());
        assert!(snapshot.scheduled_credential_probe_targets().is_empty());
        let compiled = &snapshot.get_providers()[0];
        assert!(compiled.payload.auth_token.is_none());
        assert!(!compiled.credential_pool[0].enabled);
        assert!(compiled.credential_pool[0].payload.api_key.is_empty());
        assert!(compiled.credential_pool[0].payload.auth_token.is_none());
        assert!(snapshot.list_models().is_empty());
        for model in [
            None,
            Some("default"),
            Some("cursor-personal"),
            Some("anything"),
        ] {
            assert!(snapshot
                .resolve_candidates(model)
                .iter()
                .all(|candidate| candidate.provider_account_id != "cursor"));
        }
        let response = fixture
            .call(
                Method::GET,
                "/v1/internal/gateway/console/route-config",
                Value::Null,
                None,
            )
            .await;
        let redacted = support::parse_json(response).await;
        assert!(!redacted.to_string().contains(&access()));
        assert!(redacted["routeConfig"]["secrets"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |s| s["path"] == "/providers/0/credentials/0/auth_token" && s["configured"] == true
            ));
        assert!(!receipt.to_string().contains(&access()));
        assert!(!receipt.to_string().contains("discovered@example.test"));
        fixture.close().await;
        let reopened = RouteConfigRuntime::new_local(&fixture.console).unwrap();
        reopened.recover_startup().await.unwrap();
        let recovered = reopened.route_config().snapshot();
        assert_eq!(
            recovered.revision().id(),
            receipt["savedRevision"].as_str().unwrap()
        );
        assert_eq!(
            recovered.document().providers[0].credentials[0]
                .auth_token
                .as_deref(),
            Some(access().as_str())
        );
        assert_eq!(
            recovered.document().providers[0].credentials[0]
                .account_name
                .as_deref(),
            Some("discovered@example.test")
        );
    }
}

#[tokio::test]
async fn cursor_reauthorization_preserves_account_id_and_wrong_identity_cannot_overwrite() {
    let fixture = Fixture::new().await;
    let peer = Peer::new(false).await;
    let first = fixture.body(true);
    fixture.create(&peer, first.clone()).await;
    let saved = fixture
        .wait(first["attemptId"].as_str().unwrap(), "succeeded")
        .await;
    let mut same = fixture.body(true);
    same["credentialId"] = saved["targetCredentialId"].clone();
    fixture.create(&peer, same.clone()).await;
    let unchanged = fixture
        .wait(same["attemptId"].as_str().unwrap(), "succeeded")
        .await;
    assert_eq!(unchanged["savedRevision"], saved["savedRevision"]);
    assert_eq!(unchanged["credentialPersisted"], true);
    let mut replacement = fixture.body(true);
    replacement["credentialId"] = saved["targetCredentialId"].clone();
    let newer = crate::protocol::cursor_auth::tests::token("auth0|synthetic", 4070908900);
    replacement["accessToken"] = json!(newer);
    fixture.create(&peer, replacement.clone()).await;
    let renewed = fixture
        .wait(replacement["attemptId"].as_str().unwrap(), "succeeded")
        .await;
    assert_eq!(renewed["targetCredentialId"], saved["targetCredentialId"]);
    let mut wrong = fixture.body(true);
    wrong["credentialId"] = saved["targetCredentialId"].clone();
    *peer.identity.lock() = json!({"authId":"other","email":"owner@example.test"});
    fixture.create(&peer, wrong.clone()).await;
    let failed = fixture
        .wait(wrong["attemptId"].as_str().unwrap(), "failed")
        .await;
    assert_eq!(failed["errorCode"], "cursor_account_identity_mismatch");
    assert_eq!(failed["authReceived"], true);
    assert_eq!(failed["accountMatched"], false);
    assert_eq!(
        fixture.state.route_config.snapshot().document().providers[0].credentials[0]
            .auth_token
            .as_deref(),
        Some(newer.as_str())
    );
    fixture.close().await;
}

#[tokio::test]
async fn cursor_discovered_identity_cannot_replace_or_duplicate_a_saved_subject() {
    let fixture = Fixture::new().await;
    let peer = Peer::new(false).await;
    let first = fixture.body(false);
    fixture.create(&peer, first.clone()).await;
    let saved = fixture
        .wait(first["attemptId"].as_str().unwrap(), "succeeded")
        .await;
    let snapshot = fixture.state.route_config.snapshot();
    for replace in [false, true] {
        let mut input = fixture.body(false);
        if replace {
            input["credentialId"] = saved["targetCredentialId"].clone();
            // Both token and GetMe agree on a different identity: only saved-subject binding rejects it.
            *peer.login.lock() = json!({"accessToken":crate::protocol::cursor_auth::tests::token("auth0|other",4070908800)});
            *peer.identity.lock() = json!({"authId":"auth0|other","email":"other@example.test"});
        }
        fixture.create(&peer, input.clone()).await;
        let failed = fixture
            .wait(input["attemptId"].as_str().unwrap(), "failed")
            .await;
        assert_eq!(failed["errorCode"], "cursor_import_identity_conflict");
        assert_eq!(failed["authReceived"], true);
        assert_eq!(failed["accountMatched"], true);
        assert_eq!(failed["credentialPersisted"], false);
        let current = fixture.state.route_config.snapshot();
        assert_eq!(current.revision().id(), snapshot.revision().id());
        assert_eq!(
            serde_json::to_value(current.document()).unwrap(),
            serde_json::to_value(snapshot.document()).unwrap()
        );
    }
    fixture.close().await;
}

#[tokio::test]
async fn cursor_optional_email_preserves_legacy_constraints_and_attempt_binding() {
    for expected in [
        Value::Null,
        json!(" OWNER@example.test "),
        json!("other@example.test"),
    ] {
        let fixture = Fixture::new().await;
        let peer = Peer::new(false).await;
        let mut input = fixture.body(false);
        input["expectedEmail"] = expected.clone();
        fixture.create(&peer, input.clone()).await;
        let mismatch = expected == "other@example.test";
        let receipt = fixture
            .wait(
                input["attemptId"].as_str().unwrap(),
                if mismatch { "failed" } else { "succeeded" },
            )
            .await;
        if mismatch {
            assert_eq!(receipt["errorCode"], "cursor_account_identity_mismatch");
            assert_eq!(receipt["accountMatched"], false);
            assert!(
                fixture.state.route_config.snapshot().document().providers[0]
                    .credentials
                    .is_empty()
            );
        } else {
            assert_eq!(receipt["credentialPersisted"], true);
        }
        // Replaying the exact binding is idempotent, but adding/removing an email constraint is not.
        assert_eq!(
            fixture.create(&peer, input.clone()).await["status"],
            receipt["status"]
        );
        if expected.is_null() {
            input["expectedEmail"] = json!("owner@example.test");
        } else {
            input.as_object_mut().unwrap().remove("expectedEmail");
        }
        let response = fixture
            .call(Method::POST, ROOT, input, Some(&fixture.grant))
            .await;
        assert_eq!(
            support::parse_json(response).await["error"]["code"],
            "cursor_import_attempt_conflict"
        );
        assert_eq!(peer.checks.load(Ordering::SeqCst), 1);
        fixture.close().await;
    }
}

#[tokio::test]
async fn cursor_received_invalid_browser_material_is_not_reported_as_missing_login() {
    for token in [
        "invalid".to_owned(),
        crate::protocol::cursor_auth::tests::token("auth0|synthetic", 1),
    ] {
        let fixture = Fixture::new().await;
        let peer = Peer::new(false).await;
        *peer.login.lock() = json!({"accessToken":token});
        let input = fixture.body(false);
        fixture.create(&peer, input.clone()).await;
        let failed = fixture
            .wait(input["attemptId"].as_str().unwrap(), "failed")
            .await;
        assert_eq!(failed["authReceived"], true);
        assert_eq!(failed["accountMatched"], false);
        assert_eq!(failed["credentialPersisted"], false);
        assert_eq!(peer.checks.load(Ordering::SeqCst), 0);
        fixture.close().await;
    }
}

#[tokio::test]
async fn cursor_save_conflict_preserves_received_and_matched_facts() {
    let fixture = Fixture::new().await;
    let peer = Peer::new(true).await;
    let input = fixture.body(true);
    fixture.create(&peer, input.clone()).await;
    peer.checked().await;
    let snapshot = fixture.state.route_config.snapshot();
    let mut changed = snapshot.document().clone();
    changed.providers[0].label = Some("Concurrent edit".into());
    fixture
        .state
        .route_config_runtime
        .as_ref()
        .unwrap()
        .commit_document(snapshot.revision().id(), changed, None)
        .await
        .unwrap();
    peer.release.notify_one();
    let failed = fixture
        .wait(input["attemptId"].as_str().unwrap(), "failed")
        .await;
    assert_eq!(failed["errorCode"], "console_revision_conflict");
    assert_eq!(failed["authReceived"], true);
    assert_eq!(failed["accountMatched"], true);
    assert_eq!(failed["credentialPersisted"], false);
    fixture.close().await;
}
