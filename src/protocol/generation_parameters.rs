//! Translate standard sampling/length parameters only when crossing wire families.
use super::canonical::{CanonicalRelayRequest, ProtocolFamily};
use serde_json::Value;
use std::borrow::Cow;

pub(super) fn for_target(
    req: &CanonicalRelayRequest,
    target: ProtocolFamily,
) -> Cow<'_, CanonicalRelayRequest> {
    if req.protocol_family == target {
        return Cow::Borrowed(req);
    }
    let mut translated = req.clone();
    let extra = &mut translated.extra;
    let nested = match req.protocol_family {
        ProtocolFamily::BedrockConverse => Some(("inferenceConfig", "maxTokens")),
        ProtocolFamily::GeminiGenerateContent => Some(("generationConfig", "maxOutputTokens")),
        _ => None,
    };
    if let Some((container, limit)) = nested {
        if let Some(Value::Object(config)) = extra.remove(container) {
            for (source, dest) in [
                (limit, "max_tokens"),
                ("temperature", "temperature"),
                ("topP", "top_p"),
                ("topK", "top_k"),
                ("stopSequences", "stop"),
            ] {
                if let Some(value) = config.get(source) {
                    extra.insert(dest.into(), value.clone());
                }
            }
        }
    }
    if target == ProtocolFamily::Anthropic {
        if !extra.contains_key("max_tokens") {
            if let Some(value) = extra
                .get("max_output_tokens")
                .or_else(|| extra.get("max_completion_tokens"))
                .cloned()
            {
                extra.insert("max_tokens".into(), value);
            }
        }
        extra.remove("max_output_tokens");
        extra.remove("max_completion_tokens");
        if let Some(value) = extra.remove("stop") {
            let value = if value.is_string() {
                Value::Array(vec![value])
            } else {
                value
            };
            extra.entry("stop_sequences".into()).or_insert(value);
        }
    } else if let Some(value) = extra.remove("stop_sequences") {
        extra.entry("stop".into()).or_insert(value);
    }
    Cow::Owned(translated)
}
