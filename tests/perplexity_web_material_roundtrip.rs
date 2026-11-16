//! Explicit opt-in consumes only the synthetic frontend export, through normal secret/YAML owners.
use neuro_gateway::{
    console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch},
    protocol::perplexity_web::material as product,
    routing::config::{RouteConfigStore, RouteConfigYaml},
    upstream::perplexity_web::Session,
};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Material {
    product: String,
    token: String,
    cookie_header: String,
    user_agent: String,
    expires_at: Option<String>,
}

#[test]
#[ignore = "requires explicitly exported synthetic Perplexity frontend fixtures"]
fn perplexity_web_material_frontend_secret_yaml_prepare_roundtrip() {
    let path =
        std::env::var("GATEWAY_PERPLEXITY_MATERIAL_FIXTURE_PATH").expect("synthetic fixture path");
    let fixtures: Vec<Material> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(fixtures.len(), 3);
    for material in fixtures {
        assert_eq!(material.product, product::PRESET_ID);
        let active: RouteConfigYaml = serde_json::from_value(json!({"providers":[{
            "id":"renamable", "adapter":product::ADAPTER, "auth_mode":product::AUTH_MODE,
            "base_url":product::BASE_URL, "explicit_credentials_only":true, "credentials":[]
        }], "model_routes":[]}))
        .unwrap();
        let mut draft = serde_json::to_value(&active).unwrap();
        draft["providers"][0]["credentials"] = json!([{"id":"stable-account", "enabled":false,
            "headers":{"User-Agent":material.user_agent}, "expires_at":material.expires_at,
            "session_auth":{"transport":"cookie", "primaryCookieName":product::SESSION_COOKIE,"expiresAt":material.expires_at}}]);
        let patches = [
            SecretPatch::replace("/providers/0/credentials/0/api_key", json!(material.token)),
            SecretPatch::replace(
                "/providers/0/credentials/0/headers/Cookie",
                json!(material.cookie_header),
            ),
        ];
        let resolved =
            resolve_secret_patches(&active, serde_json::from_value(draft).unwrap(), &patches)
                .unwrap();
        let yaml = serde_yaml::to_string(&resolved).unwrap();
        let reloaded: RouteConfigYaml = serde_yaml::from_str(&yaml).unwrap();
        let redacted = redact_route_document(&reloaded).unwrap();
        let public = serde_json::to_string(&redacted.document).unwrap();
        assert!(!public.contains(&material.token) && !public.contains("synthetic-clearance"));
        let keeps: Vec<_> = redacted
            .secrets
            .iter()
            .filter(|s| s.configured)
            .map(|s| SecretPatch::keep(&s.path))
            .collect();
        let second = resolve_secret_patches(&reloaded, redacted.document, &keeps).unwrap();
        let store = RouteConfigStore::from_document(second).unwrap();
        let providers = store.get_providers();
        let account = &providers[0].credential_pool[0];
        assert_eq!(account.id, "stable-account");
        assert!(!account.enabled);
        assert_eq!(account.payload.api_key, material.token);
        assert_eq!(account.payload.headers["Cookie"], material.cookie_header);
        assert_eq!(account.payload.expires_at, material.expires_at);
        assert!(Session::from_payload(&account.payload).is_ok());
        assert!(
            store.resolve_candidates(Some("turbo")).is_empty(),
            "disabled account must stay disabled"
        );
    }
}
