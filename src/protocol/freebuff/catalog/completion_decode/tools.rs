//! Native tool fragments use stable index/id/name association; never execute or repair arguments.
use super::{invalid, optional_string, BusinessError};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct Tools(BTreeMap<u64, Value>);
impl Tools {
    pub(super) fn push(&mut self, value: &Value) -> Result<(), BusinessError> {
        let calls = value.as_array().ok_or_else(invalid)?;
        for call in calls {
            let function = call
                .get("function")
                .and_then(Value::as_object)
                .ok_or_else(invalid)?;
            let id = optional_string(call.get("id"))?;
            let name = optional_string(function.get("name"))?;
            let arguments = optional_string(function.get("arguments"))?;
            let index = resolve_index(call, self.0.iter().map(|(i, value)| (*i, value)))?;
            if optional_string(call.get("type"))?.is_some_and(|kind| kind != "function") {
                return Err(invalid());
            }
            let stored = match self.0.entry(index) {
                std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::btree_map::Entry::Vacant(entry) => {
                    let name = name.filter(|name| !name.is_empty()).ok_or_else(invalid)?;
                    entry.insert(json!({"type":"function","function":{"name":name,"arguments":""}}))
                }
            };
            if let Some(id) = id {
                if stored.get("id").is_some_and(|old| old != id) {
                    return Err(invalid());
                }
                stored["id"] = json!(id);
            }
            if name.is_some_and(|name| stored["function"]["name"] != name) {
                return Err(invalid());
            }
            if let Some(arguments) = arguments {
                let Value::String(stored_arguments) = &mut stored["function"]["arguments"] else {
                    return Err(invalid());
                };
                stored_arguments.push_str(arguments);
            }
            if let Some(extra) = call.get("extra_content").filter(|v| !v.is_null()) {
                stored["extra_content"] = extra.clone();
            }
        }
        Ok(())
    }
    pub(super) fn into_value(self) -> Option<Value> {
        if self.0.is_empty() {
            None
        } else {
            Some(Value::Array(self.0.into_values().collect()))
        }
    }
}
/// Both the strict native decoder and public projection use this association rule. The known
/// iterator is ordered by native index and contains identities, not execution permissions.
pub(crate) fn resolve_index<'a>(
    call: &Value,
    known: impl Iterator<Item = (u64, &'a Value)> + Clone,
) -> Result<u64, BusinessError> {
    let id = optional_string(call.get("id"))?;
    let name = optional_string(call.get("function").and_then(|f| f.get("name")))?;
    let index = match call.get("index").filter(|v| !v.is_null()) {
        Some(index) => index.as_u64().ok_or_else(invalid)?,
        None => id
            .and_then(|id| known.clone().find(|(_, v)| v["id"] == id).map(|(i, _)| i))
            .or_else(|| {
                if id.is_some() {
                    return None;
                }
                known
                    .clone()
                    .last()
                    .filter(|(_, v)| name.is_none_or(|name| v["function"]["name"] == name))
                    .map(|(i, _)| i)
            })
            .unwrap_or_else(|| {
                let mut free = 0;
                for (index, _) in known {
                    if index != free {
                        break;
                    }
                    free += 1;
                }
                free
            }),
    };
    if index >= 1024 {
        return Err(BusinessError::ResponseTooLarge);
    }
    Ok(index)
}
