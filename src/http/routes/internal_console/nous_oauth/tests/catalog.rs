use super::{fixture::*, *};
#[tokio::test]
async fn nous_catalog_read_then_normal_cas_keeps_secrets_and_manual_models() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let peer = Peer::new(false, 600).await;
    let long = format!("org/{}", "x".repeat(400));
    *peer.catalog.lock().unwrap() = json!({"data":[{"id":long}]});
    let view = create(state, &grant, body(state), &peer).await;
    ready(state, &view, &grant, &peer).await;
    action(state, &view, Some(&grant), "import", &peer).await;
    wait_status(&session(state, &view), SessionStatus::Succeeded).await;
    let before = state.route_config.snapshot();
    let id = before.document().providers[0].credentials[0]
        .id
        .clone()
        .unwrap();
    let input =
        json!({"providerId":"nous","credentialId":id,"expectedRevision":before.revision().id()});
    let path = "/v1/internal/gateway/console/nous-model-catalog";
    assert_ne!(
        call(state, path, input.clone(), None).await.status(),
        StatusCode::OK
    );
    let response = origin(
        peer.origin.clone(),
        call(state, path, input.clone(), Some(&grant)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let result = support::parse_json(response).await;
    assert_redacted(&result);
    assert_eq!(result["catalog"]["models"], json!([{"id":long}]));
    assert_eq!(
        state.route_config.snapshot().revision().id(),
        before.revision().id()
    );
    let redacted = crate::console::secrets::redact_route_document(before.document()).unwrap();
    let mut draft = serde_json::to_value(redacted.document).unwrap();
    draft["providers"][0]["credentials"][0]["supported_models"] = json!([long, "manual"]);
    draft["providers"][0]["credentials"][0]["model_selection"] =
        json!({"fetched":[long],"manual":["manual"]});
    let patches: Vec<_> = redacted
        .secrets
        .iter()
        .filter(|s| s.configured)
        .map(|s| json!({"path":s.path,"operation":"keep"}))
        .collect();
    let update =
        json!({"expectedRevision":before.revision().id(),"document":draft,"secretPatches":patches});
    let response = support::send_console_json(
        state,
        Method::PUT,
        "/v1/internal/gateway/console/route-config",
        update.clone(),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        support::parse_json(response).await
    );
    let after = state.route_config.snapshot();
    let c = &after.document().providers[0].credentials[0];
    assert_eq!(c.model_selection.as_ref().unwrap().manual, ["manual"]);
    assert_eq!(c.model_selection.as_ref().unwrap().fetched, [long.clone()]);
    assert!(after.resolve_candidates(Some(&long)).iter().any(|c| c
        .payload
        .credential_id
        .as_deref()
        == Some(id.as_str())
        && c.upstream_model.as_deref() == Some(long.as_str())));
    assert_eq!(
        c.refresh_token,
        before.document().providers[0].credentials[0].refresh_token
    );
    assert_eq!(
        c.nous_portal,
        before.document().providers[0].credentials[0].nous_portal
    );
    let response = support::send_console_json(
        state,
        Method::PUT,
        "/v1/internal/gateway/console/route-config",
        update,
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        call(state, path, input, Some(&grant)).await.status(),
        StatusCode::CONFLICT
    );
    assert_eq!(peer.count(), 1);
    state.lifecycle.nous_oauth.join().await;
    peer.stop().await;
}
