//! Opt-in synthetic frontend export, through real SecretPatch/YAML/configuration owners.
use neuro_gateway::console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch};
use neuro_gateway::routing::config::{RouteConfigStore, RouteConfigYaml};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fixture {
    mode: String,
    active: RouteConfigYaml,
    document: RouteConfigYaml,
    secret_patches: Vec<SecretPatch>,
}

#[test]
#[ignore = "requires explicitly exported synthetic frontend fixtures"]
fn frontend_bedrock_material_reaches_resolver_yaml_and_final_account_payload() {
    let path =
        std::env::var("GATEWAY_BEDROCK_MATERIAL_FIXTURE_PATH").expect("synthetic fixture path");
    let fixtures: Vec<Fixture> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(fixtures.len(), 3);
    for fixture in fixtures {
        assert!(!serde_json::to_string(&fixture.document)
            .unwrap()
            .contains("synthetic-"));
        let resolved =
            resolve_secret_patches(&fixture.active, fixture.document, &fixture.secret_patches)
                .unwrap();
        let reloaded: RouteConfigYaml =
            serde_yaml::from_str(&serde_yaml::to_string(&resolved).unwrap()).unwrap();
        let redacted = redact_route_document(&reloaded).unwrap();
        assert!(!serde_json::to_string(&redacted.document)
            .unwrap()
            .contains("synthetic-"));
        let keeps: Vec<_> = redacted
            .secrets
            .iter()
            .filter(|s| s.configured)
            .map(|s| SecretPatch::keep(&s.path))
            .collect();
        let second = resolve_secret_patches(&reloaded, redacted.document, &keeps).unwrap();
        let store = RouteConfigStore::from_document(second).unwrap();
        let providers = store.get_providers();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].credential_pool.len(), 1);
        let account = &providers[0].credential_pool[0];
        assert_eq!(account.id, "a");
        assert_eq!(account.supported_models, ["fixture-model"]);
        assert_eq!(
            account.payload.auth_mode.as_deref(),
            Some(fixture.mode.as_str())
        );
        assert_eq!(
            account.payload.base_url,
            "https://bedrock-runtime.us-west-2.amazonaws.com"
        );
        assert_eq!(
            account.payload.canonical_adapter(),
            "bedrock_converse_compatible"
        );
        assert_eq!(
            account.payload.extra_body.as_ref().unwrap()["awsRegion"],
            json!("us-west-2")
        );
        if fixture.mode == "bedrock_bearer" {
            assert_eq!(account.payload.api_key, "synthetic-new-bearer");
            assert!(!account
                .payload
                .extra_body
                .as_ref()
                .unwrap()
                .contains_key("awsAccessKeyId"));
        } else {
            assert!(account.payload.api_key.is_empty());
            assert_eq!(
                account.payload.extra_body.as_ref().unwrap()["awsAccessKeyId"],
                json!("synthetic-new-access")
            );
            assert_eq!(
                account.payload.extra_body.as_ref().unwrap()["awsSecretAccessKey"],
                json!("synthetic-new-secret")
            );
            assert_eq!(
                account
                    .payload
                    .extra_body
                    .as_ref()
                    .unwrap()
                    .get("awsSessionToken")
                    .and_then(serde_json::Value::as_str),
                if fixture.mode == "aws_sigv4_temporary" {
                    Some("synthetic-new-sts")
                } else {
                    None
                }
            );
        }
    }
}
