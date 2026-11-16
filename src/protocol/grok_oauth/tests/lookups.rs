//! Marker-only and credential-level declarations must not escape the dedicated refresh owner.
use super::{product::document, *};
use crate::routing::config::{RouteConfigStore, RouteConfigYaml};

#[test]
fn refresh_and_discovery_are_rejected_even_without_a_preset() {
    for credential_marker in [false, true] {
        for forbidden in ["refresh_token", "refresh_endpoint", "refresh_client_id"] {
            let mut value = document();
            let provider = value["providers"][0].as_object_mut().unwrap();
            provider.remove("preset");
            provider.insert("adapter".into(), json!("openai_compatible"));
            provider.insert("responses_path".into(), json!("/responses"));
            if credential_marker {
                provider["credentials"][0]["auth_mode"] = json!(business::AUTH_MODE);
            } else {
                provider.insert("auth_mode".into(), json!(business::AUTH_MODE));
            }
            provider["credentials"][0]["refresh_token"] = json!("synthetic-refresh");
            provider["credentials"][0]["refresh_endpoint"] = json!("https://foreign.invalid/token");
            provider["credentials"][0][forbidden] = json!("private-or-foreign");
            let decoded = serde_json::from_value::<RouteConfigYaml>(value).unwrap();
            assert!(
                RouteConfigStore::from_document(decoded).is_err(),
                "must reject {forbidden}"
            );
        }
    }
}
