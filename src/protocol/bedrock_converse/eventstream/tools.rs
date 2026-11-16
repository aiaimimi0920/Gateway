//! Tool identities are immutable; pre-start input waits in the same bounded argument owner.
use super::{frame::Output, invalid};
use crate::protocol::stream_error::ProtocolStreamError;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub(super) const MAX_TOOLS: usize = 4096;
const MAX_STATE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Default)]
struct Tool {
    id: String,
    name: String,
    wire: Option<u32>,
    arguments: String,
}

#[derive(Default)]
pub(super) struct Tools {
    entries: BTreeMap<u32, Tool>,
    bytes: usize,
}

impl Tools {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn observe(
        &mut self,
        call: &Value,
        fallback: usize,
        next: &mut u32,
        output: &mut Output,
    ) -> Result<(), ProtocolStreamError> {
        let index = match call.get("index") {
            Some(value) => value.as_u64().and_then(|n| i32::try_from(n).ok()),
            None => i32::try_from(fallback).ok(),
        }
        .ok_or_else(|| invalid("Bedrock tool index does not fit the output field"))?
            as u32;
        if !call.is_object()
            || call
                .get("type")
                .is_some_and(|v| !v.is_null() && v != "function")
        {
            return Err(invalid("Bedrock tool delta is not a function"));
        }
        if !self.entries.contains_key(&index) && self.entries.len() >= MAX_TOOLS {
            return Err(invalid("Bedrock tool call limit exceeded"));
        }
        let function = call.get("function").filter(|v| !v.is_null());
        if function.is_some_and(|v| !v.is_object()) {
            return Err(invalid("Bedrock tool function is invalid"));
        }
        let id = identity(string(call.get("id"))?, true)?;
        let name = identity(string(function.and_then(|v| v.get("name")))?, false)?;
        let input = string(function.and_then(|v| v.get("arguments")))?;
        let tool = self.entries.entry(index).or_default();
        let additional = identity_bytes(&tool.id, id)?
            .checked_add(identity_bytes(&tool.name, name)?)
            .and_then(|n| n.checked_add(input.map_or(0, str::len)))
            .ok_or_else(|| invalid("Bedrock tool state byte limit exceeded"))?;
        self.bytes = self
            .bytes
            .checked_add(additional)
            .filter(|n| *n <= MAX_STATE_BYTES)
            .ok_or_else(|| invalid("Bedrock tool state byte limit exceeded"))?;
        if tool.id.is_empty() {
            tool.id = id.unwrap_or_default().to_owned();
        }
        if tool.name.is_empty() {
            tool.name = name.unwrap_or_default().to_owned();
        }
        let previous = tool.arguments.len();
        if let Some(input) = input {
            tool.arguments
                .try_reserve(input.len())
                .map_err(|_| invalid("Bedrock tool state allocation failed"))?;
            tool.arguments.push_str(input);
        }
        let new_start =
            tool.wire.is_none() && !tool.id.trim().is_empty() && !tool.name.trim().is_empty();
        if new_start {
            tool.wire = Some(*next);
            *next += 1; // At most MAX_TOOLS + one text block, independently bounded.
            output.emit(
                "contentBlockStart",
                json!({"contentBlockIndex":tool.wire.unwrap(),
                "start":{"toolUse":{"toolUseId":tool.id,"name":tool.name}}}),
            )?;
        }
        if let Some(index) = tool.wire {
            if new_start && previous > 0 {
                output.delta(index, true, &tool.arguments[..previous])?;
            }
            if let Some(input) = input {
                output.delta(index, true, input)?;
            }
        }
        Ok(())
    }

    pub fn finish(&self, output: &mut Output) -> Result<(), ProtocolStreamError> {
        for tool in self.entries.values() {
            let index = tool
                .wire
                .ok_or_else(|| invalid("Bedrock tool ended before its identity was complete"))?;
            if tool.arguments.trim().is_empty() {
                output.delta(index, true, "{}")?;
            } else if serde_json::from_str::<Value>(&tool.arguments).is_err() {
                return Err(invalid("Bedrock tool arguments are not complete JSON"));
            }
        }
        Ok(())
    }
}

fn string(value: Option<&Value>) -> Result<Option<&str>, ProtocolStreamError> {
    value
        .filter(|v| !v.is_null())
        .map(|v| {
            v.as_str()
                .ok_or_else(|| invalid("Bedrock tool delta field is not a string"))
        })
        .transpose()
}

fn identity(value: Option<&str>, id: bool) -> Result<Option<&str>, ProtocolStreamError> {
    let value = value.filter(|v| !v.trim().is_empty());
    if value.is_some_and(|v| {
        v.len() > 64
            || !v.bytes().all(|b| {
                b.is_ascii_alphanumeric()
                    || b == b'_'
                    || b == b'-'
                    || (id && matches!(b, b'.' | b':'))
            })
    }) {
        return Err(invalid("Bedrock tool identity violates the native schema"));
    }
    Ok(value)
}

fn identity_bytes(old: &str, value: Option<&str>) -> Result<usize, ProtocolStreamError> {
    let Some(value) = value.filter(|v| !v.trim().is_empty()) else {
        return Ok(0);
    };
    if !old.is_empty() && old != value {
        return Err(invalid("Bedrock tool identity changed after declaration"));
    }
    Ok(if old.is_empty() { value.len() } else { 0 })
}
