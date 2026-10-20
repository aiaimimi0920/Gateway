//! Optional product channel must reuse the credential identity, never replace its source.
use neuro_gateway::routing::config::RouteConfigStore;
use serde_json::json;

#[test]
fn explicit_credential_switch_changes_only_the_selected_channel() {
    let document = json!({"providers":[{
        "id":"codex-fixture", "preset":"codex", "base_url":"https://chatgpt.com/backend-api/codex",
        "supported_models":["native","special"], "credentials":[
            {"id":"native", "supported_models":["native"], "api_key":"fixture-token", "headers":{"Chatgpt-Account-Id":"fixture-account"}},
            {"id":"special", "supported_models":["special"], "api_key":"fixture-token", "headers":{"Chatgpt-Account-Id":"fixture-account"},
             "codex_basispoints_enabled":true}
        ]
    }]});
    let store = RouteConfigStore::from_document(serde_json::from_value(document).unwrap()).unwrap();
    let native = store
        .snapshot()
        .resolve_candidates(Some("native"))
        .remove(0);
    let special = store
        .snapshot()
        .resolve_candidates(Some("special"))
        .remove(0);
    assert_eq!(native.payload.adapter, "openai_compatible");
    assert_eq!(special.payload.adapter, "basispoints_compatible");
    assert_eq!(native.payload.api_key, special.payload.api_key);
    assert_eq!(special.payload.credential_id.as_deref(), Some("special"));
    assert!(special
        .supported_protocol_families
        .iter()
        .any(|f| f == "openai_responses"));
}

#[test]
fn explicit_native_credential_overrides_the_provider_switch_and_arbitrary_endpoints_are_rejected() {
    let mut document = json!({"providers":[{"id":"pool","preset":"codex", "base_url":"https://chatgpt.com/backend-api/codex",
        "codex_basispoints_enabled":true, "supported_models":["model"], "credentials":[{"id":"native","api_key":"fixture","codex_basispoints_enabled":false}]}]});
    let store =
        RouteConfigStore::from_document(serde_json::from_value(document.clone()).unwrap()).unwrap();
    assert_eq!(
        store.resolve_candidates(Some("model"))[0].adapter,
        "openai_compatible"
    );
    document["providers"][0]["base_url"] = json!("https://arbitrary.invalid/v1");
    assert!(RouteConfigStore::from_document(serde_json::from_value(document).unwrap()).is_err());
}

#[test]
fn request_plan_uses_fixed_product_origin_without_rewriting_the_credential_source() {
    use neuro_gateway::{
        protocol::responses::normalize_responses, upstream::client::UpstreamClient,
    };
    let document = json!({"providers":[{"id":"pool","preset":"codex","base_url":"https://chatgpt.com/backend-api/codex",
        "codex_basispoints_enabled":true,"api_key":"fixture"}]});
    let store = RouteConfigStore::from_document(serde_json::from_value(document).unwrap()).unwrap();
    let candidate = store.resolve_candidates(Some("gpt-fixture")).remove(0);
    let req = normalize_responses(json!({"model":"gpt-fixture","input":"hello"})).unwrap();
    let plan =
        UpstreamClient::build_request_plan(&candidate.payload, &req, "gpt-fixture", false).unwrap();
    assert_eq!(plan.url, "https://bps.openai.com/basispoints/api/responses");
    assert_eq!(
        candidate.payload.base_url,
        "https://chatgpt.com/backend-api/codex"
    );
    assert_eq!(plan.body.unwrap()["store"], false);
}

#[test]
fn site_health_configuration_survives_the_existing_document_contract() {
    let source = json!({"providers":[{"id":"site", "preset":"openai", "base_url":"https://fixture.invalid/v1",
        "api_key":"fixture", "site_health":{"mode":"restrict", "minimum_success_rate":95.0}}]});
    let typed: neuro_gateway::routing::config::RouteConfigYaml =
        serde_json::from_value(source).unwrap();
    let document = serde_json::to_value(typed).unwrap();
    assert_eq!(document["providers"][0]["site_health"]["mode"], "restrict");
}
