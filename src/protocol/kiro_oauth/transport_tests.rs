use super::{fixture::*, *};
use serde_json::{json, Value};
use std::time::Duration;

#[tokio::test]
async fn public_registration_device_and_refresh_send_exact_json_and_no_inherited_credentials() {
    let mock = Mock::new(vec![registration(), device(None), tokens()]).await;
    let http = http();
    let client = register(&mock, &http).await;
    let challenge = begin(&mock, &client, &http).await;
    assert_eq!(challenge.user_code(), "ABCD-EFGH");
    assert_eq!(challenge.interval(), Duration::from_secs(5));
    let grant = client.refresh_grant("synthetic-old-refresh").unwrap();
    let before = time::OffsetDateTime::now_utc();
    let result = TEST_ORIGIN
        .scope(mock.origin.clone(), grant.execute(&http))
        .await
        .unwrap();
    assert_eq!(result.refresh_token(), Some("synthetic-new-refresh"));
    assert!(result.expires_at().unwrap() >= before + time::Duration::seconds(3600));
    let calls = mock.calls.lock().unwrap().clone();
    assert_eq!(
        calls.iter().map(|call| call.0.as_str()).collect::<Vec<_>>(),
        ["/client/register", "/device_authorization", "/token"]
    );
    for (_, headers, _) in &calls {
        assert!(headers["content-type"]
            .to_str()
            .unwrap()
            .starts_with("application/json"));
        for key in ["authorization", "cookie", "x-api-key"] {
            assert!(!headers.contains_key(key));
        }
    }
    let register: Value = serde_json::from_slice(&calls[0].2).unwrap();
    assert_eq!(
        register,
        json!({"clientName":"Neuro Gateway","clientType":"public","scopes":["codewhisperer:conversations"],
        "grantTypes":["urn:ietf:params:oauth:grant-type:device_code","refresh_token"]})
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&calls[1].2).unwrap(),
        json!({"clientId":"synthetic-gateway-client", "clientSecret":"synthetic-registered-secret", "startUrl":"https://view.awsapps.com/start"})
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&calls[2].2).unwrap(),
        json!({"clientId":"synthetic-gateway-client", "clientSecret":"synthetic-registered-secret", "grantType":"refresh_token", "refreshToken":"synthetic-old-refresh"})
    );
    for text in [
        format!("{client:?}"),
        format!("{challenge:?}"),
        format!("{result:?}"),
    ] {
        assert!(!text.contains("synthetic-"));
    }
    mock.stop().await;
}
#[tokio::test]
async fn identity_center_registration_keeps_directory_start_and_explicit_scope_selection() {
    let mock = Mock::new(vec![
        registration(),
        ResponseSpec::json(
            200,
            json!({"deviceCode":"code","userCode":"AAAA-BBBB",
        "verificationUri":"https://device.sso.eu-west-1.api.aws/","expiresIn":30}),
        ),
    ])
    .await;
    let http = http();
    let directory =
        Directory::identity_center("eu-west-1", "https://organization.awsapps.com/start/").unwrap();
    let client = TEST_ORIGIN
        .scope(
            mock.origin.clone(),
            ClientRegistration::register(
                &http,
                directory.clone(),
                &[Scope::Analysis, Scope::Conversations],
            ),
        )
        .await
        .unwrap();
    assert_eq!(client.directory(), &directory);
    assert_eq!(client.scopes(), [Scope::Analysis, Scope::Conversations]);
    let challenge = begin(&mock, &client, &http).await;
    assert!(challenge.remaining() <= Duration::from_secs(30));
    let body: Value = serde_json::from_slice(&mock.calls.lock().unwrap()[1].2).unwrap();
    assert_eq!(body["startUrl"], "https://organization.awsapps.com/start/");
    mock.stop().await;
}
#[tokio::test]
async fn registration_rejects_invalid_or_expired_secret_and_does_not_follow_returned_endpoints() {
    for value in [
        json!({"clientId":"c","clientSecret":"s","clientIdIssuedAt":1,"clientSecretExpiresAt":2}),
        json!({"clientId":"c","clientSecret":"","clientIdIssuedAt":1,"clientSecretExpiresAt":999999999999i64}),
        json!({"clientId":"c","clientSecret":"s","clientIdIssuedAt":1}),
        json!({"clientId":"c","clientSecret":"s","clientIdIssuedAt":3,"clientSecretExpiresAt":2}),
    ] {
        let mock = Mock::new(vec![ResponseSpec::json(200, value)]).await;
        let result = TEST_ORIGIN
            .scope(
                mock.origin.clone(),
                ClientRegistration::register(
                    &http(),
                    Directory::builder_id(),
                    &[Scope::Conversations],
                ),
            )
            .await;
        assert!(result.is_err());
        assert_eq!(mock.count(), 1);
        mock.stop().await;
    }
    let mut registered: Value = serde_json::from_slice(&registration().body).unwrap();
    registered["tokenEndpoint"] = json!("https://untrusted.invalid/token");
    registered["authorizationEndpoint"] = json!("https://untrusted.invalid/authorize");
    let mock = Mock::new(vec![ResponseSpec::json(200, registered), tokens()]).await;
    let http = http();
    let client = register(&mock, &http).await;
    TEST_ORIGIN
        .scope(
            mock.origin.clone(),
            client.refresh_grant("refresh").unwrap().execute(&http),
        )
        .await
        .unwrap();
    assert_eq!(mock.calls.lock().unwrap()[1].0, "/token");
    mock.stop().await;
}
#[tokio::test]
async fn empty_duplicate_scopes_and_invalid_refresh_are_rejected_before_network() {
    let mock = Mock::new(vec![registration()]).await;
    let http = http();
    for scopes in [vec![], vec![Scope::Conversations, Scope::Conversations]] {
        assert_eq!(
            TEST_ORIGIN
                .scope(
                    mock.origin.clone(),
                    ClientRegistration::register(&http, Directory::builder_id(), &scopes)
                )
                .await
                .unwrap_err(),
            OAuthError::InvalidInput
        );
    }
    assert_eq!(mock.count(), 0);
    let client = register(&mock, &http).await;
    for token in ["", "with space", "private\nheader"] {
        assert_eq!(
            client.refresh_grant(token).unwrap_err(),
            OAuthError::InvalidInput
        );
    }
    assert_eq!(mock.count(), 1);
    mock.stop().await;
}
