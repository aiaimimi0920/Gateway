//! Request-local admission, retained-value accounting and cumulative VM work.
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use serde_json::Value;
use std::cell::Cell;
use std::collections::HashMap;

use super::super::values::{turnstile_to_string, TurnstileValue};

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_KEY_BYTES: usize = 64 * 1024;
const MAX_INSTRUCTIONS: usize = 8192;
const MAX_CALL_DEPTH: usize = 64;
const MAX_VALUE_DEPTH: usize = 64;
const MAX_VALUE_NODES: usize = 32768;
const MAX_VALUE_BYTES: usize = 2 * 1024 * 1024;
const MAX_REGISTERS: usize = 4096;
const MAX_STATE_BYTES: usize = 8 * 1024 * 1024;
const MAX_WORK_BYTES: usize = 64 * 1024 * 1024;
const MAX_RESULT_BYTES: usize = 2 * 1024 * 1024;

pub(super) fn valid_input(dx: &str, key: &str) -> bool {
    dx.len() <= MAX_INPUT_BYTES && key.len() <= MAX_KEY_BYTES
}

struct StoredValue {
    value: TurnstileValue,
    cost: usize,
}

pub(super) struct TurnstileState {
    values: HashMap<String, StoredValue>,
    retained: usize,
    instructions: usize,
    calls: usize,
    depth: usize,
    remaining_work: Cell<usize>,
    failed: Cell<bool>,
}

impl TurnstileState {
    pub(super) fn new() -> Self {
        Self {
            values: HashMap::new(),
            retained: 0,
            instructions: 0,
            calls: 0,
            depth: 0,
            remaining_work: Cell::new(MAX_WORK_BYTES),
            failed: Cell::new(false),
        }
    }

    pub(super) fn failed(&self) -> bool {
        self.failed.get()
    }

    fn reject(&self) -> bool {
        self.failed.set(true);
        false
    }

    fn charge(&self, cost: usize) -> bool {
        if self.failed() {
            return false;
        }
        let Some(remaining) = self.remaining_work.get().checked_sub(cost) else {
            return self.reject();
        };
        self.remaining_work.set(remaining);
        true
    }

    pub(super) fn admit_instruction(&mut self) -> bool {
        if self.failed() || self.instructions >= MAX_INSTRUCTIONS {
            return self.reject();
        }
        self.instructions += 1;
        true
    }

    pub(super) fn enter_call(&mut self) -> bool {
        if self.failed() || self.calls >= MAX_INSTRUCTIONS || self.depth >= MAX_CALL_DEPTH {
            return self.reject();
        }
        self.calls += 1;
        self.depth += 1;
        true
    }

    pub(super) fn leave_call(&mut self) {
        self.depth -= 1;
    }

    pub(super) fn admit_json(&self, value: &Value) -> bool {
        let mut measure = ValueMeasure::default();
        if measure.json(value, 1).is_none() {
            return self.reject();
        }
        self.charge(measure.bytes)
    }

    pub(super) fn get(&self, key: &str) -> Option<&TurnstileValue> {
        let stored = self.values.get(key)?;
        // Charge every read conservatively: callers may clone, compare or serialize it.
        self.charge(stored.cost).then_some(&stored.value)
    }

    pub(super) fn insert(&mut self, key: String, value: TurnstileValue) {
        if self.failed() {
            return;
        }
        let Some(cost) = measured_value(&value).and_then(|cost| cost.checked_add(key.len())) else {
            self.reject();
            return;
        };
        if !self.charge(cost) {
            return;
        }
        let previous = self.values.get(&key).map(|stored| stored.cost);
        let retained = self
            .retained
            .checked_sub(previous.unwrap_or(0))
            .and_then(|bytes| bytes.checked_add(cost));
        let Some(retained) = retained.filter(|bytes| *bytes <= MAX_STATE_BYTES) else {
            self.reject();
            return;
        };
        if previous.is_none()
            && (self.values.len() >= MAX_REGISTERS || self.values.try_reserve(1).is_err())
        {
            self.reject();
            return;
        }
        self.values.insert(key, StoredValue { value, cost });
        self.retained = retained;
    }

    pub(super) fn write_result(&self, result: &mut String, value: &TurnstileValue) {
        if self.failed() {
            return;
        }
        let Some(cost) = measured_value(value) else {
            self.reject();
            return;
        };
        if !self.charge(cost) {
            return;
        }
        let text = turnstile_to_string(value);
        let encoded_len = text
            .len()
            .checked_add(2)
            .map(|length| length / 3)
            .and_then(|length| length.checked_mul(4));
        let Some(encoded_len) = encoded_len.filter(|length| *length <= MAX_RESULT_BYTES) else {
            self.reject();
            return;
        };
        if !self.charge(text.len().saturating_add(encoded_len)) {
            return;
        }
        *result = BASE64_STANDARD.encode(text.as_bytes());
    }
}

#[derive(Default)]
struct ValueMeasure {
    bytes: usize,
    nodes: usize,
}

impl ValueMeasure {
    fn node(&mut self, depth: usize) -> Option<()> {
        if depth > MAX_VALUE_DEPTH || self.nodes >= MAX_VALUE_NODES {
            return None;
        }
        self.nodes += 1;
        // Conservative logical cost, including container/enum overhead per node.
        self.text(64)
    }

    fn text(&mut self, bytes: usize) -> Option<()> {
        self.bytes = self.bytes.checked_add(bytes)?;
        (self.bytes <= MAX_VALUE_BYTES).then_some(())
    }

    fn json(&mut self, value: &Value, depth: usize) -> Option<()> {
        self.node(depth)?;
        match value {
            Value::String(text) => self.text(text.len())?,
            Value::Array(values) => {
                for value in values {
                    self.json(value, depth + 1)?;
                }
            }
            Value::Object(values) => {
                for (key, value) in values {
                    self.text(key.len())?;
                    self.json(value, depth + 1)?;
                }
            }
            _ => {}
        }
        Some(())
    }

    fn value(&mut self, value: &TurnstileValue, depth: usize) -> Option<()> {
        self.node(depth)?;
        match value {
            TurnstileValue::String(text) => self.text(text.len())?,
            TurnstileValue::Array(values) => {
                for value in values {
                    self.value(value, depth + 1)?;
                }
            }
            TurnstileValue::Object(values) => self.entries(values, depth)?,
            TurnstileValue::OrderedMap(map) => self.entries(map.entries(), depth)?,
            _ => {}
        }
        Some(())
    }

    fn entries(&mut self, entries: &[(String, TurnstileValue)], depth: usize) -> Option<()> {
        for (key, value) in entries {
            self.text(key.len())?;
            self.value(value, depth + 1)?;
        }
        Some(())
    }
}

fn measured_value(value: &TurnstileValue) -> Option<usize> {
    let mut measure = ValueMeasure::default();
    measure.value(value, 1)?;
    Some(measure.bytes)
}
