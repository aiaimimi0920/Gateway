//! Public protocol envelopes only. Purchase, signing, cancellation and accounting retain one owner.
use super::{BusinessError, RelayStep};
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind, TokenUsage};
use serde_json::{json, Value};
mod messages;
mod request;
mod response;
mod responses;
pub(super) use request::request;
pub(super) use response::project;
pub(super) use response::response;
pub(super) struct Completed {
    pub body: Value,
    pub usage: Option<TokenUsage>,
    pub semantics: Option<String>,
}
fn fields(value: &Value, allowed: &[&str]) -> Result<(), BusinessError> {
    if value
        .as_object()
        .is_none_or(|o| o.keys().any(|k| !allowed.contains(&k.as_str())))
    {
        return Err(BusinessError::InvalidInput);
    }
    Ok(())
}
fn copy_fields(source: &Value, target: &mut Value, names: &[&str]) {
    for name in names {
        if let Some(value) = source.get(name) {
            target[*name] = value.clone();
        }
    }
}
// Count serialized bytes without a second full body buffer. Both mapped input and expanded output
// are admitted before the next resource/FINISH boundary; UTF-8/JSON escaping is measured by serde.
pub(super) fn within_limit(value: &Value, limit: usize) -> bool {
    struct Counter(usize);
    impl std::io::Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self
                .0
                .checked_sub(bytes.len())
                .ok_or_else(|| std::io::Error::other("FreeBuff public JSON limit"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(Counter(limit), value).is_ok()
}
pub(super) fn observe(body: Option<&Value>) -> (Option<TokenUsage>, Option<String>) {
    let Some(body) = body else {
        return (None, None);
    };
    let usage = body.get("usage").and_then(|u| {
        let prompt = u["prompt_tokens"].as_u64()?;
        let completion = u["completion_tokens"].as_u64()?;
        let total = match u.get("total_tokens").filter(|v| !v.is_null()) {
            Some(total) => total.as_u64()?,
            None => prompt.checked_add(completion)?,
        };
        Some(TokenUsage {
            prompt_tokens: prompt,
            completion_tokens: completion,
            total_tokens: total,
            cache_read_input_tokens: u
                .pointer("/prompt_tokens_details/cached_tokens")
                .and_then(Value::as_u64),
            cache_creation_input_tokens: None,
        })
    });
    let semantics = body
        .pointer("/choices/0/finish_reason")
        .and_then(Value::as_str)
        .map(|s| {
            if s == "function_call" {
                "tool_calls"
            } else {
                s
            }
            .to_owned()
        });
    (usage, semantics)
}
