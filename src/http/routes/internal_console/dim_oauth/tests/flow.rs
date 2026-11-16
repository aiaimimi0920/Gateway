use super::{fixture::*, *};
use crate::console::dim_oauth::SessionStatus;
use std::{sync::atomic::Ordering, time::Duration};

#[tokio::test]
async fn dim_oauth_management_callback_does_not_save_and_wrong_state_does_not_consume() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let peer = Peer::new(3600, Duration::ZERO).await;
    let view = create(state, &grant, create_body(state), &peer).await;
    let session = session(state, &view);
    assert!(state
        .lifecycle
        .dim_oauth
        .get_owned(session.id(), "another-owner")
        .is_err());
    let good = callback(&view);
    let bad = format!(
        "{}?code=foreign&state=wrong",
        view["redirectUri"].as_str().unwrap()
    );
    assert!(deliver(&bad).await.starts_with("HTTP/1.1 400"));
    assert_eq!(session.view().status, SessionStatus::WaitingUser);
    assert_eq!(peer.count(), 0);
    assert!(deliver(&good).await.starts_with("HTTP/1.1 200"));
    assert_eq!(session.view().status, SessionStatus::Ready);
    assert_eq!(peer.count(), 1);
    assert_eq!(peer.userinfo.load(Ordering::SeqCst), 1);
    assert!(state.route_config.snapshot().document().providers[0]
        .credentials
        .is_empty());
    let response = action(state, &view, Some(&grant), "import", None, &peer).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_redacted(&support::parse_json(response).await);
    wait_status(&session, SessionStatus::Succeeded).await;
    let snapshot = state.route_config.snapshot();
    let saved = &snapshot.document().providers[0].credentials[0];
    assert_eq!(saved.api_key.as_deref(), Some("synthetic-dim-access"));
    assert_eq!(
        saved.refresh_token.as_deref(),
        Some("synthetic-dim-refresh")
    );
    assert_eq!(
        saved.dim_oauth.as_ref().unwrap().subject(),
        "synthetic-subject"
    );
    assert_eq!(
        saved.refresh_endpoint.as_deref(),
        Some("https://dimagent.cn/oauth/token")
    );
    assert_eq!(
        saved.refresh_client_id.as_deref(),
        Some("permitted-gateway")
    );
    assert!(saved.expires_at.is_some());
    assert_eq!(saved.token_expires_in_secs, None);
    let catalog = saved
        .dim_catalog
        .as_ref()
        .expect("explicit save fetches a wire directory");
    assert_eq!(catalog.models().len(), 4);
    assert!(saved.discovery.is_none());
    for (model, family) in [
        ("opaque-chat", "openai_chat"),
        ("opaque-responses", "openai_responses"),
        ("opaque-messages", "anthropic_messages"),
        ("opaque-gemini", "gemini_generate_content"),
    ] {
        let candidate = snapshot
            .resolve_candidates(Some(model))
            .into_iter()
            .find(|c| c.provider_account_id == "dim")
            .unwrap();
        assert_eq!(candidate.protocol_family, family);
        assert_eq!(candidate.supported_protocol_families, [family]);
    }
    assert_eq!(
        snapshot.document().providers[1].api_key,
        "synthetic-unrelated-secret"
    );
    let revision = snapshot.revision().id().to_owned();
    let response = action(state, &view, Some(&grant), "import", None, &peer).await;
    assert_eq!(
        support::parse_json(response).await["session"]["status"],
        "succeeded"
    );
    assert_eq!(state.route_config.snapshot().revision().id(), revision);
    assert_eq!(peer.count(), 1);
    let account = saved.id.as_deref().unwrap();
    let persisted: crate::routing::config::RouteConfigYaml =
        serde_json::from_slice(&serde_json::to_vec(snapshot.document()).unwrap()).unwrap();
    let reloaded = crate::routing::config::RouteConfigStore::from_document(persisted).unwrap();
    assert_eq!(
        reloaded.get_providers()[0].credential_pool[0]
            .payload
            .api_key,
        "synthetic-dim-access"
    );
    assert_eq!(
        reloaded
            .snapshot()
            .resolve_candidates(Some("opaque-gemini"))
            .into_iter()
            .find(|c| c.provider_account_id == "dim")
            .unwrap()
            .protocol_profile,
        "dim_user_oauth_gemini"
    );
    super::wire::probe_all(state, &grant, account, &peer).await;
    super::wire::send_all(state, &peer).await;
    let calls = peer.calls.lock().unwrap();
    let fields: std::collections::HashMap<_, _> =
        url::form_urlencoded::parse(calls[0].1.as_bytes())
            .into_owned()
            .collect();
    assert_eq!(fields["client_id"], "permitted-gateway");
    assert_eq!(fields["grant_type"], "authorization_code");
    assert_eq!(
        fields["redirect_uri"],
        view["redirectUri"].as_str().unwrap()
    );
    assert!(!calls[0].0.contains_key("cookie"));
    assert!(!calls[0].0.contains_key("authorization"));
    assert_eq!(calls[0].0["x-title"], "Neuro Gateway");
    drop(calls);
    state.lifecycle.dim_oauth.join().await;
    peer.stop().await;
}

#[tokio::test]
async fn dim_oauth_management_manual_callback_and_listener_share_one_consume_gate() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let peer = Peer::new(3600, Duration::from_secs(1)).await;
    let view = create(state, &grant, create_body(state), &peer).await;
    let session = session(state, &view);
    let callback = callback(&view);
    let complete = action(
        state,
        &view,
        Some(&grant),
        "complete",
        Some(&callback),
        &peer,
    );
    let duplicate = deliver(&callback);
    let (one, two) = tokio::join!(complete, duplicate);
    assert_eq!(one.status(), StatusCode::OK);
    assert!(two.starts_with("HTTP/1.1 400"));
    assert_eq!(peer.count(), 1);
    assert_eq!(session.view().status, SessionStatus::Ready);
    state.lifecycle.dim_oauth.join().await;
    peer.stop().await;
}
