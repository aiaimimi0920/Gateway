//! Credential grouping must not depend on display-model or JSON field ordering.

use super::display_metadata::{
    derive_credential_material_key, read_selected_display_model, read_supported_models,
};
use serde_json::json;

#[test]
fn credential_material_grouping_ignores_models_but_distinguishes_secrets() {
    let first = json!({
        "apiKey": "synthetic-secret-a",
        "extraBody": {"appId": "app-a", "uid": "user-a"},
        "model": "model-a"
    });
    let reordered = json!({
        "model": "model-b",
        "extraBody": {"uid": "user-a", "appId": "app-a"},
        "apiKey": " synthetic-secret-a "
    });
    let mut another = reordered.clone();
    another["apiKey"] = json!("synthetic-secret-b");
    let key = derive_credential_material_key(&first).unwrap();
    assert!(key.starts_with("credmat:"));
    assert_eq!(key.len(), 24);
    assert_eq!(
        Some(key.clone()),
        derive_credential_material_key(&reordered)
    );
    assert_ne!(Some(key), derive_credential_material_key(&another));
    assert_eq!(
        derive_credential_material_key(&json!({
            "credentialMaterialKey": " family-a ", "apiKey": "synthetic-secret-a"
        })),
        Some("family-a".to_string())
    );
}

#[test]
fn display_metadata_preserves_alias_priority_and_model_order() {
    let payload = json!({
        "selectedDisplayModel": " selected-a ",
        "displayModel": "fallback-a",
        "supportedModels": ["model-b", " model-a, model-b\nmodel-c", 8],
        "allowed_models": ["model-a", "Model-A"],
        "defaultModel": "model-d"
    });
    assert_eq!(
        read_selected_display_model(&payload).as_deref(),
        Some("selected-a")
    );
    assert_eq!(
        read_supported_models(&payload),
        vec!["model-b", "model-a", "model-c", "Model-A", "model-d"]
    );
}
