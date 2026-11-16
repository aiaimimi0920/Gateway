use super::{fixture::*, *};
#[tokio::test]
async fn lobsterai_management_callback_explicit_save_directory_and_reopen() {
    let fixture = Fixture::new(document()).await;
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let peer = Peer::start(vec![
        Reply::json(tokens()),
        Reply::json(catalog()),
        Reply::json(catalog()),
    ])
    .await;
    let view = create(state, &grant, body(state), &peer).await;
    let session = session(state, &view);
    assert!(state
        .lifecycle
        .lobsterai_oauth
        .get_owned(session.id(), "foreign-owner")
        .is_err());
    let callback = protocol::test_support::callback(
        view["authorizationUrl"].as_str().unwrap(),
        "synthetic-code",
    );
    let wrong = callback.replace("state=", "state=wrong");
    assert!(deliver(&wrong).await.starts_with("HTTP/1.1 400"));
    peer.inspect(|seen| assert!(seen.is_empty()));
    ready(state, &view).await;
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
    let c = &saved.document().providers[0].credentials[0];
    assert_eq!(c.api_key.as_deref(), Some("synthetic-access"));
    assert_eq!(c.refresh_token.as_deref(), Some("synthetic-refresh"));
    assert!(c.expires_at.is_none() && c.token_expires_in_secs.is_none());
    assert_eq!(c.supported_models, ["opaque/model"]);
    assert_eq!(
        c.lobsterai_catalog.as_ref().unwrap().models()[0].supports_tool_calling,
        None
    );
    assert_eq!(
        saved.document().providers[1].api_key,
        "synthetic-unrelated-secret"
    );
    let duplicate = action(state, &view, Some(&grant), "import", &peer).await;
    assert_redacted(&support::parse_json(duplicate).await);
    assert_eq!(
        state.route_config.snapshot().revision().id(),
        saved.revision().id()
    );
    let response=protocol::transport::TEST_ORIGIN.scope(peer.origin.clone(),call(state,"/v1/internal/gateway/console/lobsterai-model-catalog",json!({"providerId":"lobster","credentialId":c.id,"expectedRevision":saved.revision().id()}),Some(&grant))).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(
        support::parse_json(response).await["catalog"]["models"][0]["modelId"],
        "opaque/model"
    );
    assert_eq!(
        state.route_config.snapshot().revision().id(),
        saved.revision().id()
    );
    let reopened = crate::console::RouteConfigRuntime::new_local(&fixture.console).unwrap();
    reopened.recover_startup().await.unwrap();
    assert_eq!(
        reopened.route_config().snapshot().document().providers[0].credentials[0].id,
        c.id
    );
    peer.inspect(|seen| {
        assert_eq!(seen.len(), 3);
        assert_eq!(seen[0].path, "/api/auth/exchange");
        assert_eq!(seen[0].body["firstKeyfrom"], "neuro-gateway");
        assert!(!seen[0].headers.contains_key("authorization"));
        for read in &seen[1..] {
            assert_eq!(read.path, "/api/models/available");
            assert_eq!(read.headers["authorization"], "Bearer synthetic-access");
        }
    });
    state.lifecycle.lobsterai_oauth.join().await;
}
#[tokio::test]
async fn lobsterai_management_reauthorization_preserves_identity_id_metadata_and_advances_generation(
) {
    for same in [true, false] {
        let material = protocol::test_support::receipt()
            .admit()
            .unwrap()
            .into_material();
        let old = material.context.clone();
        let mut doc = document();
        doc.providers[0].credentials.push(serde_json::from_value(json!({"id":"stable","api_key":material.access_token,"refresh_token":material.refresh_token,"lobsterai":material.context,"account_name":"Operator label","enabled":false,"supported_models":["manual"]})).unwrap());
        let fixture = Fixture::new(doc).await;
        let state = &fixture.state;
        let grant = support::grant_console_secret_access(state).await;
        let mut reply = tokens();
        if !same {
            reply["data"]["user"]["yid"] = json!("other-user");
        }
        let peer = Peer::start(if same {
            vec![Reply::json(reply), Reply::json(catalog())]
        } else {
            vec![Reply::json(reply)]
        })
        .await;
        let mut input = body(state);
        input["credentialId"] = json!("stable");
        let view = create(state, &grant, input, &peer).await;
        ready(state, &view).await;
        let response = action(state, &view, Some(&grant), "import", &peer).await;
        if same {
            assert_eq!(response.status(), StatusCode::OK);
            wait_status(&session(state, &view), SessionStatus::Succeeded).await;
        } else {
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            assert_eq!(
                support::parse_json(response).await["error"]["code"],
                "lobsterai_login_identity_mismatch"
            );
        }
        let saved = state.route_config.snapshot();
        let c = &saved.document().providers[0].credentials[0];
        assert_eq!(c.id.as_deref(), Some("stable"));
        assert_eq!(c.account_name.as_deref(), Some("Operator label"));
        assert_eq!(c.enabled, Some(false));
        assert!(c.supported_models.contains(&"manual".into()));
        if same {
            assert!(!old.permits_rotation(c.lobsterai.as_ref().unwrap()));
            assert_ne!(
                old.reauthorization_id(),
                c.lobsterai.as_ref().unwrap().reauthorization_id()
            );
        } else {
            assert_eq!(old, *c.lobsterai.as_ref().unwrap());
        }
        state.lifecycle.lobsterai_oauth.join().await;
    }
}
