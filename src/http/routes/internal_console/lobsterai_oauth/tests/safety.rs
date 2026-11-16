use super::{fixture::*, *};
#[tokio::test]
async fn lobsterai_management_requires_consent_local_storage_and_exact_revision() {
    let fixture = Fixture::new(document()).await;
    let grant = support::grant_console_secret_access(&fixture.state).await;
    let denied = call(&fixture.state, ROOT, body(&fixture.state), None).await;
    assert_ne!(denied.status(), StatusCode::OK);
    assert_eq!(denied.headers()["cache-control"], "no-store");
    let mut input = body(&fixture.state);
    input["expectedRevision"] = json!("stale");
    assert_eq!(
        call(&fixture.state, ROOT, input, Some(&grant))
            .await
            .status(),
        StatusCode::CONFLICT
    );
    fixture.state.lifecycle.lobsterai_oauth.join().await;
    let mut server = Fixture::new(document()).await;
    Arc::get_mut(&mut server.state).unwrap().config.storage_mode =
        crate::config::GatewayStorageMode::Server;
    let grant = support::grant_console_secret_access(&server.state).await;
    let denied = call(&server.state, ROOT, body(&server.state), Some(&grant)).await;
    assert_eq!(
        support::parse_json(denied).await["error"]["code"],
        "lobsterai_local_runtime_required"
    );
    server.state.lifecycle.lobsterai_oauth.join().await;
}
#[tokio::test]
async fn lobsterai_management_cancel_shutdown_and_capacity_release_listeners_without_exchange() {
    let fixture = Fixture::new(document()).await;
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let peer = Peer::start(vec![]).await;
    let view = create(state, &grant, body(state), &peer).await;
    assert_eq!(
        action(state, &view, Some(&grant), "cancel", &peer)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        session(state, &view).view().status,
        SessionStatus::Cancelled
    );
    let mut urls = vec![view["redirectUri"].as_str().unwrap().to_owned()];
    for _ in 0..15 {
        urls.push(
            create(state, &grant, body(state), &peer).await["redirectUri"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }
    let response = protocol::transport::TEST_ORIGIN
        .scope(
            peer.origin.clone(),
            call(state, ROOT, body(state), Some(&grant)),
        )
        .await;
    assert_ne!(response.status(), StatusCode::OK);
    state.lifecycle.lobsterai_oauth.join().await;
    for address in urls {
        let url = url::Url::parse(&address).unwrap();
        assert!(tokio::net::TcpStream::connect((
            std::net::Ipv4Addr::LOCALHOST,
            url.port().unwrap()
        ))
        .await
        .is_err());
    }
    peer.inspect(|seen| assert!(seen.is_empty()));
}
#[tokio::test]
async fn lobsterai_management_changed_target_blocks_exchange_and_terminal_status_remains_readable()
{
    let fixture = Fixture::new(document()).await;
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let peer = Peer::start(vec![]).await;
    let view = create(state, &grant, body(state), &peer).await;
    changed(state).await;
    let callback = protocol::test_support::callback(
        view["authorizationUrl"].as_str().unwrap(),
        "synthetic-code",
    );
    deliver(&callback).await;
    wait_status(&session(state, &view), SessionStatus::Failed).await;
    let response = support::send_console_json(
        state,
        Method::GET,
        &format!("{ROOT}/{}", view["id"].as_str().unwrap()),
        Value::Null,
        None,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_redacted(&support::parse_json(response).await);
    peer.inspect(|seen| assert!(seen.is_empty()));
    state.lifecycle.lobsterai_oauth.join().await;
}
