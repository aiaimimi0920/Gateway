//! Retiring a site preset must not prevent using that site through the generic adapter.
use neuro_gateway::{
    preset::get_builtin_preset,
    routing::config::{RouteConfigStore, RouteConfigYaml},
};
use serde_json::json;

fn provider(preset: Option<&str>) -> RouteConfigYaml {
    serde_json::from_value(json!({"providers": [{
        "id": "muyuan-custom",
        "preset": preset,
        "adapter": "openai_compatible",
        "protocol_profile": "openai_compatible_generic",
        "base_url": "https://muyuan.do/v1",
        "supported_models": ["fixture-model"],
        "credentials": [{"id": "custom-account", "api_key": "synthetic-fixture"}]
    }]}))
    .unwrap()
}

#[test]
fn retired_muyuan_preset_is_not_registered_or_loadable() {
    assert!(get_builtin_preset("muyuan-openai").is_none());
    let error = match RouteConfigStore::from_document(provider(Some("muyuan-openai"))) {
        Ok(_) => panic!("retired preset unexpectedly accepted"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("Unknown preset 'muyuan-openai'"));
}

#[test]
fn muyuan_custom_provider_remains_loadable() {
    let store = RouteConfigStore::from_document(provider(None)).unwrap();
    let snapshot = store.snapshot();
    assert_eq!(snapshot.provider_count(), 1);
    assert!(snapshot.has_routes());
}
