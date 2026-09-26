//! Credential family fingerprints and ordered model/display metadata.

use serde_json::Value;
use sha2::{Digest, Sha256};

pub(super) fn shared_provider_payload_hints(payload: &Value) -> Value {
    serde_json::json!({
        "baseUrl": payload.get("baseUrl").cloned().or_else(|| payload.get("base_url").cloned()),
        "defaultModel": payload.get("defaultModel").cloned().or_else(|| payload.get("default_model").cloned()),
        "accountLabel": payload.get("accountLabel").cloned().or_else(|| payload.get("account_label").cloned()),
    })
}

pub(super) fn read_selected_display_model(payload: &Value) -> Option<String> {
    let object = payload.as_object()?;
    [
        "selectedDisplayModel",
        "selectedModelDisplayName",
        "resolvedDisplayModel",
        "boundDisplayModel",
        "boundModelLabel",
        "displayModel",
    ]
    .iter()
    .find_map(|key| object.get(*key).and_then(Value::as_str))
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .map(str::to_string)
}

pub(super) fn read_supported_models(payload: &Value) -> Vec<String> {
    fn push_text_like(value: &Value, values: &mut Vec<String>) {
        match value {
            Value::String(text) => {
                for item in text
                    .split([',', '\n'])
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                {
                    values.push(item.to_string());
                }
            }
            Value::Array(items) => {
                for item in items {
                    push_text_like(item, values);
                }
            }
            _ => {}
        }
    }

    let Some(object) = payload.as_object() else {
        return Vec::new();
    };

    let mut values = Vec::new();
    for key in [
        "supportedModels",
        "supported_models",
        "allowedModels",
        "allowed_models",
        "modelCode",
        "model_code",
        "modelId",
        "model_id",
        "defaultModel",
        "default_model",
        "model",
    ] {
        if let Some(value) = object.get(key) {
            push_text_like(value, &mut values);
        }
    }

    let mut unique = Vec::new();
    for value in values {
        if !unique.contains(&value) {
            unique.push(value);
        }
    }
    unique
}

pub(super) fn derive_credential_material_key(payload: &Value) -> Option<String> {
    let object = payload.as_object()?;
    if let Some(explicit) = [
        "credentialMaterialKey",
        "sharedCredentialKey",
        "credentialFamilyKey",
        "materialKey",
    ]
    .iter()
    .find_map(|key| object.get(*key).and_then(Value::as_str))
    {
        let trimmed = explicit.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    let extra = object
        .get("extraBody")
        .or_else(|| object.get("extra_body"))
        .and_then(Value::as_object);
    let mut components: Vec<(&str, String)> = Vec::new();
    for key in [
        "apiKey",
        "api_key",
        "authToken",
        "auth_token",
        "apiSecret",
        "api_secret",
    ] {
        if let Some(value) = object.get(key).and_then(Value::as_str) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                components.push((key, trimmed.to_string()));
            }
        }
    }
    for key in ["appId", "app_id", "uid"] {
        if let Some(value) = extra.and_then(|map| map.get(key)).and_then(Value::as_str) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                components.push((key, trimmed.to_string()));
            }
        }
    }
    if components.is_empty() {
        return None;
    }

    components.sort_by(|left, right| left.0.cmp(right.0));
    let mut hasher = Sha256::new();
    for (key, value) in components {
        hasher.update(key.as_bytes());
        hasher.update(b"=");
        hasher.update(value.as_bytes());
        hasher.update(b"\n");
    }
    let digest = hasher.finalize();
    let mut fingerprint = String::with_capacity(16);
    for byte in digest.iter().take(8) {
        use std::fmt::Write as _;
        let _ = write!(&mut fingerprint, "{byte:02x}");
    }
    Some(format!("credmat:{fingerprint}"))
}
