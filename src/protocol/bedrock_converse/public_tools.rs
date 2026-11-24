//! Validate client-owned function identities for the public text/function projection.
use std::collections::HashSet;

use serde_json::Value;

pub(crate) struct PublicTools {
    allowed: HashSet<String>,
    seen: HashSet<String>,
    required: bool,
    called: bool,
}

impl PublicTools {
    pub(crate) fn new(
        allowed: impl IntoIterator<Item = String>,
        prior_ids: impl IntoIterator<Item = String>,
        required: bool,
    ) -> Self {
        Self {
            allowed: allowed.into_iter().collect(),
            seen: prior_ids.into_iter().collect(),
            required,
            called: false,
        }
    }

    pub(crate) fn accept(&mut self, id: &str, name: &str) -> bool {
        if !tool_id(id) || !self.allowed.contains(name) || self.seen.contains(id) {
            return false;
        }
        self.seen.insert(id.to_owned());
        self.called = true;
        true
    }

    pub(crate) fn complete(&self, reason: &str) -> bool {
        (reason == "tool_calls") == self.called && !(self.required && reason == "stop")
    }
}

pub(crate) fn tool_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

pub(crate) fn tool_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
}

pub(crate) fn arguments(encoded: &str) -> Option<Value> {
    crate::protocol::unique_json::parse(encoded.as_bytes())
        .ok()
        .filter(Value::is_object)
}
