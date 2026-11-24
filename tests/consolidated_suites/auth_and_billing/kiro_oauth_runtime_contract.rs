//! Registered OAuth runtime is opt-in, profile-bound and never reinterpreted as a static CLI Key.
use neuro_gateway::routing::config::{RouteConfigStore, RouteConfigYaml};
use serde_json::{json, Value};

fn context() -> Value {
    json!({"auth_method":"identity_center","directory_region":"eu-west-1",
        "start_url":"https://d-synthetic.awsapps.com/start","client_id":"synthetic-client",
        "client_secret":"synthetic-secret","client_secret_expires_at":1893456000i64,
        "scopes":["codewhisperer:conversations"],"machine_id":"a".repeat(64),
        "runtime_target":{"base_url":"https://runtime.eu-central-1.kiro.dev","profile_arn":"arn:aws:codewhisperer:eu-central-1:123456789012:profile/synthetic"}})
}
fn document() -> Value {
    json!({"providers":[{"id":"oauth-runtime","preset":"kiro","auth_mode":"kiro_oauth_runtime",
        "base_url":"https://runtime.eu-central-1.kiro.dev","responses_path":"/",
        "explicit_credentials_only":true,"extra_body":{"profileArn":"arn:aws:codewhisperer:eu-central-1:123456789012:profile/synthetic"},
        "credentials":[{"id":"account","api_key":"synthetic-access","refresh_token":"synthetic-refresh",
            "kiro_oauth":context()}]}]})
}
fn compile(value: Value) -> anyhow::Result<RouteConfigStore> {
    RouteConfigStore::from_document(serde_json::from_value::<RouteConfigYaml>(value)?)
}

#[cfg(feature = "line-kiro-official-vendor-api")]
#[test]
fn kiro_oauth_runtime_compiles_with_separate_directory_and_profile_regions() {
    let store = compile(document()).expect("explicit registered OAuth runtime should compile");
    assert_eq!(store.snapshot().provider_count(), 1);
}

#[cfg(feature = "line-kiro-official-vendor-api")]
#[test]
fn kiro_oauth_runtime_empty_template_pool_compiles_without_inventing_registration() {
    let mut value = document();
    value["providers"][0]["credentials"] = json!([]);
    let store = compile(value).unwrap();
    assert!(store
        .snapshot()
        .resolve_candidates(Some("synthetic-model"))
        .is_empty());
}

#[test]
fn kiro_oauth_runtime_rejects_missing_registration_and_cross_product_overrides() {
    for (name, value) in [
        ("kiro_oauth", Value::Null),
        ("auth_mode", json!("kiro_api_key")),
        ("base_url", json!("https://runtime.us-east-1.kiro.dev")),
    ] {
        let mut value_document = document();
        value_document["providers"][0]["credentials"][0][name] = value;
        assert!(compile(value_document).is_err(), "{name}");
    }
}

#[test]
fn kiro_oauth_runtime_requires_explicit_matching_profile_and_fixed_audience() {
    for (name, value) in [
        ("extra_body", json!({})),
        (
            "extra_body",
            json!({"profileArn":"arn:aws:codewhisperer:us-east-1:123456789012:profile/other"}),
        ),
        ("extra_body", json!({"profileArn":42})),
        (
            "base_url",
            json!("https://runtime.eu-central-1.kiro.dev.evil.invalid"),
        ),
        ("base_url", json!("https://runtime.eu-west-1.kiro.dev")),
        ("responses_path", json!("/generateAssistantResponse")),
        ("headers", json!({"Authorization":"foreign"})),
    ] {
        let mut doc = document();
        doc["providers"][0][name] = value;
        assert!(compile(doc).is_err(), "{name}");
    }
}

#[cfg(feature = "line-kiro-official-vendor-api")]
#[test]
fn kiro_oauth_runtime_profile_keep_is_bound_even_with_the_same_region() {
    use neuro_gateway::console::secrets::{
        redact_route_document, resolve_secret_patches, SecretPatch,
    };
    let original: RouteConfigYaml = serde_json::from_value(document()).unwrap();
    let redacted = redact_route_document(&original).unwrap();
    let patches: Vec<_> = redacted
        .secrets
        .iter()
        .map(|s| SecretPatch::keep(&s.path))
        .collect();
    let mut draft = redacted.document;
    draft.providers[0].extra_body.insert(
        "profileArn".into(),
        json!("arn:aws:codewhisperer:eu-central-1:123456789012:profile/other"),
    );
    assert!(resolve_secret_patches(&original, draft, &patches).is_err());
}

#[cfg(not(feature = "line-kiro-official-vendor-api"))]
#[test]
fn kiro_oauth_runtime_compiled_out_cannot_create_a_sender() {
    assert!(compile(document()).is_err());
}
