//! Directory Region is not inference Region. Explicit IdC input is preserved through ordinary save.
use super::*;
#[tokio::test]
async fn identity_center_directory_is_explicit_and_never_inferred_from_business_region() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let mut body = create_body(state);
    body["directory"] = json!({"authMethod":"identity_center","directoryRegion":"eu-west-1","startUrl":"https://d-example.awsapps.com/start"});
    let device = ResponseSpec::json(
        200,
        json!({"deviceCode":"synthetic-idc-device-secret","userCode":"ABCD-EFGH",
        "verificationUri":"https://device.sso.eu-west-1.amazonaws.com/","expiresIn":600,"interval":1}),
    );
    let mock = Mock::new(vec![registration(), device, tokens(true, true)]).await;
    let (view, session) = create(state, &grant, body, &mock).await;
    ready(state, &view, &session, &grant, &mock).await;
    let response = action(state, &view, Some(&grant), "import").await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        support::parse_json(response).await
    );
    state.lifecycle.kiro_oauth.join().await;
    assert_eq!(session.view().status, SessionStatus::Succeeded);
    let snapshot = state.route_config.snapshot();
    let saved = &snapshot.document().providers[0].credentials[0];
    assert_eq!(
        snapshot.document().providers[0].base_url,
        "https://q.us-east-1.amazonaws.com"
    );
    let context = saved.kiro_oauth.as_ref().unwrap();
    assert_eq!(
        context.token_endpoint(),
        "https://oidc.eu-west-1.amazonaws.com/token"
    );
    let metadata = serde_json::to_value(context).unwrap();
    assert_eq!(metadata["auth_method"], "identity_center");
    assert_eq!(metadata["start_url"], "https://d-example.awsapps.com/start");
    let calls = mock.calls.lock().unwrap().clone();
    assert_eq!(
        serde_json::from_slice::<Value>(&calls[1].2).unwrap()["startUrl"],
        "https://d-example.awsapps.com/start"
    );
    assert_eq!(calls.len(), 3);
    mock.stop().await;
}
