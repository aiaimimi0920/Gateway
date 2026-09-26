//! Udio JSON field aliases and typed scalar decoding.

use serde_json::Map;
use serde_json::Value;

pub(super) fn read_optional_string(obj: &Map<String, Value>, aliases: &[&str]) -> Option<String> {
    aliases
        .iter()
        .find_map(|key| obj.get(*key).and_then(|value| value.as_str()))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn read_optional_bool(obj: &Map<String, Value>, aliases: &[&str]) -> Option<bool> {
    aliases.iter().find_map(|key| {
        obj.get(*key).and_then(|value| match value {
            Value::Bool(flag) => Some(*flag),
            Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => Some(true),
                "false" | "0" | "no" | "off" => Some(false),
                _ => None,
            },
            _ => None,
        })
    })
}

pub(super) fn read_optional_u64(obj: &Map<String, Value>, aliases: &[&str]) -> Option<u64> {
    aliases.iter().find_map(|key| {
        obj.get(*key).and_then(|value| match value {
            Value::Number(number) => number.as_u64(),
            Value::String(text) => text.trim().parse::<u64>().ok(),
            _ => None,
        })
    })
}

pub(super) fn read_number_fields(obj: &Map<String, Value>, aliases: &[&str]) -> Option<f64> {
    aliases
        .iter()
        .find_map(|key| obj.get(*key))
        .and_then(read_number_value)
}

fn read_number_value(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    }
}

pub(super) fn read_value_string(value: &Value, aliases: &[&str]) -> Option<String> {
    let obj = value.as_object()?;
    read_optional_string(obj, aliases)
}

pub(super) fn read_object_string(obj: &Map<String, Value>, aliases: &[&str]) -> Option<String> {
    read_optional_string(obj, aliases)
}

pub(super) fn read_object_bool(obj: &Map<String, Value>, aliases: &[&str]) -> Option<bool> {
    read_optional_bool(obj, aliases)
}

pub(super) fn read_number_fields_from_object(
    obj: &Map<String, Value>,
    aliases: &[&str],
) -> Option<f64> {
    read_number_fields(obj, aliases)
}

pub(super) fn read_value_bool(value: &Value, aliases: &[&str]) -> Option<bool> {
    let obj = value.as_object()?;
    aliases.iter().find_map(|key| {
        obj.get(*key).and_then(|value| match value {
            Value::Bool(flag) => Some(*flag),
            Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            },
            _ => None,
        })
    })
}

pub(super) fn read_value_number(value: &Value, aliases: &[&str]) -> Option<f64> {
    let obj = value.as_object()?;
    read_number_fields(obj, aliases)
}
