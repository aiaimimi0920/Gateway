//! AWS OIDC grants are JSON and require a registered client secret, not generic form OAuth.
use super::*;
use crate::routing::config::RouteConfigStore;
use serde_json::json;

async fn prepare(
    endpoint: &str,
    adapter: &str,
) -> Result<grant::Prepared, crate::token_refresh::transport::RefreshError> {
    let store = RouteConfigStore::from_document(serde_json::from_value(json!({"providers":[{
        "id":"renamed-pool","adapter":adapter,"base_url":"https://codewhisperer.us-east-1.amazonaws.com",
        "explicit_credentials_only":true,"credentials":[{"id":"account","api_key":"synthetic-access",
        "refresh_token":"synthetic-refresh","refresh_endpoint":endpoint,"refresh_client_id":"synthetic-client",
        "expires_at":"2020-01-01T00:00:00Z"}]}]})).unwrap()).unwrap();
    let snapshot = store.snapshot();
    let (_, credential) = snapshot
        .oauth_refresh_target("renamed-pool", "account")
        .unwrap();
    let attempt = credential
        .refresh_config
        .as_ref()
        .unwrap()
        .begin_refresh(Duration::from_secs(300), &credential.payload.api_key)
        .unwrap();
    grant::Prepared::new(&client(), &attempt, &credential.payload).await
}

#[tokio::test]
async fn kiro_refresh_must_not_enter_generic_form_at_aws_oidc_endpoint() {
    let result = prepare(
        "https://oidc.us-east-1.amazonaws.com/token",
        "kiro_compatible",
    )
    .await;
    assert!(
        !matches!(result, Ok(grant::Prepared::Generic(_))),
        "Kiro refresh reached generic form"
    );
}

#[tokio::test]
async fn kiro_refresh_foreign_endpoint_is_rejected_recoverably_before_reservation() {
    let result = prepare("https://untrusted.invalid/token", "kiro_compatible").await;
    assert!(
        result.is_err(),
        "Kiro refresh accepted a foreign destination"
    );
    assert!(!result.err().unwrap().terminal);
}

#[tokio::test]
async fn aws_oidc_endpoint_cannot_be_disguised_as_generic_form_oauth() {
    let result = prepare(
        "https://oidc.us-east-1.amazonaws.com/token",
        "openai_compatible",
    )
    .await;
    assert!(
        result.is_err(),
        "AWS OIDC refresh requires registered JSON client context"
    );
    assert!(!result.err().unwrap().terminal);
}
