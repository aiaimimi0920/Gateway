//! Suno JSON field aliases and typed scalar decoding.

use serde_json::Map;
use serde_json::Value;

pub(super) fn read_value_string(value: &Value, aliases: &[&str]) -> Option<String> {
    let obj = value.as_object()?;
    read_optional_string(obj, aliases)
}

pub(super) fn read_optional_string(obj: &Map<String, Value>, aliases: &[&str]) -> Option<String> {
    aliases
        .iter()
        .find_map(|key| obj.get(*key).and_then(|value| value.as_str()))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
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

pub(super) fn read_optional_i64(obj: &Map<String, Value>, aliases: &[&str]) -> Option<i64> {
    aliases.iter().find_map(|key| {
        obj.get(*key).and_then(|value| match value {
            Value::Number(number) => number.as_i64(),
            Value::String(text) => text.trim().parse::<i64>().ok(),
            _ => None,
        })
    })
}

pub(super) fn read_optional_bool(obj: &Map<String, Value>, aliases: &[&str]) -> Option<bool> {
    aliases.iter().find_map(|key| {
        obj.get(*key).and_then(|value| match value {
            Value::Bool(flag) => Some(*flag),
            Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" => Some(true),
                "false" | "0" | "no" => Some(false),
                _ => None,
            },
            _ => None,
        })
    })
}
