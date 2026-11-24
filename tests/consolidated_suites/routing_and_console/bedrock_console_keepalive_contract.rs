//! A keepalive service token is an audience-bound secret too; all material is synthetic.
use neuro_gateway::console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch};
use neuro_gateway::routing::config::RouteConfigYaml;
use serde_json::{json, Value};

fn fixture(shared: bool) -> RouteConfigYaml {
    let keepalive =
        json!({"serviceUrl":"https://old-service.invalid", "authToken":"synthetic-service-token"});
    serde_json::from_value(json!({"providers":[{"id":"p", "preset":"bedrock-converse", "auth_mode":"bedrock_bearer",
        "base_url":"https://bedrock-runtime.us-east-1.amazonaws.com",
        "keepalive": if shared {keepalive.clone()} else {Value::Null},
        "credentials":[{"id":"a", "api_key":"synthetic-bearer", "extra_body":{"awsRegion":"us-east-1"},
            "keepalive": if shared {Value::Null} else {keepalive}}]}]})).unwrap()
}

fn change_destination(shared: bool) {
    for (field, value) in [
        ("serviceUrl", "https://new-service.invalid"),
        ("ensurePath", "/different"),
    ] {
        let active = fixture(shared);
        let redacted = redact_route_document(&active).unwrap();
        let mut draft = serde_json::to_value(redacted.document).unwrap();
        let owner = if shared {
            "/providers/0"
        } else {
            "/providers/0/credentials/0"
        };
        draft.pointer_mut(owner).unwrap()["keepalive"][field] = json!(value);
        let patches: Vec<_> = redacted
            .secrets
            .iter()
            .filter(|s| s.configured)
            .map(|s| {
                if s.path.ends_with("/keepalive/authToken") {
                    SecretPatch::keep(&s.path)
                } else {
                    SecretPatch::replace(&s.path, json!("synthetic-new-material"))
                }
            })
            .collect();
        let error = resolve_secret_patches(&active, serde_json::from_value(draft).unwrap(), &patches)
            .expect_err("replacing the Bedrock key does not authorize reusing a service token at another audience");
        assert_eq!(error.code(), "secret_keep_destination_changed");
    }
}

#[test]
fn bedrock_provider_keepalive_token_cannot_be_kept_at_another_service() {
    change_destination(true);
}

#[test]
fn bedrock_credential_keepalive_token_cannot_be_kept_at_another_service() {
    change_destination(false);
}

#[test]
fn bedrock_shared_keepalive_uses_whole_object_inheritance() {
    let active = fixture(true);
    let redacted = redact_route_document(&active).unwrap();
    let mut draft = serde_json::to_value(&redacted.document).unwrap();
    // A new service object, including a separately replaced token, does not borrow the provider object.
    draft["providers"][0]["credentials"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"new",
        "api_key":null, "keepalive":{"serviceUrl":"https://new-service.invalid"}}));
    let mut patches: Vec<_> = redacted
        .secrets
        .iter()
        .filter(|s| s.configured)
        .map(|s| SecretPatch::keep(&s.path))
        .collect();
    patches.push(SecretPatch::replace(
        "/providers/0/credentials/1/keepalive/authToken",
        json!("synthetic-new-service-token"),
    ));
    patches.push(SecretPatch::replace(
        "/providers/0/credentials/1/api_key",
        json!("synthetic-new-key"),
    ));
    assert!(
        resolve_secret_patches(&active, serde_json::from_value(draft).unwrap(), &patches).is_ok()
    );
}
