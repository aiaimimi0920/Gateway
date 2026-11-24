//! A Nous subscription must have its own product, not silently reuse an OpenAI or Hermes identity.
#[test]
#[cfg(feature = "line-nous-portal-oauth")]
fn nous_portal_has_a_distinct_explicit_oauth_product() {
    let preset = neuro_gateway::preset::get_builtin_preset("nous-portal-oauth")
        .expect("Nous Portal needs its own product preset");
    assert_eq!(preset.adapter, "openai_compatible");
    assert_eq!(preset.auth_mode.as_deref(), Some("nous_portal_oauth"));
    assert!(preset.default_model.is_none());
    assert!(preset.responses_path.is_none() && preset.chat_completions_path.is_none());
}

#[test]
fn nous_product_profile_and_payload_select_the_dedicated_compilation_line() {
    use neuro_gateway::{implementation_lines as lines, protocol::registry};
    assert_eq!(
        lines::required_feature_for_protocol_profile("nous-portal-oauth"),
        Some("line-nous-portal-oauth")
    );
    assert_eq!(
        registry::default_protocol_family_for_profile("nous-portal-oauth", "openai_compatible"),
        "openai_chat"
    );
    let payload = serde_json::from_value(serde_json::json!({"adapter":"openai_compatible",
        "auth_mode":"nous_portal_oauth", "base_url":"https://inference-api.nousresearch.com/v1", "api_key":"synthetic-access"})).unwrap();
    assert_eq!(
        lines::line_for_payload(&payload).unwrap().feature_name(),
        "line-nous-portal-oauth"
    );
    assert_eq!(
        lines::ensure_payload_compiled(&payload).is_ok(),
        cfg!(feature = "line-nous-portal-oauth")
    );
    assert_eq!(
        neuro_gateway::preset::get_builtin_preset("nous-portal-oauth").is_some(),
        cfg!(feature = "line-nous-portal-oauth")
    );
}

#[test]
#[cfg(feature = "line-nous-portal-oauth")]
fn nous_product_can_create_an_empty_explicit_pool_without_synthesizing_credentials() {
    use neuro_gateway::routing::config::RouteConfigStore;
    let document = serde_json::from_value(serde_json::json!({"providers":[{
        "id":"nous", "preset":"nous-portal-oauth", "base_url":"https://inference-api.nousresearch.com/v1",
        "explicit_credentials_only":true, "credentials":[]
    }]})).unwrap();
    let store = RouteConfigStore::from_document(document).unwrap();
    assert!(store
        .snapshot()
        .resolve_candidates(Some("org/model"))
        .is_empty());
}
