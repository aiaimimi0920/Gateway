use super::*;

fn existing_document() -> crate::routing::config::RouteConfigYaml {
    let mut document = document();
    document.providers[0].credentials = serde_json::from_value(json!([
        {"id":"sibling","api_key":"sibling-token"},
        {"id":"original","api_key":"old-session-token","supported_models":["model-a"],
         "freebuff_session":{"fingerprint_id":FINGERPRINT,"fingerprint_hash":"old-hash"},
         "extra_body":{"freebuffAgentId":"agent-a"}}
    ]))
    .unwrap();
    document
}

#[tokio::test]
async fn freebuff_reauthorization_reuses_stable_account_device_and_model_agent_metadata() {
    let fixture = support::ConsoleStateFixture::new(existing_document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let mock = Mock::new(vec![code(), user()]).await;
    let mut body = create_body(state);
    body["credentialId"] = json!("original");
    body.as_object_mut().unwrap().remove("fingerprintId");
    let (view, session) = create(state, &grant, body, &mock).await;
    make_ready(state, &view, &session, &grant, &mock).await;
    assert_eq!(
        action(state, &view, Some(&grant), "import").await.status(),
        StatusCode::OK
    );
    state.lifecycle.freebuff_login.join().await;
    assert_eq!(session.view().status, SessionStatus::Succeeded);
    assert_eq!(session.view().credential_id.as_deref(), Some("original"));
    let snapshot = state.route_config.snapshot();
    let accounts = &snapshot.document().providers[0].credentials;
    assert_eq!(accounts.len(), 2);
    assert_eq!(accounts[0].api_key.as_deref(), Some("sibling-token"));
    assert_eq!(accounts[1].api_key.as_deref(), Some("synthetic-token"));
    assert_eq!(accounts[1].extra_body["freebuffAgentId"], "agent-a");
    assert_eq!(accounts[1].supported_models, ["model-a"]);
    assert_eq!(
        accounts[1]
            .freebuff_session
            .as_ref()
            .unwrap()
            .fingerprint_id(),
        Some(FINGERPRINT)
    );
    assert_eq!(
        mock.calls.lock().unwrap()[0].3,
        json!({"fingerprintId":FINGERPRINT})
            .to_string()
            .into_bytes()
    );
    mock.stop().await;
}

#[tokio::test]
async fn freebuff_reauthorization_rejects_device_change_and_wrong_account_before_initialization() {
    let fixture = support::ConsoleStateFixture::new(existing_document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let mock = Mock::new(vec![]).await;
    for (id, device) in [
        ("original", "different-device"),
        ("missing-account", FINGERPRINT),
    ] {
        let mut body = create_body(state);
        body["credentialId"] = json!(id);
        body["fingerprintId"] = json!(device);
        let response = TEST_ORIGIN
            .scope(
                mock.origin.clone(),
                support::send_console_json(state, Method::POST, ROOT, body, Some(&grant), None),
            )
            .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(response.headers()["cache-control"], "no-store");
    }
    state.lifecycle.freebuff_login.join().await;
    assert_eq!(mock.count(), 0);
    mock.stop().await;
}
