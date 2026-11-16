//! Sequential Preview call/parameter lifetimes are independent; only closed calls reach projection.
use super::{Contract, GatewayError, Value};
use serde_json::json;
use std::collections::HashSet;
mod argument_path;
use argument_path::Segment;

pub(super) struct FunctionStream {
    contract: Contract,
    pending: Option<Pending>,
}
struct Pending {
    call: Value,
    args: Value,
    strings: HashSet<Vec<Segment>>,
}

fn continuing(contract: Contract, raw: &Value) -> Result<bool, GatewayError> {
    let invalid = |reason| contract.invalid(reason);
    raw.get("willContinue").map_or(Ok(false), |v| {
        v.as_bool()
            .ok_or_else(|| invalid("Invalid argument continuation flag."))
    })
}

impl FunctionStream {
    pub(super) fn new(contract: Contract) -> Self {
        Self {
            contract,
            pending: None,
        }
    }
    pub(super) fn accept(&mut self, raw: &Value) -> Result<Option<Value>, GatewayError> {
        let contract = self.contract;
        let invalid = |reason| contract.invalid(reason);
        let object = raw
            .as_object()
            .ok_or_else(|| invalid("Invalid function call."))?;
        if self.pending.is_none()
            && !object.contains_key("partialArgs")
            && !object.contains_key("willContinue")
        {
            return Ok(Some(raw.clone()));
        }
        // Complete args and Preview deltas have no documented merge/overwrite precedence.
        if object.contains_key("args") {
            return Err(invalid("Mixed complete and partial function arguments."));
        }
        if self.pending.is_none() {
            let name = raw
                .get("name")
                .and_then(Value::as_str)
                .filter(|v| !v.trim().is_empty())
                .ok_or_else(|| invalid("Missing streamed function name."))?;
            self.pending = Some(Pending {
                call: json!({"name":name}),
                args: json!({}),
                strings: HashSet::new(),
            });
        }
        let pending = self.pending.as_mut().unwrap();
        if raw.get("name").is_some_and(|v| v != &pending.call["name"]) {
            return Err(invalid("Interleaved or conflicting function name."));
        }
        if let Some(id) = raw.get("id") {
            if !id.as_str().is_some_and(|v| !v.trim().is_empty())
                || pending.call.get("id").is_some_and(|old| old != id)
            {
                return Err(invalid("Interleaved or conflicting function ID."));
            }
            pending.call["id"] = id.clone();
        }
        if let Some(parts) = raw.get("partialArgs") {
            for part in parts
                .as_array()
                .ok_or_else(|| invalid("Invalid partial argument array."))?
            {
                pending.accept(contract, part)?;
            }
        }
        if continuing(contract, raw)? {
            return Ok(None);
        }
        if !pending.strings.is_empty() {
            return Err(invalid(
                "Function ended with an unfinished string argument.",
            ));
        }
        let mut pending = self.pending.take().unwrap();
        pending.call["args"] = pending.args;
        Ok(Some(pending.call))
    }

    pub(super) fn finish(self) -> Result<(), GatewayError> {
        if self.pending.is_some() {
            return Err(self.contract.invalid("Unfinished streamed function call."));
        }
        Ok(())
    }
}

impl Pending {
    fn accept(&mut self, contract: Contract, raw: &Value) -> Result<(), GatewayError> {
        let invalid = |reason| contract.invalid(reason);
        let object = raw
            .as_object()
            .ok_or_else(|| invalid("Invalid partial argument."))?;
        if object.keys().any(|key| {
            !matches!(
                key.as_str(),
                "jsonPath"
                    | "willContinue"
                    | "numberValue"
                    | "stringValue"
                    | "boolValue"
                    | "nullValue"
            )
        }) {
            return Err(invalid("Unsupported partial argument field."));
        }
        let path = argument_path::parse(
            contract,
            raw.get("jsonPath")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("Missing argument path."))?,
        )?;
        let keep = continuing(contract, raw)?;
        let mut deltas = ["numberValue", "stringValue", "boolValue", "nullValue"]
            .into_iter()
            .filter_map(|key| raw.get(key).map(|value| (key, value)));
        let delta = deltas.next();
        if deltas.next().is_some() {
            return Err(invalid("Ambiguous partial argument value."));
        }
        let open = self.strings.contains(&path);
        if open {
            let text = match delta {
                Some(("stringValue", Value::String(text))) => Some(text.as_str()),
                None if !keep => None,
                _ => return Err(invalid("Invalid string continuation.")),
            };
            let (slot, _) = argument_path::slot(contract, &mut self.args, &path)?;
            let Value::String(value) = slot else {
                return Err(invalid("String continuation path conflicts."));
            };
            if let Some(text) = text {
                value.push_str(text);
            }
            if !keep {
                self.strings.remove(&path);
            }
        } else {
            let value = match delta {
                Some(("stringValue", value @ Value::String(_))) => value,
                Some(("numberValue", value @ Value::Number(_))) if !keep => value,
                Some(("boolValue", value @ Value::Bool(_))) if !keep => value,
                Some(("nullValue", value @ Value::Null)) if !keep => value,
                _ => return Err(invalid("Invalid or missing partial argument value.")),
            };
            let (slot, fresh) = argument_path::slot(contract, &mut self.args, &path)?;
            if !fresh {
                return Err(invalid("Repeated or conflicting completed argument."));
            }
            *slot = value.clone();
            if keep {
                self.strings.insert(path);
            }
        }
        Ok(())
    }
}
