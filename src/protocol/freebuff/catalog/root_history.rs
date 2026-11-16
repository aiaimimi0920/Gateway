//! Trusted-host canonical turn history. This owns data, never file/tool permissions or a run grant.
use super::{root_builder::history, BusinessError};
use serde_json::Value;

pub(crate) struct RootHistory {
    messages: Vec<Value>,
    fresh_from: usize,
    limit: usize,
}
impl RootHistory {
    /// The supplied prefix is completed historical work. A new live request must use begin_turn.
    pub(crate) fn new(messages: Vec<Value>, limit: usize) -> Result<Self, BusinessError> {
        history::check_size(&messages, limit)?;
        let messages = if messages.is_empty() {
            messages
        } else {
            history::prepare(messages, limit)?
        };
        Ok(Self {
            fresh_from: messages.len(),
            messages,
            limit,
        })
    }
    pub(crate) fn messages(&self) -> &[Value] {
        &self.messages
    }
    pub(crate) fn begin_turn(&mut self, user: Value) -> Result<(), BusinessError> {
        let index = self.messages.len();
        self.steer(user)?;
        self.fresh_from = index;
        Ok(())
    }
    /// Steering belongs to the current turn and is preserved verbatim across a summary swap.
    pub(crate) fn steer(&mut self, user: Value) -> Result<(), BusinessError> {
        if user.get("role").and_then(Value::as_str) != Some("user") {
            return Err(BusinessError::InvalidInput);
        }
        self.extend(vec![user])
    }
    /// Results are supplied by the trusted host; accepting data never executes the named tool.
    pub(crate) fn append_tools(&mut self, tools: Vec<Value>) -> Result<(), BusinessError> {
        if tools.iter().any(|v| {
            v["role"] != "tool"
                || v.get("tool_call_id")
                    .and_then(Value::as_str)
                    .is_none_or(str::is_empty)
        }) {
            return Err(BusinessError::InvalidInput);
        }
        self.extend(tools)
    }
    fn extend(&mut self, values: Vec<Value>) -> Result<(), BusinessError> {
        if self.messages.len().saturating_add(values.len()) > 4096 {
            return Err(BusinessError::InvalidInput);
        }
        let old = self.messages.len();
        self.messages.extend(values);
        if let Err(error) = history::check_size(&self.messages, self.limit) {
            self.messages.truncate(old);
            return Err(error);
        }
        Ok(())
    }
    pub(super) fn is_live_turn(&self) -> bool {
        self.fresh_users().next().is_some()
    }
    pub(super) fn fresh_users(&self) -> impl Iterator<Item = &Value> + Clone {
        self.messages[self.fresh_from..]
            .iter()
            .filter(|v| v["role"] == "user")
    }
    /// An exclusive borrow pins the entire covered prefix until commit; no weak anchor search.
    pub(super) fn compacted(&self, memory: Value) -> Result<Self, BusinessError> {
        let mut messages = vec![memory];
        messages.extend(self.fresh_users().cloned());
        if messages.len() >= self.messages.len() {
            return Err(BusinessError::SummaryInvalid);
        }
        history::check_size(&messages, self.limit)?;
        Ok(Self {
            messages,
            fresh_from: 1,
            limit: self.limit,
        })
    }
    pub(super) fn record_completion(&mut self, body: &Value) -> Result<(), BusinessError> {
        let choices = body["choices"]
            .as_array()
            .filter(|c| c.len() == 1)
            .ok_or(BusinessError::InvalidResponse)?;
        let mut message = choices[0]["message"].clone();
        // The native completion protocol already validated this as an assistant message.
        message
            .as_object_mut()
            .ok_or(BusinessError::InvalidResponse)?
            .insert("role".into(), Value::String("assistant".into()));
        self.extend(vec![message])
    }
}
#[cfg(test)]
mod tests;
impl std::fmt::Debug for RootHistory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FreeBuffRootHistory { content: <redacted> }")
    }
}
