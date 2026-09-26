//! Keepalive metadata readers and diagnostic summaries.

use serde_json::Value;
use std::collections::HashMap;

pub(super) fn truncate_error_summary(value: &str, max_length: usize) -> String {
    let message = value.trim();
    if message.len() <= max_length {
        return message.to_string();
    }

    let mut truncated = message
        .chars()
        .take(max_length.saturating_sub(1))
        .collect::<String>();
    truncated.push('…');
    truncated
}

pub(super) fn read_extra_body_string(
    extra_body: Option<&HashMap<String, Value>>,
    aliases: &[&str],
) -> Option<String> {
    let extra_body = extra_body?;
    let normalized_aliases = aliases
        .iter()
        .map(|alias| alias.replace(['_', '-'], "").to_ascii_lowercase())
        .collect::<Vec<_>>();
    for (key, value) in extra_body {
        let normalized_key = key.replace(['_', '-'], "").to_ascii_lowercase();
        if !normalized_aliases.contains(&normalized_key) {
            continue;
        }
        match value {
            Value::String(text) if !text.trim().is_empty() => return Some(text.trim().to_string()),
            Value::Number(number) => return Some(number.to_string()),
            Value::Bool(flag) => return Some(flag.to_string()),
            _ => {}
        }
    }
    None
}

pub(super) fn read_extra_body_bool(
    extra_body: Option<&HashMap<String, Value>>,
    aliases: &[&str],
) -> Option<bool> {
    let text = read_extra_body_string(extra_body, aliases)?;
    match text.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" | "enabled" => Some(true),
        "0" | "false" | "no" | "off" | "disabled" | "never" => Some(false),
        _ => None,
    }
}

pub(super) fn read_nested_json_string(value: Option<&Value>, aliases: &[&str]) -> Option<String> {
    let object = value?.as_object()?;
    let normalized_aliases = aliases
        .iter()
        .map(|alias| alias.replace(['_', '-'], "").to_ascii_lowercase())
        .collect::<Vec<_>>();
    for (key, value) in object {
        let normalized_key = key.replace(['_', '-'], "").to_ascii_lowercase();
        if !normalized_aliases.contains(&normalized_key) {
            continue;
        }
        if let Some(text) = value
            .as_str()
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(str::to_string)
        {
            return Some(text);
        }
    }
    None
}

pub(super) fn read_json_field_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)?
        .as_str()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string)
}
