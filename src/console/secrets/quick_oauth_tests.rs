//! Secret redaction and Keep destination-binding tests for Amazon Quick (`quick_oauth`).

use super::*;
use serde_json::json;

#[test]
fn quick_oauth_secret_redaction_and_keep_destination_binding() {
    let active: RouteConfigYaml = serde_json::from_value(json!({
        "providers": [{
            "id": "quick-oauth",
            "preset": "quick-oauth",
            "auth_mode": "quick_oauth",
            "explicit_credentials_only": true,
            "base_url": "https://quick.aws.com",
            "credentials": [{
                "id": "quick-cred-1",
                "auth_mode": "quick_oauth",
                "api_key": "secret-bearer-jwt",
                "refresh_token": "secret-refresh-token",
                "quick_oauth": {
                    "idc_region": "us-east-1",
                    "issuer_url": "https://d-1234567890.awsapps.com/start",
                    "ws_host": "cell001.dp.appintegrations.us-east-1.prod.plato.ai.aws.dev",
                    "client_id": "dynamic-client-id",
                    "client_secret": "secret-oidc-client-secret",
                    "email": "user@example.com"
                }
            }]
        }]
    }))
    .unwrap();

    let redacted_bundle = redact_route_document(&active).unwrap();
    let encoded = serde_json::to_value(&redacted_bundle.document).unwrap();
    let raw_str = encoded.to_string();
    assert!(!raw_str.contains("secret-bearer-jwt"));
    assert!(!raw_str.contains("secret-refresh-token"));
    assert!(!raw_str.contains("secret-oidc-client-secret"));

    let keeps = [
        SecretPatch::keep("/providers/0/credentials/0/api_key"),
        SecretPatch::keep("/providers/0/credentials/0/refresh_token"),
        SecretPatch::keep("/providers/0/credentials/0/quick_oauth/client_secret"),
    ];
    let restored =
        resolve_secret_patches(&active, redacted_bundle.document.clone(), &keeps).unwrap();
    assert_eq!(
        restored.providers[0].credentials[0]
            .quick_oauth
            .as_ref()
            .and_then(|c| c.client_secret()),
        Some("secret-oidc-client-secret")
    );

    for (field, mutated) in [
        ("idc_region", json!("us-west-2")),
        (
            "issuer_url",
            json!("https://d-0000000000.awsapps.com/start"),
        ),
        ("ws_host", json!("evil.aws.dev")),
        ("client_id", json!("other-client-id")),
    ] {
        let mut tampered = encoded.clone();
        tampered["providers"][0]["credentials"][0]["quick_oauth"][field] = mutated;
        let err =
            resolve_secret_patches(&active, serde_json::from_value(tampered).unwrap(), &keeps)
                .unwrap_err();
        assert_eq!(err.code(), "secret_keep_destination_changed", "{field}");
    }

    let mut model_edited = redacted_bundle.document;
    model_edited.providers[0].credentials[0].supported_models = vec!["smart".into(), "fast".into()];
    assert!(resolve_secret_patches(&active, model_edited, &keeps).is_ok());
}
