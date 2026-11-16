use super::{fixture::*, *};
#[tokio::test]
async fn lobsterai_management_directory_failure_does_not_publish_or_repeat_save() {
    let fixture = Fixture::new(document()).await;
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let peer = Peer::start(vec![
        Reply::json(tokens()),
        Reply::raw(503, b"unavailable".to_vec()),
    ])
    .await;
    let view = create(state, &grant, body(state), &peer).await;
    ready(state, &view).await;
    let revision = state.route_config.snapshot().revision().id().to_owned();
    action(state, &view, Some(&grant), "import", &peer).await;
    wait_status(&session(state, &view), SessionStatus::SaveUnconfirmed).await;
    assert_eq!(
        session(state, &view).view().error_code,
        Some("lobsterai_catalog_read_failed")
    );
    assert_eq!(
        action(state, &view, Some(&grant), "import", &peer)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(state.route_config.snapshot().revision().id(), revision);
    peer.inspect(|seen| assert_eq!(seen.len(), 2));
    state.lifecycle.lobsterai_oauth.join().await;
}
#[tokio::test]
async fn lobsterai_management_accepted_save_outlives_cancel_and_loses_cas_without_rebase() {
    for conflict in [false, true] {
        let fixture = Fixture::new(document()).await;
        let state = &fixture.state;
        let grant = support::grant_console_secret_access(state).await;
        let peer = Peer::start(vec![Reply::json(tokens()), Reply::json(catalog())]).await;
        let view = create(state, &grant, body(state), &peer).await;
        ready(state, &view).await;
        let publication = state.lifecycle.health_control.publication.lock().await;
        assert_eq!(
            action(state, &view, Some(&grant), "import", &peer)
                .await
                .status(),
            StatusCode::OK
        );
        assert_eq!(
            action(state, &view, Some(&grant), "cancel", &peer)
                .await
                .status(),
            StatusCode::OK
        );
        if conflict {
            changed(state).await;
        }
        drop(publication);
        wait_status(
            &session(state, &view),
            if conflict {
                SessionStatus::SaveUnconfirmed
            } else {
                SessionStatus::Succeeded
            },
        )
        .await;
        assert_eq!(
            state.route_config.snapshot().document().providers[0]
                .credentials
                .len(),
            usize::from(!conflict)
        );
        assert_eq!(
            action(state, &view, Some(&grant), "import", &peer)
                .await
                .status(),
            StatusCode::OK
        );
        peer.inspect(|seen| assert_eq!(seen.len(), 2));
        state.lifecycle.lobsterai_oauth.join().await;
    }
}
#[tokio::test]
async fn lobsterai_management_catalog_storage_rejects_invalid_or_inaccessible_declarations() {
    let fixture = Fixture::new(document()).await;
    let state = &fixture.state;
    let material = protocol::test_support::receipt()
        .admit()
        .unwrap()
        .into_material();
    for rows in [
        json!([{"modelId":"duplicate"},{"modelId":"duplicate"}]),
        json!([{"modelId":"x","contextWindow":0}]),
        json!([{"modelId":"x","accessible":false}]),
    ] {
        let mut doc = document();
        doc.providers[0].credentials.push(serde_json::from_value(json!({"id":"stable","api_key":material.access_token,"refresh_token":material.refresh_token,"lobsterai":material.context,"lobsterai_catalog":{"models":rows}})).unwrap());
        assert!(crate::console::document::validate_route_document(doc).is_err());
    }
    state.lifecycle.lobsterai_oauth.join().await;
}
