//! Four common generation controls map to Converse inferenceConfig, never top-level aliases.
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::canonical::ProtocolFamily;
use serde_json::{json, Value};
use std::borrow::Cow;

pub(super) fn for_target(req: &CanonicalRelayRequest) -> Cow<'_, CanonicalRelayRequest> {
    if req.protocol_family == ProtocolFamily::GeminiGenerateContent {
        // Reuse the existing nested-source projection without changing its wire owner.
        crate::protocol::generation_parameters::for_target(req, ProtocolFamily::BedrockConverse)
    } else {
        // Local mapping handles both stop aliases; do not clone full prompts/raw bodies.
        Cow::Borrowed(req)
    }
}

pub(super) const FIELDS: [(&str, &[&str]); 4] = [
    (
        "maxTokens",
        &["max_tokens", "max_completion_tokens", "max_output_tokens"],
    ),
    ("temperature", &["temperature"]),
    ("topP", &["top_p"]),
    ("stopSequences", &["stop", "stop_sequences"]),
];

pub(super) fn is_alias(key: &str) -> bool {
    FIELDS.iter().any(|(_, aliases)| aliases.contains(&key))
}

pub(super) fn wire_value(field: &str, value: &Value) -> Value {
    if field == "stopSequences" && value.is_string() {
        Value::Array(vec![value.clone()])
    } else {
        value.clone()
    }
}

pub(super) fn apply(req: &CanonicalRelayRequest, body: &mut Value) {
    let original = req.extra.get("inferenceConfig");
    let mut config = original.cloned().unwrap_or_else(|| json!({}));
    if let Some(object) = config.as_object_mut() {
        for (field, aliases) in FIELDS {
            if let Some(value) = aliases.iter().find_map(|alias| req.extra.get(*alias)) {
                // Checked admission rejects conflicts; the legacy structural packer is deterministic.
                object
                    .entry(field)
                    .or_insert_with(|| wire_value(field, value));
            }
        }
    }
    if original.is_some() || config.as_object().is_some_and(|object| !object.is_empty()) {
        body["inferenceConfig"] = config;
    }
}
