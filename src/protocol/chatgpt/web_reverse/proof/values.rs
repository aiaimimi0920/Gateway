//! Turnstile values, ordered-object semantics and wire conversions.
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use serde_json::{json, Value};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct OrderedTurnstileMap {
    entries: Vec<(String, TurnstileValue)>,
}

impl OrderedTurnstileMap {
    pub(super) fn entries(&self) -> &[(String, TurnstileValue)] {
        &self.entries
    }

    pub(super) fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub(super) fn add(&mut self, key: String, value: TurnstileValue) {
        if let Some((_, existing)) = self
            .entries
            .iter_mut()
            .find(|(existing_key, _)| existing_key == &key)
        {
            *existing = value;
            return;
        }
        self.entries.push((key, value));
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum TurnstileValue {
    Undefined,
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<TurnstileValue>),
    Object(Vec<(String, TurnstileValue)>),
    OrderedMap(OrderedTurnstileMap),
    Func(i64),
}

pub(super) fn turnstile_value_from_json(value: &Value) -> TurnstileValue {
    match value {
        Value::Null => TurnstileValue::Null,
        Value::Bool(value) => TurnstileValue::Bool(*value),
        Value::Number(value) => TurnstileValue::Number(value.as_f64().unwrap_or_default()),
        Value::String(value) => TurnstileValue::String(value.clone()),
        Value::Array(values) => {
            TurnstileValue::Array(values.iter().map(turnstile_value_from_json).collect())
        }
        Value::Object(values) => TurnstileValue::Object(
            values
                .iter()
                .map(|(key, value)| (key.clone(), turnstile_value_from_json(value)))
                .collect(),
        ),
    }
}

pub(super) fn turnstile_value_to_json(value: &TurnstileValue) -> Option<Value> {
    match value {
        TurnstileValue::Undefined | TurnstileValue::Func(_) => Some(Value::Null),
        TurnstileValue::Null => Some(Value::Null),
        TurnstileValue::Bool(value) => Some(json!(*value)),
        TurnstileValue::Number(value) => serde_json::Number::from_f64(*value).map(Value::Number),
        TurnstileValue::String(value) => Some(Value::String(value.clone())),
        TurnstileValue::Array(values) => values
            .iter()
            .map(turnstile_value_to_json)
            .collect::<Option<Vec<_>>>()
            .map(Value::Array),
        TurnstileValue::Object(values) => {
            let mut map = serde_json::Map::new();
            for (key, value) in values {
                map.insert(key.clone(), turnstile_value_to_json(value)?);
            }
            Some(Value::Object(map))
        }
        TurnstileValue::OrderedMap(map) => {
            let mut object = serde_json::Map::new();
            for (key, value) in &map.entries {
                object.insert(key.clone(), turnstile_value_to_json(value)?);
            }
            Some(Value::Object(object))
        }
    }
}

pub(super) fn turnstile_key_from_json(value: &Value) -> Option<String> {
    if let Some(value) = value.as_i64() {
        return Some(turnstile_key_from_i64(value));
    }
    if let Some(value) = value.as_u64() {
        return Some(value.to_string());
    }
    value.as_f64().map(turnstile_key_from_f64)
}

pub(super) fn turnstile_key_from_i64(value: i64) -> String {
    value.to_string()
}

fn turnstile_key_from_f64(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value}")
    }
}

pub(super) fn turnstile_to_string(value: &TurnstileValue) -> String {
    match value {
        TurnstileValue::Undefined | TurnstileValue::Null => "undefined".to_string(),
        TurnstileValue::Bool(value) => {
            if *value {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        TurnstileValue::Number(value) => format_turnstile_number(*value),
        TurnstileValue::String(value) => match value.as_str() {
            "window.Math" => "[object Math]".to_string(),
            "window.Reflect" => "[object Reflect]".to_string(),
            "window.performance" => "[object Performance]".to_string(),
            "window.localStorage" => "[object Storage]".to_string(),
            "window.Object" => "function Object() { [native code] }".to_string(),
            "window.Reflect.set" => "function set() { [native code] }".to_string(),
            "window.performance.now" => "function () { [native code] }".to_string(),
            "window.Object.create" => "function create() { [native code] }".to_string(),
            "window.Object.keys" => "function keys() { [native code] }".to_string(),
            "window.Math.random" => "function random() { [native code] }".to_string(),
            _ => value.clone(),
        },
        TurnstileValue::Array(values)
            if values
                .iter()
                .all(|value| matches!(value, TurnstileValue::String(_))) =>
        {
            values
                .iter()
                .map(turnstile_to_string)
                .collect::<Vec<_>>()
                .join(",")
        }
        TurnstileValue::Array(values) => {
            let inner = values
                .iter()
                .map(turnstile_to_string)
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{inner}]")
        }
        TurnstileValue::Object(values) => {
            let inner = values
                .iter()
                .map(|(key, value)| format!("'{key}': {}", turnstile_to_string(value)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{{inner}}}")
        }
        TurnstileValue::OrderedMap(_) => "<OrderedMap>".to_string(),
        TurnstileValue::Func(code) => format!("<function {code}>"),
    }
}

pub(super) fn turnstile_is_string_or_number(value: &TurnstileValue) -> bool {
    matches!(value, TurnstileValue::String(_) | TurnstileValue::Number(_))
}

fn format_turnstile_number(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value}")
    }
}

pub(super) fn py_json_dumps_turnstile(value: &TurnstileValue) -> String {
    match value {
        TurnstileValue::Undefined | TurnstileValue::Null | TurnstileValue::Func(_) => {
            "null".to_string()
        }
        TurnstileValue::Bool(value) => value.to_string(),
        TurnstileValue::Number(value) => format_turnstile_number(*value),
        TurnstileValue::String(value) => {
            serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
        }
        TurnstileValue::Array(values) => {
            let inner = values
                .iter()
                .map(py_json_dumps_turnstile)
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{inner}]")
        }
        TurnstileValue::Object(values) => {
            let inner = values
                .iter()
                .map(|(key, value)| {
                    let key = serde_json::to_string(key).unwrap_or_else(|_| "\"\"".to_string());
                    format!("{key}: {}", py_json_dumps_turnstile(value))
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{{inner}}}")
        }
        TurnstileValue::OrderedMap(map) => {
            let inner = map
                .entries
                .iter()
                .map(|(key, value)| {
                    let key = serde_json::to_string(key).unwrap_or_else(|_| "\"\"".to_string());
                    format!("{key}: {}", py_json_dumps_turnstile(value))
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{{inner}}}")
        }
    }
}

pub(super) fn xor_string(text: &str, key: &str) -> Option<String> {
    if key.is_empty() {
        return Some(text.to_string());
    }
    let key = key.as_bytes();
    let bytes = text
        .as_bytes()
        .iter()
        .enumerate()
        .map(|(index, value)| value ^ key[index % key.len()])
        .collect::<Vec<_>>();
    String::from_utf8(bytes).ok()
}

pub(super) fn decode_base64_compat(text: &str) -> Option<Vec<u8>> {
    let mut normalized = text.trim().to_string();
    while normalized.len() % 4 != 0 {
        normalized.push('=');
    }
    BASE64_STANDARD.decode(normalized).ok()
}
