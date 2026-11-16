//! Resolve all spellings before discarding provenance; never select authentication by map order.
use crate::error::GatewayError;
use serde_json::{Map, Value};

pub(super) fn invalid() -> GatewayError {
    GatewayError::bad_request("Kiro 导入材料不完整、字段冲突或与服务商不匹配")
        .with_code("kiro_import_material_invalid")
}

pub(super) fn key(value: &str) -> String {
    value
        .chars()
        .filter(|c| !matches!(c, '_' | '-'))
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

pub(super) struct Fields {
    pub root: Map<String, Value>,
    pub extra: Map<String, Value>,
}

impl Fields {
    pub fn new(raw: Value) -> Result<Self, GatewayError> {
        let Value::Object(mut root) = raw else {
            return Err(invalid());
        };
        let extra = take(&mut root, &["extrabody"])?;
        let extra = match extra {
            None | Some(Value::Null) => Map::new(),
            Some(Value::Object(extra)) => extra,
            _ => return Err(invalid()),
        };
        // An archival copy is not another authorization source or another stored secret copy.
        root.retain(|name, _| key(name) != "rawsource");
        Ok(Self { root, extra })
    }

    pub fn root(&mut self, aliases: &[&str]) -> Result<Option<Value>, GatewayError> {
        take(&mut self.root, aliases)
    }

    pub fn read(&mut self, aliases: &[&str]) -> Result<Option<Value>, GatewayError> {
        equal(self.root(aliases)?, take(&mut self.extra, aliases)?)
    }

    pub fn legacy_refresh(&self) -> Result<Option<String>, GatewayError> {
        let mut selected = None;
        for (name, value) in &self.extra {
            if matches!(key(name).as_str(), "kirorefreshtoken" | "refreshtoken") {
                if selected.is_some_and(|old| old != value) {
                    return Err(invalid());
                }
                selected = Some(value);
            }
        }
        string(selected.cloned())
    }

    pub fn machine(&mut self) -> Result<Option<String>, GatewayError> {
        let mut result = None;
        for map in [&mut self.root, &mut self.extra] {
            let names: Vec<_> = map
                .keys()
                .filter(|name| matches!(key(name).as_str(), "kiromachineid" | "machineid"))
                .cloned()
                .collect();
            for name in names {
                let value = map.remove(&name).unwrap();
                let machine = value
                    .as_str()
                    .and_then(crate::protocol::kiro::normalize_machine_id)
                    .ok_or_else(invalid)?;
                if result.as_ref().is_some_and(|old| old != &machine) {
                    return Err(invalid());
                }
                result = Some(machine);
            }
        }
        Ok(result)
    }
}

fn take(map: &mut Map<String, Value>, aliases: &[&str]) -> Result<Option<Value>, GatewayError> {
    let names: Vec<_> = map
        .keys()
        .filter(|name| aliases.contains(&key(name).as_str()))
        .cloned()
        .collect();
    let mut result = None;
    for name in names {
        result = equal(result, map.remove(&name))?;
    }
    Ok(result)
}

fn equal(left: Option<Value>, right: Option<Value>) -> Result<Option<Value>, GatewayError> {
    match (left, right) {
        (Some(left), Some(right)) if left != right => Err(invalid()),
        (Some(value), _) | (_, Some(value)) => Ok(Some(value)),
        _ => Ok(None),
    }
}

pub(super) fn string(value: Option<Value>) -> Result<Option<String>, GatewayError> {
    value
        .map(|value| {
            value
                .as_str()
                .filter(|text| !text.is_empty())
                .map(str::to_owned)
                .ok_or_else(invalid)
        })
        .transpose()
}
