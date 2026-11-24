//! Map the selected o3-mini deployment's length and effort controls without using its alias.
use crate::{
    error::GatewayError,
    protocol::canonical::{CanonicalRelayRequest, EndpointKind},
};
use serde_json::{Map, Value};

use super::invalid;

pub(super) fn adapt(
    req: &CanonicalRelayRequest,
    fields: &mut Map<String, Value>,
    enabled: bool,
) -> Result<(), GatewayError> {
    let effort = fields.get("reasoning_effort").filter(|v| !v.is_null());
    if (effort.is_some() || !req.raw_body["reasoning_effort"].is_null())
        && (!enabled
            || req.endpoint_kind != EndpointKind::ChatCompletions
            || effort.is_some_and(|v| !matches!(v.as_str(), Some("low" | "medium" | "high"))))
    {
        return Err(invalid());
    }
    if !enabled {
        return Ok(());
    }

    // Reject unrepresentable controls instead of silently replacing the caller's policy.
    for name in [
        "temperature",
        "top_p",
        "presence_penalty",
        "frequency_penalty",
        "logprobs",
        "top_logprobs",
        "logit_bias",
        "stop",
        "thinking",
    ] {
        if fields.remove(name).is_some_and(|v| !v.is_null()) {
            return Err(invalid());
        }
    }
    let mut cap = None;
    for name in ["max_tokens", "max_completion_tokens", "max_output_tokens"] {
        if let Some(value) = req.extra.get(name).filter(|v| !v.is_null()) {
            if cap.is_some() || !value.as_u64().is_some_and(|n| n > 0) {
                return Err(invalid());
            }
            cap = Some(value.clone());
        }
        fields.remove(name);
    }
    if let Some(cap) = cap {
        fields.insert("max_completion_tokens".into(), cap);
    }
    // Optional null is omission on the preview wire, not an instruction to change the default.
    if fields.get("reasoning_effort").is_some_and(Value::is_null) {
        fields.remove("reasoning_effort");
    }
    Ok(())
}
