//! Explicit OAuth login, six public protocol variants, Keep, clear and two same-directory rebuilds.
use super::*;

#[test]
fn kiro_oauth_local_tcp_login_three_public_protocols_keep_clear_and_rebuild() {
    run(120, flow);
}
async fn flow() {
    let upstream = wire::Server::stream(wire::event_frame("local-answer"), false).await;
    let mut fixture = Fixture::new(&upstream).await;
    let original = fixture
        .server
        .state
        .route_config
        .snapshot()
        .document()
        .providers[0]
        .credentials[0]
        .clone();
    assert_eq!(original.id.as_deref(), Some(fixture.credential.as_str()));
    let context = original.kiro_oauth.as_ref().unwrap();
    assert_eq!(context.runtime_target().unwrap().profile_arn(), PROFILE);
    assert_eq!(
        context.token_endpoint(),
        "https://oidc.us-east-1.amazonaws.com/token"
    );
    let mut count = 0;
    for endpoint in ["chat/completions", "messages", "responses"] {
        for stream in [false, true] {
            let response = fixture
                .server
                .open(endpoint, &fixture.token, body(endpoint, stream))
                .await;
            assert_eq!(response.status().as_u16(), 200);
            let text = response.text().await.unwrap();
            assert!(text.contains("local-answer"), "{text}");
            fixture::assert_redacted(&json!(text));
            if stream {
                assert!(
                    text.contains(match endpoint {
                        "messages" => "message_stop",
                        "responses" => "response.completed",
                        _ => "[DONE]",
                    }),
                    "{text}"
                );
            } else {
                let value: Value = serde_json::from_str(&text).unwrap();
                assert_eq!(value["model"], ALIAS);
                match endpoint {
                    "messages" => assert_eq!(value["stop_reason"], "end_turn"),
                    "responses" => assert_eq!(value["status"], "completed"),
                    _ => assert_eq!(value["choices"][0]["finish_reason"], "stop"),
                }
            }
            count += 1;
            let rows = fixture.audits(count).await;
            fixture.assert_identity(&rows, "completed");
            assert_eq!(
                remaining(&fixture.server, &fixture.key_id).await,
                10 - count as i64
            );
        }
    }
    let mut document = fixture.document().await;
    document["providers"][0]["credentials"][0]["account_name"] = json!("kept-oauth-metadata");
    let (status, kept) = fixture.save(document, "keep", None).await;
    assert_eq!(status, 200, "{kept}");
    let revision = fixture.server.revision();
    let port = fixture.server.port;
    fixture.server.stop().await;
    fixture.server = LocalServer::start_with_oidc(
        fixture.config.clone(),
        &upstream,
        Some(fixture.oidc.origin.clone()),
        Some(port),
    )
    .await;
    assert_eq!(fixture.server.revision(), revision);
    let saved = fixture
        .server
        .state
        .route_config
        .snapshot()
        .document()
        .providers[0]
        .credentials[0]
        .clone();
    assert_eq!(saved.id, original.id);
    assert_eq!(saved.api_key, original.api_key);
    assert_eq!(saved.refresh_token, original.refresh_token);
    assert_eq!(saved.kiro_oauth, original.kiro_oauth);
    assert_eq!(saved.account_name.as_deref(), Some("kept-oauth-metadata"));
    assert_eq!(remaining(&fixture.server, &fixture.key_id).await, 4);
    let response = fixture
        .server
        .open("responses", &fixture.token, body("responses", true))
        .await;
    assert_eq!(response.status().as_u16(), 200);
    assert!(response
        .text()
        .await
        .unwrap()
        .contains("response.completed"));
    fixture.assert_identity(&fixture.audits(7).await, "completed");
    let document = fixture.document().await;
    assert_eq!(
        fixture
            .save(document.clone(), "clear", Some(&fixture.grant))
            .await
            .0,
        403,
        "Old runtime grant must not survive rebuild"
    );
    fixture.grant = fixture.server.grant().await;
    let (status, cleared) = fixture.save(document, "clear", Some(&fixture.grant)).await;
    assert_eq!(status, 200, "{cleared}");
    fixture::assert_redacted(&cleared);
    let response = fixture
        .server
        .open(
            "chat/completions",
            &fixture.token,
            body("chat/completions", false),
        )
        .await;
    assert_ne!(response.status().as_u16(), 200);
    fixture.server.stop().await;
    fixture.server = LocalServer::start_with_oidc(
        fixture.config.clone(),
        &upstream,
        Some(fixture.oidc.origin.clone()),
        Some(port),
    )
    .await;
    let snapshot = fixture.server.state.route_config.snapshot();
    let cleared = &snapshot.document().providers[0].credentials[0];
    assert_eq!(cleared.id.as_deref(), Some(fixture.credential.as_str()));
    assert!(cleared.api_key.is_none() && cleared.refresh_token.is_none());
    assert!(cleared
        .kiro_oauth
        .as_ref()
        .unwrap()
        .client_secret()
        .is_none());
    assert_eq!(remaining(&fixture.server, &fixture.key_id).await, 3);
    assert_wire(&upstream, 7);
    assert_eq!(
        fixture.oidc.count(),
        3,
        "Rebuild must neither reauthorize nor spend a generation"
    );
    drop(snapshot);
    fixture.stop().await;
    upstream.stop().await;
}
