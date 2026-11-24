//! Additive wire format, stable plan identities and bounded monitoring configuration.
use neuro_gateway::{
    routing::config::{RouteConfigStore, RouteConfigYaml},
    site_health::SiteHealthConfig,
};
use serde_json::{json, Value};

fn config(value: Value) -> anyhow::Result<SiteHealthConfig> {
    let config: SiteHealthConfig = serde_json::from_value(value)?;
    config.validate("https://fixture.invalid/v1")?;
    Ok(config)
}

#[test]
fn old_flat_policy_roundtrips_and_new_plans_preserve_local_remote_group_separation() {
    let old = config(json!({"mode":"restrict","monitor_groups":["remote-plus"]})).unwrap();
    let encoded = serde_json::to_value(&old).unwrap();
    assert_eq!(encoded["mode"], "restrict");
    assert!(encoded.get("policy").is_none());
    let plans = json!({"profiles":[
        {"id":"plus","name":"Plus","account_group_ids":["local-plus","local-shared"],
         "policy":{"mode":"restrict","monitor_groups":["remote-plus"]}},
        {"id":"pro","name":"Pro","account_group_ids":["local-pro"],"policy":{}}
    ]});
    let parsed = config(plans).unwrap();
    let encoded = serde_json::to_value(&parsed).unwrap();
    assert_eq!(
        encoded["profiles"][0]["account_group_ids"],
        json!(["local-plus", "local-shared"])
    );
    assert_eq!(
        encoded["profiles"][0]["policy"]["monitor_groups"],
        json!(["remote-plus"])
    );
    let yaml = serde_yaml::to_string(&parsed).unwrap();
    assert_eq!(
        serde_yaml::from_str::<SiteHealthConfig>(&yaml).unwrap(),
        parsed
    );
}

#[test]
fn ambiguous_plans_auth_broadcast_and_unknown_fields_are_rejected() {
    for invalid in [
        json!({"profiles":[{"id":"p","name":"P","policy":{}},{"id":"p","name":"Q","policy":{}}]}),
        json!({"profiles":[{"id":"p","name":"P","policy":{}},{"id":"q","name":"Q","account_group_ids":["g"],"policy":{}}]}),
        json!({"profiles":[{"id":"p","name":"P","account_group_ids":["g"],"policy":{}},{"id":"q","name":"Q","account_group_ids":["g"],"policy":{}}]}),
        json!({"profiles":[{"id":"","name":"P","policy":{}}]}),
        json!({"adapter":"auto","auth":{"kind":"bearer","token":"private"}}),
        json!({"adapter":"sub2api-v2","auth":{"kind":"api_key","header":"host","token":"private"}}),
        json!({"unknown_field":true}),
        json!({"profiles":[{"id":"p","name":"P","policy":{"unknown_field":true}}]}),
    ] {
        assert!(config(invalid.clone()).is_err(), "accepted {invalid}");
    }
}

#[test]
fn actual_plan_count_not_provider_count_is_bounded() {
    let providers: Vec<_> = (0..5).map(|p| json!({
        "id":format!("p{p}"),"adapter":"openai_compatible","base_url":"https://fixture.invalid/v1",
        "site_health":{"profiles":(0..16).map(|i|json!({
            "id":format!("h{i}"),"name":"Health","account_group_ids":[format!("g{i}")],"policy":{}
        })).collect::<Vec<_>>()}
    })).collect();
    let doc: RouteConfigYaml = serde_json::from_value(json!({"providers":providers})).unwrap();
    let error = RouteConfigStore::from_document(doc).err().unwrap();
    assert!(error.to_string().contains("64"), "{error}");
}

#[test]
fn plan_activation_defaults_on_and_roundtrips_without_losing_secrets() {
    let old = config(json!({"profiles":[{"id":"p","name":"P","policy":{}}]})).unwrap();
    assert!(old.is_enabled() && old.profiles[0].policy.is_enabled());
    let mut paused = config(json!({"profiles":[{"id":"p","name":"P","policy":{
        "enabled":false,"adapter":"sub2api-v2","mode":"restrict","unknown_policy":"block",
        "auth":{"kind":"bearer","token":"monitor-secret"}
    }}]}))
    .unwrap();
    let yaml = serde_yaml::to_string(&paused).unwrap();
    assert_eq!(
        serde_yaml::from_str::<SiteHealthConfig>(&yaml).unwrap(),
        paused
    );
    assert_eq!(
        paused.profiles[0].policy.auth.token.as_deref(),
        Some("monitor-secret")
    );
    assert!(neuro_gateway::site_health::allows(
        &paused.profiles[0].policy,
        None,
        "",
        "model",
        0
    ));
    paused.profiles[0].policy.enabled = Some(true);
    assert!(!neuro_gateway::site_health::allows(
        &paused.profiles[0].policy,
        None,
        "",
        "model",
        0
    ));
    assert!(config(json!({"enabled":"false"})).is_err());
    assert!(config(json!({"enabled":null})).is_err());
    let restored = serde_json::to_value(&old).unwrap();
    assert!(restored.get("enabled").is_none());
    assert!(restored["profiles"][0]["policy"].get("enabled").is_none());
}
