//! Accio requires current access + refresh JSON; the generic form sender must never own it.
use super::*;
use crate::routing::config::RouteConfigStore;
use serde_json::json;

pub(super) async fn prepared(
    endpoint: &str,
    adapter: Option<&str>,
) -> Result<grant::Prepared, crate::token_refresh::transport::RefreshError> {
    let mut value = json!({"providers":[{"id":"synthetic-accio-pool", "preset":"accio",
        "base_url":"https://phoenix-gw.alibaba.com", "explicit_credentials_only":true,
        "credentials":[{"id":"synthetic-account", "api_key":"synthetic-current-access",
            "refresh_token":"synthetic-current-refresh", "refresh_endpoint":endpoint,
            "refresh_client_id":"synthetic-client", "expires_at":"2020-01-01T00:00:00Z",
            "headers":{"utdid":"synthetic-device"}}]}]});
    if let Some(adapter) = adapter {
        value["providers"][0]
            .as_object_mut()
            .unwrap()
            .remove("preset");
        value["providers"][0]["adapter"] = json!(adapter);
    }
    let store = RouteConfigStore::from_document(serde_json::from_value(value).unwrap()).unwrap();
    let snapshot = store.snapshot();
    let (_, credential) = snapshot
        .oauth_refresh_target("synthetic-accio-pool", "synthetic-account")
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
async fn accio_refresh_cannot_enter_generic_form_even_at_correct_endpoint() {
    let result = prepared(
        "https://phoenix-gw.alibaba.com/api/auth/refresh_token",
        None,
    )
    .await;
    assert!(
        !matches!(result, Ok(grant::Prepared::Generic(_))),
        "Accio was routed to form transport"
    );
}

#[tokio::test]
async fn accio_refresh_rejects_foreign_destination_before_building_a_request() {
    let result = prepared("https://untrusted.invalid/token", None).await;
    assert!(
        result.is_err(),
        "Accio material was accepted for a foreign refresh endpoint"
    );
    assert!(
        !result.err().unwrap().terminal,
        "configuration is recoverable, not a permanently invalid account"
    );
}

#[tokio::test]
async fn accio_endpoint_cannot_be_disguised_as_a_generic_adapter() {
    let result = prepared(
        "https://phoenix-gw.alibaba.com/api/auth/refresh_token",
        Some("openai_compatible"),
    )
    .await;
    assert!(
        result.is_err(),
        "Accio endpoint was accepted by the generic form transport"
    );
}
