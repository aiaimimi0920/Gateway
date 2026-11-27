//! Token telemetry from the pinned Cursor protobuf schema, never a local estimate.
//! Turn totals supersede model-call totals. Partial optional totals remain unknown.
use super::{
    protobuf::{fields, utf8_bounded},
    ProbeError,
};
use crate::protocol::canonical::TokenUsage;
use std::collections::HashMap;

const MAX_CALLS: usize = 128;
// JSON clients must be able to represent every published count exactly.
const MAX_TOKENS: u64 = (1 << 53) - 1;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Counts {
    input: u64,
    output: u64,
    read: Option<u64>,
    write: Option<u64>,
    reasoning: Option<u64>,
}

impl Counts {
    fn canonical(&self) -> Result<TokenUsage, ProbeError> {
        if self.reasoning.is_some_and(|n| n > self.output) {
            return Err(invalid());
        }
        Ok(TokenUsage {
            prompt_tokens: self.input,
            completion_tokens: self.output,
            total_tokens: sum(self.input, self.output)?,
            cache_read_input_tokens: self.read,
            cache_creation_input_tokens: self.write,
        })
    }
}

#[derive(Default)]
pub(super) struct Usage {
    calls: HashMap<String, Counts>,
    terminal: Option<Option<Counts>>,
    split: bool,
}

impl Usage {
    pub(super) fn model_call(&mut self, bytes: &[u8]) -> Result<(), ProbeError> {
        if self.terminal.is_some() {
            return Err(invalid());
        }
        let mut id = None;
        let mut values = [None; 5];
        for field in fields(bytes)? {
            match field.tag {
                1 if field.wire == 2 && id.is_none() => {
                    let value = utf8_bounded(field.bytes, 256)?;
                    if value.trim().is_empty() {
                        return Err(invalid());
                    }
                    id = Some(value);
                }
                2..=6 => scalar(
                    &mut values[(field.tag - 2) as usize],
                    field.wire,
                    field.varint,
                )?,
                _ => return Err(invalid()),
            }
        }
        let id = id.ok_or_else(invalid)?;
        // Fields 2..5 are non-optional proto3 scalars: absent wire values are zero.
        let counts = Counts {
            input: values[0].unwrap_or(0),
            output: values[1].unwrap_or(0),
            read: Some(values[2].unwrap_or(0)),
            write: Some(values[3].unwrap_or(0)),
            reasoning: values[4],
        };
        counts.canonical()?;
        if let Some(previous) = self.calls.get(&id) {
            return if previous == &counts {
                Ok(())
            } else {
                Err(invalid())
            };
        }
        if self.calls.len() >= MAX_CALLS {
            return Err(invalid());
        }
        self.calls.insert(id, counts);
        Ok(())
    }

    pub(super) fn turn_end(&mut self, bytes: &[u8]) -> Result<(), ProbeError> {
        if self.terminal.is_some() {
            return Err(invalid());
        }
        let mut seen = std::collections::HashSet::new();
        let mut values = [None; 5];
        for field in fields(bytes)? {
            if !seen.insert(field.tag) {
                return Err(invalid());
            }
            match field.tag {
                1..=5 => scalar(
                    &mut values[(field.tag - 1) as usize],
                    field.wire,
                    field.varint,
                )?,
                6 if field.wire == 0 => {}
                7 if field.wire == 2 => {} // Estimated price is not token usage or Free attribution.
                _ => return Err(invalid()),
            }
        }
        let counts = match (values[0], values[1]) {
            (Some(input), Some(output)) => Some(Counts {
                input,
                output,
                read: values[2],
                write: values[3],
                reasoning: values[4],
            }),
            _ if values.iter().all(Option::is_none) => self.call_total()?,
            _ => None, // Do not turn missing optional totals into zero.
        };
        if let Some(counts) = &counts {
            counts.canonical()?;
        }
        self.terminal = Some(counts);
        Ok(())
    }

    pub(super) fn split_phase(&mut self) {
        // A tool handoff has no model-call identity. Late telemetry cannot be
        // safely assigned to the client phase; never bill the whole Run twice.
        self.split = true;
    }

    pub(super) fn finish(&self) -> Result<Option<TokenUsage>, ProbeError> {
        if self.split {
            return Ok(None);
        }
        self.terminal
            .as_ref()
            .and_then(Option::as_ref)
            .map(Counts::canonical)
            .transpose()
    }

    fn call_total(&self) -> Result<Option<Counts>, ProbeError> {
        if self.calls.is_empty() {
            return Ok(None);
        }
        let mut total = Counts {
            input: 0,
            output: 0,
            read: Some(0),
            write: Some(0),
            reasoning: None,
        };
        for counts in self.calls.values() {
            total.input = sum(total.input, counts.input)?;
            total.output = sum(total.output, counts.output)?;
            total.read = Some(sum(total.read.unwrap(), counts.read.unwrap())?);
            total.write = Some(sum(total.write.unwrap(), counts.write.unwrap())?);
            if let Some(value) = counts.reasoning {
                total.reasoning = Some(sum(total.reasoning.unwrap_or(0), value)?);
            }
        }
        Ok(Some(total))
    }
}

fn sum(a: u64, b: u64) -> Result<u64, ProbeError> {
    a.checked_add(b)
        .filter(|n| *n <= MAX_TOKENS)
        .ok_or_else(invalid)
}

fn scalar(slot: &mut Option<u64>, wire: u8, value: u64) -> Result<(), ProbeError> {
    if wire != 0 || value > MAX_TOKENS || slot.replace(value).is_some() {
        return Err(invalid());
    }
    Ok(())
}

fn invalid() -> ProbeError {
    ProbeError::new("invalid_token_usage")
}

#[cfg(test)]
#[path = "usage_tests.rs"]
mod tests;
