//! Product registry and final configuration admission; synthetic material, no network.
use neuro_gateway::{
    implementation_lines as lines,
    preset::get_builtin_preset,
    protocol::registry,
    routing::{candidate::ProviderAccountPayload, config::RouteConfigStore},
};
use serde_json::{json, Value};
const BASE: &str = "https://agents.lumalabs.ai/v1";
const ADAPTER: &str = "lumalabs_agents_compatible";
const PROFILE: &str = "lumalabs_agents_api";
const FEATURE: &str = "line-lumalabs-agents-official-api";

fn provider() -> Value {
    json!({"id":"renamed-product","preset":"lumalabs-api","base_url":BASE,
        "explicit_credentials_only":true,"credentials":[{"id":"stable-a","api_key":"luma-api-synthetic-only",
        "supported_models":["uni-1","uni-1-max","ray-3.2"]}]})
}
fn compile(provider: Value) -> anyhow::Result<RouteConfigStore> {
    RouteConfigStore::from_document(
        serde_json::from_value(json!({"providers":[provider]})).unwrap(),
    )
}
#[test]
fn luma_agents_registry_and_origin_inference_are_not_the_old_web_product() {
    assert_eq!(
        registry::default_protocol_profile_for_preset("lumalabs-api"),
        PROFILE
    );
    assert_eq!(
        registry::default_protocol_profile_for_adapter(ADAPTER),
        PROFILE
    );
    assert_eq!(
        registry::infer_protocol_profile(ADAPTER, None, Some(BASE)),
        PROFILE
    );
    assert_eq!(
        registry::canonicalize_protocol_profile_key("lumalabs-api"),
        PROFILE
    );
    assert_eq!(
        registry::default_protocol_family_for_profile(PROFILE, ADAPTER),
        "lumalabs_agents_media"
    );
    assert_eq!(
        registry::default_protocol_profile_for_preset("lumalabs"),
        "lumalabs"
    );
    assert_eq!(
        registry::infer_protocol_profile(
            "lumalabs_compatible",
            None,
            Some("https://app.lumalabs.ai")
        ),
        "lumalabs"
    );
}
#[test]
fn luma_agents_line_and_preset_obey_their_own_compile_feature() {
    assert_eq!(lines::required_feature_for_adapter(ADAPTER), Some(FEATURE));
    assert_eq!(
        lines::required_feature_for_protocol_profile(PROFILE),
        Some(FEATURE)
    );
    let payload: ProviderAccountPayload =
        serde_json::from_value(json!({"adapter":ADAPTER,"base_url":BASE,"api_key":""})).unwrap();
    let enabled = cfg!(feature = "line-lumalabs-agents-official-api");
    assert_eq!(lines::ensure_payload_compiled(&payload).is_ok(), enabled);
    assert_eq!(lines::is_protocol_profile_compiled_in(PROFILE), enabled);
    assert_eq!(get_builtin_preset("lumalabs-api").is_some(), enabled);
    if let Some(preset) = get_builtin_preset("lumalabs-api") {
        assert_eq!(preset.adapter, ADAPTER);
        assert_eq!(preset.auth_mode.as_deref(), Some("bearer"));
        assert!(preset.default_model.is_none());
        assert!(preset.session_auth.is_none());
        assert!(preset.headers.is_empty());
    }
}
#[cfg(feature = "line-lumalabs-agents-official-api")]
#[test]
fn luma_agents_normal_static_accounts_empty_template_and_cleared_key_compile() {
    let store = compile(provider()).unwrap();
    let providers = store.get_providers();
    let p = &providers[0];
    assert_eq!(p.protocol_profile, PROFILE);
    assert_eq!(p.protocol_family, "lumalabs_agents_media");
    assert_eq!(p.credential_pool[0].payload.adapter, ADAPTER);
    assert!(p.credential_pool[0].refresh_config.is_none());
    let mut value = provider();
    value["credentials"] = json!([]);
    assert!(compile(value).unwrap().get_providers()[0]
        .credential_pool
        .is_empty());
    let mut value = provider();
    value["credentials"][0]["api_key"] = Value::Null;
    assert!(
        compile(value).unwrap().get_providers()[0].credential_pool[0]
            .payload
            .api_key
            .is_empty()
    );
    let mut raw = provider();
    raw.as_object_mut().unwrap().remove("preset");
    raw["adapter"] = json!(ADAPTER);
    raw["protocol_profile"] = json!(PROFILE);
    raw["protocol_family"] = json!("lumalabs_agents_media");
    assert!(compile(raw).is_ok());
    let mut auto = provider();
    auto["call_flow"] =
        json!({"call_adapter":"auto", "fingerprint_adapter":"auto", "special_adapter":"auto"});
    auto["request_client_profile"] = json!("auto");
    assert!(compile(auto).is_ok());
}
#[cfg(feature = "line-lumalabs-agents-official-api")]
#[test]
fn luma_agents_raw_static_identity_cannot_acquire_refresh_material() {
    let mut value = provider();
    value.as_object_mut().unwrap().remove("preset");
    value["adapter"] = json!(ADAPTER);
    value["credentials"][0]["refresh_token"] = json!("private-refresh");
    assert!(
        compile(value).is_err(),
        "static Agents Key incorrectly acquired generic refresh"
    );
}
#[cfg(feature = "line-lumalabs-agents-official-api")]
#[test]
fn luma_agents_original_identity_and_final_account_merge_reject_foreign_surfaces() {
    for (field, value) in [
        ("adapter", json!("openai_compatible")),
        ("protocol_profile", json!("lumalabs")),
        ("protocol_family", json!("openai_chat")),
        ("base_url", json!("https://app.lumalabs.ai")),
        ("call_flow", json!({"call_adapter":"openai"})),
        ("request_client_profile", json!("codex_cli")),
        ("headers", json!({"Cookie":"private-session"})),
        ("extra_body", json!({"realmId":"web-only"})),
        ("chat_completions_path", json!("/another")),
        ("execution_mode", json!("browser_backed")),
        ("supported_models", json!(["wrong-model"])),
        ("default_model", json!("wrong-model")),
    ] {
        let mut bad = provider();
        bad[field] = value;
        assert!(compile(bad).is_err(), "accepted provider {field}");
    }
    for (field, value) in [
        ("base_url", json!("https://other.example/v1")),
        ("auth_mode", json!("session")),
        ("auth_token", json!("web-session")),
        ("refresh_token", json!("rt")),
        ("refresh_endpoint", json!("https://other.example/token")),
        ("refresh_client_id", json!("client")),
        ("token_expires_in_secs", json!(3600)),
        ("session_auth", json!({"transport":"cookie"})),
        ("headers", json!({"Cookie":"private"})),
        ("extra_body", json!({"realmId":"private"})),
        ("request_client_profile", json!("cpa")),
        ("execution_mode", json!("browser_backed")),
        ("api_key", json!("wos-session=web")),
        ("supported_models", json!(["wrong-model"])),
        (
            "model_selection",
            json!({"fetched":[],"manual":["wrong-model"]}),
        ),
    ] {
        let mut bad = provider();
        bad["credentials"][0][field] = value;
        assert!(compile(bad).is_err(), "accepted credential {field}");
    }
    let mut erased = provider();
    erased["adapter"] = json!("openai_compatible");
    erased["base_url"] = json!("https://other.example/v1");
    erased["credentials"][0]["api_key"] = json!("");
    assert!(
        compile(erased).is_err(),
        "original preset disappeared before final validation"
    );
}
