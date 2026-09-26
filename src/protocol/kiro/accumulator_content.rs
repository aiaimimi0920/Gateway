//! Limits for total nonstream retained response content, not individual frames.
use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalToolCall;
use std::collections::HashMap;

#[derive(Clone, Copy)]
pub(super) struct AccumulationLimits {
    pub(super) bytes: usize,
    pub(super) tools: usize,
}

impl Default for AccumulationLimits {
    fn default() -> Self {
        Self {
            bytes: 64 * 1024 * 1024,
            tools: 4096,
        }
    }
}

struct PendingToolCall {
    id: String,
    name: String,
    arguments: String,
}

pub(super) struct AccumulatedContent {
    text: String,
    pending: HashMap<String, PendingToolCall>,
    order: Vec<String>,
    limits: AccumulationLimits,
    retained: usize,
}

impl AccumulatedContent {
    pub(super) fn new(limits: AccumulationLimits) -> Self {
        Self {
            text: String::new(),
            pending: HashMap::new(),
            order: Vec::new(),
            limits,
            retained: 0,
        }
    }

    fn next_total(&self, additional: usize) -> Result<usize, GatewayError> {
        self.retained
            .checked_add(additional)
            .filter(|total| *total <= self.limits.bytes)
            .ok_or_else(|| limit_error("retained content bytes"))
    }

    pub(super) fn push_text(&mut self, content: &str) -> Result<(), GatewayError> {
        let total = self.next_total(content.len())?;
        // Standard geometric growth avoids reallocating for every tiny upstream delta.
        self.text
            .try_reserve(content.len())
            .map_err(|_| allocation_error())?;
        self.text.push_str(content);
        self.retained = total;
        Ok(())
    }

    pub(super) fn push_tool(
        &mut self,
        id: String,
        name: String,
        input: String,
        names: &HashMap<String, String>,
    ) -> Result<(), GatewayError> {
        if let Some(previous) = self.pending.get(&id).map(|call| call.arguments.len()) {
            let length = previous
                .checked_add(input.len())
                .ok_or_else(|| limit_error("tool arguments"))?;
            let total = self.next_total(length.max(2) - previous.max(2))?;
            let call = self.pending.get_mut(&id).expect("existing tool");
            call.arguments
                .try_reserve(input.len())
                .map_err(|_| allocation_error())?;
            call.arguments.push_str(&input);
            self.retained = total;
            return Ok(());
        }
        if self.pending.len() >= self.limits.tools {
            return Err(limit_error("tool count"));
        }
        let restored = names.get(&name).unwrap_or(&name);
        // IDs live in the map, ordered list and entry. Reserve '{}' for empty arguments.
        let additional = id
            .len()
            .checked_mul(3)
            .and_then(|bytes| bytes.checked_add(restored.len()))
            .and_then(|bytes| bytes.checked_add(input.len().max(2)))
            .ok_or_else(|| limit_error("tool metadata"))?;
        let total = self.next_total(additional)?;
        self.pending
            .try_reserve(1)
            .map_err(|_| allocation_error())?;
        self.order.try_reserve(1).map_err(|_| allocation_error())?;
        let entry_id = copy_string(&id)?;
        let order_id = copy_string(&id)?;
        let name = copy_string(restored)?;
        self.order.push(order_id);
        self.pending.insert(
            id,
            PendingToolCall {
                id: entry_id,
                name,
                arguments: input,
            },
        );
        self.retained = total;
        Ok(())
    }

    pub(super) fn finish(mut self) -> Result<(String, Vec<CanonicalToolCall>), GatewayError> {
        let mut calls = Vec::new();
        calls
            .try_reserve_exact(self.order.len())
            .map_err(|_| allocation_error())?;
        for id in self.order {
            if let Some(call) = self.pending.remove(&id) {
                calls.push(CanonicalToolCall {
                    id: Some(call.id),
                    call_type: "function".to_string(),
                    name: Some(call.name),
                    arguments: Some(normalize_arguments(call.arguments)?),
                    raw: HashMap::new(),
                });
            }
        }
        Ok((self.text, calls))
    }
}

fn normalize_arguments(mut arguments: String) -> Result<String, GatewayError> {
    let start = arguments.len() - arguments.trim_start().len();
    let end = arguments.trim_end().len();
    if start >= end {
        arguments.clear();
        arguments.try_reserve(2).map_err(|_| allocation_error())?;
        arguments.push_str("{}");
    } else {
        arguments.truncate(end);
        // A single final compaction, never a per-frame front drain or a full-size clone.
        arguments.drain(..start);
    }
    Ok(arguments)
}

fn copy_string(value: &str) -> Result<String, GatewayError> {
    let mut result = String::new();
    result
        .try_reserve_exact(value.len())
        .map_err(|_| allocation_error())?;
    result.push_str(value);
    Ok(result)
}

fn limit_error(scope: &str) -> GatewayError {
    GatewayError::server_error(format!("Kiro accumulated response exceeds {scope} limit"))
        .with_code("kiro_accumulated_response_too_large")
}

fn allocation_error() -> GatewayError {
    GatewayError::server_error("Unable to allocate Kiro accumulated response")
        .with_code("kiro_accumulated_response_allocation_failed")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_total_rejects_overflow_without_changing_retained_content() {
        let mut content = AccumulatedContent::new(AccumulationLimits {
            bytes: usize::MAX,
            tools: 1,
        });
        content.retained = usize::MAX;
        assert!(content.push_text("x").is_err());
        assert!(content.text.is_empty());
        assert_eq!(content.retained, usize::MAX);
    }

    #[test]
    fn normalization_preserves_unicode_trim_and_reuses_argument_allocation() {
        let value = "\u{2003}{\"x\":1}\u{2003}".to_string();
        let capacity = value.capacity();
        let normalized = normalize_arguments(value).unwrap();
        assert_eq!(normalized, "{\"x\":1}");
        assert_eq!(normalized.capacity(), capacity);
        assert_eq!(normalize_arguments(" ".into()).unwrap(), "{}");
    }
}
