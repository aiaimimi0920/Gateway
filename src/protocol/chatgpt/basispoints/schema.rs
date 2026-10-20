//! Bounded client-tool schema subset, not a general JSON Schema engine.
use serde_json::Value;

pub(super) fn accepts(schema: &Value, value: &Value) -> bool {
    matches(schema, value, schema, 0, &mut 8192)
}

fn matches(schema: &Value, value: &Value, root: &Value, depth: usize, budget: &mut usize) -> bool {
    if depth > 24 || *budget == 0 {
        return false;
    }
    *budget -= 1;
    if let Some(allowed) = schema.as_bool() {
        return allowed;
    }
    if !schema.is_object() {
        return false;
    }
    if let Some(reference) = schema.get("$ref") {
        let Some(path) = reference.as_str().and_then(|s| s.strip_prefix('#')) else {
            return false;
        };
        if !root
            .pointer(path)
            .is_some_and(|target| matches(target, value, root, depth + 1, budget))
        {
            return false;
        }
    }
    if let Some(kind) = schema.get("type") {
        let valid = |kind: &str| match kind {
            "null" => value.is_null(),
            "boolean" => value.is_boolean(),
            "string" => value.is_string(),
            "object" => value.is_object(),
            "array" => value.is_array(),
            "number" => value.is_number(),
            "integer" => value
                .as_f64()
                .is_some_and(|n| n.is_finite() && n.fract() == 0.0),
            _ => false,
        };
        if !kind.as_str().map(valid).unwrap_or_else(|| {
            kind.as_array()
                .is_some_and(|kinds| kinds.iter().any(|kind| kind.as_str().is_some_and(valid)))
        }) {
            return false;
        }
    }
    if schema
        .get("const")
        .is_some_and(|constant| constant != value)
    {
        return false;
    }
    if schema.get("enum").is_some_and(|choices| {
        !choices
            .as_array()
            .is_some_and(|choices| choices.contains(value))
    }) {
        return false;
    }
    for keyword in ["allOf", "anyOf", "oneOf"] {
        if let Some(variants) = schema.get(keyword) {
            let Some(variants) = variants
                .as_array()
                .filter(|v| !v.is_empty() && v.len() <= 256)
            else {
                return false;
            };
            let count = variants
                .iter()
                .filter(|variant| matches(variant, value, root, depth + 1, budget))
                .count();
            if match keyword {
                "allOf" => count != variants.len(),
                "oneOf" => count != 1,
                _ => count == 0,
            } {
                return false;
            }
        }
    }
    if let Some(object) = value.as_object() {
        if let Some(required) = schema.get("required") {
            let Some(required) = required.as_array() else {
                return false;
            };
            if required
                .iter()
                .any(|key| !key.as_str().is_some_and(|key| object.contains_key(key)))
            {
                return false;
            }
        }
        for (key, value) in object {
            if let Some(property) = schema.get("properties").and_then(|p| p.get(key)) {
                if !matches(property, value, root, depth + 1, budget) {
                    return false;
                }
            } else if let Some(additional) = schema.get("additionalProperties") {
                if !matches(additional, value, root, depth + 1, budget) {
                    return false;
                }
            }
        }
    }
    if let (Some(items), Some(array)) = (schema.get("items"), value.as_array()) {
        if array
            .iter()
            .any(|value| !matches(items, value, root, depth + 1, budget))
        {
            return false;
        }
    }
    true
}
