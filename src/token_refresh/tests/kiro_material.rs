//! Optional Kiro extension preserves legacy evidence; incomplete context never becomes generic refresh.
use super::*;
use serde_json::json;

fn context() -> crate::protocol::kiro_oauth::AuthorizationContext {
    serde_json::from_value(
        json!({"auth_method":"builder_id","directory_region":"us-east-1",
        "start_url":"https://view.awsapps.com/start","client_id":"synthetic-client",
        "client_secret":"synthetic-secret","client_secret_expires_at":1893456000i64,
        "scopes":["codewhisperer:conversations"],"machine_id":"a".repeat(64)}),
    )
    .unwrap()
}

fn typed_payload(provider: &mut crate::routing::config::ProviderConfigYaml) {
    // Test the registered capability even when the optional preset is correctly compiled out.
    provider.preset = None;
    provider.adapter = Some("kiro_compatible".into());
    provider.base_url = "https://q.us-east-1.amazonaws.com".into();
    provider.session_auth = Some(crate::credential_runtime::SessionAuthConfig {
        transport: "bearer".into(),
        primary_cookie_name: None,
        secondary_cookie_name: None,
        header_name: Some("authorization".into()),
        expires_at: None,
    });
}

#[tokio::test]
async fn kiro_redacted_context_has_no_refresh_owner_even_with_explicit_generic_endpoint() {
    let fixture = LocalFixture::new("https://oidc.us-east-1.amazonaws.com/token").await;
    let snapshot = fixture.runtime.route_config().snapshot();
    let mut document = snapshot.document().clone();
    let provider = &mut document.providers[0];
    typed_payload(provider);
    let credential = &mut provider.credentials[0];
    credential.headers.clear();
    credential.refresh_client_id = Some("synthetic-client".into());
    let mut context = context();
    context.take_client_secret();
    credential.kiro_oauth = Some(context);
    fixture
        .runtime
        .commit_document(snapshot.revision().id(), document, None)
        .await
        .unwrap();
    let snapshot = fixture.runtime.route_config().snapshot();
    assert!(snapshot
        .oauth_refresh_target("oauth", "account")
        .unwrap()
        .1
        .refresh_config
        .is_none());
}

#[tokio::test]
async fn kiro_expired_registration_fails_before_durable_sending_or_network() {
    let fixture = LocalFixture::new("https://oidc.us-east-1.amazonaws.com/token").await;
    let snapshot = fixture.runtime.route_config().snapshot();
    let mut document = snapshot.document().clone();
    let provider = &mut document.providers[0];
    typed_payload(provider);
    let credential = &mut provider.credentials[0];
    credential.headers.clear();
    credential.refresh_client_id = None;
    let mut value = serde_json::to_value(context()).unwrap();
    value["client_secret_expires_at"] = json!(1);
    credential.kiro_oauth = Some(serde_json::from_value(value).unwrap());
    fixture
        .runtime
        .commit_document(snapshot.revision().id(), document, None)
        .await
        .unwrap();
    let snapshot = fixture.runtime.route_config().snapshot();
    let target = snapshot.oauth_refresh_target("oauth", "account").unwrap().1;
    let owner = target.refresh_config.as_ref().unwrap();
    let attempt = owner
        .begin_refresh(Duration::ZERO, &target.payload.api_key)
        .unwrap();
    let error = super::super::grant::Prepared::new(&client(), &attempt, &target.payload)
        .await
        .err()
        .unwrap();
    if cfg!(feature = "line-kiro-official-vendor-api") {
        assert!(error.terminal);
        assert_eq!(error.code, "kiro_oauth_reauthorization_required");
    } else {
        assert!(!error.terminal);
        assert_eq!(error.code, "kiro_oauth_feature_disabled");
    }
}
