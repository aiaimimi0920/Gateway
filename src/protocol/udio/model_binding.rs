//! Bind the configured routing identity to legacy Web wire, without inventing a model catalogue.
use super::{UDIO_DEFAULT_MODEL, UDIO_DEFAULT_MODEL_TYPE};
use crate::error::GatewayError;
use serde_json::{Map, Value};

const MODEL_KEYS: [&str; 4] = [
    "model_type",
    "modelType",
    "upstream_model_type",
    "upstreamModelType",
];

pub(super) fn bound_gen_params(
    body: &Map<String, Value>,
    resolved_model: &str,
) -> Result<Map<String, Value>, GatewayError> {
    // This is the existing operation alias, not evidence of a current native model/version.
    let native_model = if resolved_model == UDIO_DEFAULT_MODEL {
        UDIO_DEFAULT_MODEL_TYPE
    } else {
        resolved_model
    };
    if native_model.is_empty()
        || native_model.len() > 128
        || native_model
            .chars()
            .any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(invalid_binding());
    }
    let mut params = match body.get("gen_params") {
        None => Map::new(),
        Some(Value::Object(params)) => params.clone(),
        Some(_) => return Err(invalid_binding()),
    };
    // Check every spelling; first-match precedence must not hide a conflicting identity.
    for object in [body, &params] {
        for key in MODEL_KEYS {
            if let Some(value) = object.get(key) {
                if value.as_str() != Some(native_model) {
                    return Err(invalid_binding());
                }
            }
        }
    }
    for key in MODEL_KEYS {
        params.remove(key);
    }
    params.insert("model_type".into(), Value::String(native_model.into()));
    Ok(params)
}

fn invalid_binding() -> GatewayError {
    GatewayError::bad_request("Udio model parameters must match the routed model; details omitted.")
        .with_provider("udio_compatible")
        .with_code("udio_model_binding_invalid")
}
