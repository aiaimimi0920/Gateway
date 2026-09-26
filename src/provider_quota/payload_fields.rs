use serde_json::Value;
pub(super) fn round_percent(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

pub(super) fn find_numeric_field(value: &Value, candidate_keys: &[&str]) -> Option<f64> {
    match value {
        Value::Object(map) => {
            for key in candidate_keys {
                if let Some(number) = map.get(*key).and_then(value_as_f64) {
                    return Some(number);
                }
            }
            map.values()
                .find_map(|child| find_numeric_field(child, candidate_keys))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|child| find_numeric_field(child, candidate_keys)),
        _ => None,
    }
}

pub(super) fn find_bool_field(value: &Value, candidate_keys: &[&str]) -> Option<bool> {
    match value {
        Value::Object(map) => {
            for key in candidate_keys {
                if let Some(flag) = map.get(*key).and_then(value_as_bool) {
                    return Some(flag);
                }
            }
            map.values()
                .find_map(|child| find_bool_field(child, candidate_keys))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|child| find_bool_field(child, candidate_keys)),
        _ => None,
    }
}

pub(super) fn find_status_string(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            for key in ["status", "state", "quota_status"] {
                if let Some(status) = map.get(key).and_then(Value::as_str) {
                    let status = status.trim().to_lowercase();
                    if !status.is_empty() {
                        return Some(status);
                    }
                }
            }
            map.values().find_map(find_status_string)
        }
        Value::Array(values) => values.iter().find_map(find_status_string),
        _ => None,
    }
}

fn value_as_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(value) => value.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn value_as_bool(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(value) => Some(*value),
        Value::String(value) => match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" => Some(true),
            "false" | "0" | "no" => Some(false),
            _ => None,
        },
        _ => None,
    }
}
