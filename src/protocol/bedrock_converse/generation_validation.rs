//! Reject malformed controls before network admission; never put caller values in errors.
use super::generation::{self, FIELDS};
use crate::{error::GatewayError, protocol::canonical::CanonicalRelayRequest};
use serde_json::Value;

fn invalid(field: &'static str) -> GatewayError {
    GatewayError::bad_request(format!(
        "Invalid or conflicting Bedrock generation parameter: {field}."
    ))
    .with_code("bedrock_generation_parameters")
}

fn valid(field: &str, value: &Value, alias: bool) -> bool {
    match field {
        "maxTokens" => value.as_u64().is_some_and(|value| value >= 1),
        "temperature" | "topP" => value
            .as_f64()
            .is_some_and(|value| value.is_finite() && (0.0..=1.0).contains(&value)),
        "stopSequences" if alias && value.is_string() => {
            value.as_str().is_some_and(|value| !value.is_empty())
        }
        "stopSequences" => value.as_array().is_some_and(|items| {
            items.len() <= 2500
                && items
                    .iter()
                    .all(|item| item.as_str().is_some_and(|value| !value.is_empty()))
        }),
        _ => false,
    }
}

fn equal(field: &str, left: &Value, right: &Value) -> bool {
    if matches!(field, "temperature" | "topP") {
        left.as_f64() == right.as_f64()
    } else {
        left == right
    }
}

fn validate_config(config: &Value) -> Result<(), GatewayError> {
    let object = config
        .as_object()
        .ok_or_else(|| invalid("inferenceConfig"))?;
    for (field, _) in FIELDS {
        if object
            .get(field)
            .is_some_and(|value| !valid(field, value, false))
        {
            return Err(invalid(field));
        }
    }
    // Unknown native keys remain intact; model-specific support is still decided upstream.
    Ok(())
}

pub(super) fn validate_request(req: &CanonicalRelayRequest) -> Result<(), GatewayError> {
    if let Some(config) = req.extra.get("inferenceConfig") {
        validate_config(config)?;
    }
    for (field, aliases) in FIELDS {
        let mut selected = req
            .extra
            .get("inferenceConfig")
            .and_then(|config| config.get(field))
            .cloned();
        for alias in aliases {
            if let Some(value) = req.extra.get(*alias) {
                if !valid(field, value, true) {
                    return Err(invalid(field));
                }
                let value = generation::wire_value(field, value);
                if selected
                    .as_ref()
                    .is_some_and(|previous| !equal(field, previous, &value))
                {
                    return Err(invalid(field));
                }
                selected = Some(value);
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_body(body: &Value) -> Result<(), GatewayError> {
    if body
        .as_object()
        .is_some_and(|object| object.keys().any(|key| generation::is_alias(key)))
    {
        return Err(invalid("top-level alias"));
    }
    if let Some(config) = body.get("inferenceConfig") {
        validate_config(config)?;
    }
    Ok(())
}
