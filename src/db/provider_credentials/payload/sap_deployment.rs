//! SAP deployment declarations are not credential overrides in Hosted routing.
use super::*;
use crate::protocol::sap_aicore::{
    AUTH_MODE, BEDROCK_CONVERSE, BEDROCK_IMAGE_INPUT, IMAGE_INPUT, NATIVE_RESPONSES,
    NATIVE_RESPONSES_IMAGE_INPUT, O3_MINI, ORCHESTRATION_IMAGE_INPUT, ORCHESTRATION_MODEL,
    ORCHESTRATION_REASONING, STORED_RESPONSES,
};

pub(super) fn is_sap(layer: &Map<String, Value>) -> bool {
    layer.get("auth_mode").and_then(Value::as_str) == Some(AUTH_MODE)
}

// Preserve duplicate aliases in one layer for the typed reader to reject, as with call_flow.
pub(super) fn normalize_settings(
    mut layer: Cow<'_, Map<String, Value>>,
) -> Cow<'_, Map<String, Value>> {
    if layer.contains_key("extraBody") && !layer.contains_key("extra_body") {
        let layer = layer.to_mut();
        if let Some(value) = layer.remove("extraBody") {
            layer.insert("extra_body".into(), value);
        }
    }
    layer
}

pub(super) fn inherit_settings(
    account: &Map<String, Value>,
    credential: &mut Cow<'_, Map<String, Value>>,
) {
    // A credential serialized with extra_body: null does not declare deployment settings.
    if account.get("extra_body").is_some_and(Value::is_object)
        && credential.get("extra_body").is_some_and(Value::is_null)
        && !credential.contains_key("extraBody")
    {
        credential.to_mut().remove("extra_body");
    }
}

pub(super) fn preserve_capabilities(
    account: &Map<String, Value>,
    merged: &mut Map<String, Value>,
) -> bool {
    for mode in [ORCHESTRATION_MODEL, BEDROCK_CONVERSE] {
        let declared = is_sap(account)
            .then(|| account.get("extra_body").and_then(|v| v.get(mode)))
            .flatten();
        if let Some(declared) = declared {
            // A protocol mode owns a different endpoint. Never downgrade it to Azure Chat
            // or silently rebind a credential to another deployment/resource group.
            if !is_sap(merged)
                || !same_value(deployment(account), deployment(merged))
                || !same_value(group(account), group(merged))
                || version(merged).is_some()
            {
                return false;
            }
            let Some(settings) = merged.get_mut("extra_body").and_then(Value::as_object_mut) else {
                return false;
            };
            settings.insert(mode.into(), declared.clone());
        } else if let Some(settings) = merged.get_mut("extra_body").and_then(Value::as_object_mut) {
            settings.remove(mode);
        }
    }
    let same_binding = is_sap(account)
        && is_sap(merged)
        && same_value(deployment(account), deployment(merged))
        && same_value(group(account), group(merged));
    let same_azure_binding = same_binding && same_value(version(account), version(merged));
    // Converse has no Azure API version. The mode guard above already rejects rebinding
    // and any credential that attempts to introduce an Azure version.
    let same_bedrock_binding = same_binding
        && version(merged).is_none()
        && account
            .get("extra_body")
            .and_then(|v| v.get(BEDROCK_CONVERSE))
            == Some(&Value::Bool(true));
    let same_orchestration_binding = same_binding
        && version(merged).is_none()
        && account
            .get("extra_body")
            .and_then(|v| v.get(ORCHESTRATION_MODEL))
            .is_some_and(Value::is_string);
    if let Some(settings) = merged.get_mut("extra_body").and_then(Value::as_object_mut) {
        for flag in [
            IMAGE_INPUT,
            O3_MINI,
            NATIVE_RESPONSES,
            NATIVE_RESPONSES_IMAGE_INPUT,
            STORED_RESPONSES,
            BEDROCK_IMAGE_INPUT,
            ORCHESTRATION_IMAGE_INPUT,
            ORCHESTRATION_REASONING,
        ] {
            if let Some(declared) =
                account
                    .get("extra_body")
                    .and_then(|v| v.get(flag))
                    .filter(|_| match flag {
                        BEDROCK_IMAGE_INPUT => same_bedrock_binding,
                        ORCHESTRATION_IMAGE_INPUT | ORCHESTRATION_REASONING => {
                            same_orchestration_binding
                        }
                        _ => same_azure_binding,
                    })
            {
                settings.insert(flag.into(), declared.clone());
            } else {
                settings.remove(flag);
            }
        }
    }
    true
}

fn deployment(layer: &Map<String, Value>) -> Option<&Value> {
    match (layer.get("base_url"), layer.get("baseUrl")) {
        (Some(value), None) | (None, Some(value)) => Some(value),
        _ => None,
    }
}

fn group(layer: &Map<String, Value>) -> Option<&Value> {
    layer.get("headers")?.get("AI-Resource-Group")
}

fn version(layer: &Map<String, Value>) -> Option<&Value> {
    layer.get("extra_body")?.get("sap_api_version")
}

fn same_value(source: Option<&Value>, merged: Option<&Value>) -> bool {
    source.is_some_and(Value::is_string) && source == merged
}
