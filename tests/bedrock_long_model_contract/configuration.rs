use neuro_gateway::{
    console::document::validate_route_document,
    routing::config::{RouteConfigStore, RouteConfigYaml},
};
use serde_json::{json, Value};

pub fn model(length: usize) -> String {
    let prefix = "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/";
    format!("{prefix}{}", "A".repeat(length - prefix.len()))
}
pub fn policy(model: &str) -> Value {
    json!({"automaticEnabled":true,"intervalMinutes":60,"modelSelection":"selected","models":[model],
        "cases":[{"id":"case","name":"case","prompt":"Reply OK","difficulty":1,"enabled":true}]})
}
pub fn document() -> Value {
    json!({"providers":[{"id":"bedrock","preset":"bedrock-converse","base_url":"https://bedrock-runtime.us-east-1.amazonaws.com",
        "explicit_credentials_only":true,"credentials":[{"id":"one","api_key":"synthetic-only"}]}],"model_routes":[],"aliases":{}})
}
fn compile(value: Value) -> bool {
    let document: RouteConfigYaml = match serde_json::from_value(value) {
        Ok(value) => value,
        Err(_) => return false,
    };
    validate_route_document(document.clone()).is_ok()
        && RouteConfigStore::from_document(document).is_ok()
}

#[test]
fn long_bedrock_selection_survives_compilation_without_normalizing_identity() {
    let mut value = document();
    let arn = model(2048);
    value["providers"][0]["credentials"][0]["model_selection"] =
        json!({"manual":[arn],"fetched":[]});
    let store =
        RouteConfigStore::from_document(serde_json::from_value(value.clone()).unwrap()).unwrap();
    let provider = &store.get_providers()[0];
    assert_eq!(provider.credential_pool[0].supported_models, [arn.clone()]);
    assert_eq!(provider.supported_models, [arn]);
    value["providers"][0]["preset"] = json!("openai");
    assert!(!compile(value));
}

#[test]
fn long_mapping_targets_have_a_structure_gate_then_a_product_gate() {
    let mut value = document();
    let arn = model(1500);
    value["providers"][0]["model_map_targets"] = json!({"alias":[arn.clone(),"short"]});
    assert!(compile(value.clone()));
    value["providers"][0]["preset"] = json!("openai");
    assert!(!compile(value.clone()));
    value["providers"][0]["model_map_targets"] = json!({"alias":[model(2049)]});
    assert!(!compile(value));
}

#[test]
fn all_policy_owners_accept_long_models_only_for_bedrock() {
    for owner in [
        "test_policy",
        "default_account_test_policy",
        "subpool",
        "account",
        "named",
    ] {
        let mut value = document();
        let p = &mut value["providers"][0];
        match owner {
            "account" => p["credentials"][0]["test_policy"] = policy(&model(1500)),
            "subpool" => {
                p["credentials"][0]["credential_identity_category_id"] = json!("group");
                p["subpool_test_policies"] = json!({"group":policy(&model(1500))});
            }
            "named" => {
                p["test_plans"] = json!([{"id":"named","name":"Named","scopes":[{"kind":"account","id":"one"}],"policy":policy(&model(1500))}])
            }
            field => p[field] = policy(&model(1500)),
        }
        assert!(compile(value.clone()), "{owner}");
        value["providers"][0]["preset"] = json!("openai");
        assert!(!compile(value), "{owner}");
    }
}

#[test]
fn extended_identity_never_relaxes_other_ids_or_bedrock_grammar() {
    for invalid in [
        model(2049),
        format!("{}/../bad", model(1400)),
        format!("{}?bad", model(1400)),
        format!("{}模", model(1400)),
    ] {
        let mut value = document();
        value["providers"][0]["test_policy"] = policy(&invalid);
        assert!(!compile(value));
    }
    let mut value = document();
    let mut test = policy(&model(1500));
    test["cases"][0]["id"] = json!("a".repeat(257));
    value["providers"][0]["test_policy"] = test;
    assert!(!compile(value));
    let mut value = document();
    value["providers"][0]["test_plans"] = json!([{"id":"a".repeat(257),"name":"Named","scopes":[{"kind":"pool"}],"policy":policy("short")}]);
    assert!(!compile(value));
}

#[test]
fn named_and_subpool_policies_use_each_final_payload_not_the_provider_preset() {
    let mut value = document();
    let base = "https://bedrock-runtime.us-east-1.amazonaws.com";
    value["providers"][0]["credentials"][0]["credential_identity_category_id"] = json!("native");
    value["providers"][0]["credentials"].as_array_mut().unwrap().push(json!({
        "id":"two","api_key":"other-synthetic","credential_identity_category_id":"discovered",
        "discovery":{"source_url":base,"api_base":format!("{base}/v1"),"protocol":"chat_completions",
            "models":["short"],"verified_models":["short"],"checked_at":"now",
            "binding":neuro_gateway::provider_discovery::binding(base,"other-synthetic")}
    }));
    let long_policy = policy(&model(1500));
    value["providers"][0]["test_plans"] = json!([{"id":"named","name":"Named","scopes":[{"kind":"account","id":"one"}],"policy":long_policy}]);
    assert!(compile(value.clone()));
    let arn = model(1500);
    value["providers"][0]["credentials"][0]["model_selection"] =
        json!({"manual":[arn],"fetched":[]});
    let mixed =
        RouteConfigStore::from_document(serde_json::from_value(value.clone()).unwrap()).unwrap();
    for _ in 0..6 {
        let candidates = mixed.resolve_candidates(Some(&arn));
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].payload.credential_id.as_deref(), Some("one"));
    }
    value["providers"][0]["test_plans"][0]["scopes"] = json!([{"kind":"account","id":"two"}]);
    assert!(!compile(value.clone()));
    value["providers"][0]["test_plans"] = json!([]);
    value["providers"][0]["subpool_test_policies"] = json!({"native":long_policy});
    assert!(compile(value.clone()));
    value["providers"][0]["subpool_test_policies"] = json!({"discovered":long_policy});
    assert!(!compile(value.clone()));
    value["providers"][0]["subpool_test_policies"] = json!({});
    value["providers"][0]["test_policy"] = long_policy;
    assert!(!compile(value));
}
