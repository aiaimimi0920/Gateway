//! Gemini declarations have two distinct schema dialects. Only the documented
//! equivalent OpenAPI subset is translated; JSON Schema stays JSON Schema.
use super::tool_schema::invalid;
use crate::error::GatewayError;
use serde_json::{json, Map, Value};

pub(super) fn keys(value: &Value, allowed: &[&str]) -> Result<(), GatewayError> {
    if value
        .as_object()
        .is_none_or(|fields| fields.keys().any(|key| !allowed.contains(&key.as_str())))
    {
        return Err(invalid());
    }
    Ok(())
}

pub(super) fn declarations(value: &Value) -> Result<Value, GatewayError> {
    let groups = value
        .as_array()
        .filter(|a| a.len() <= 64)
        .ok_or_else(invalid)?;
    let mut result = Vec::new();
    for group in groups {
        keys(group, &["functionDeclarations"])?;
        let declarations = group["functionDeclarations"]
            .as_array()
            .ok_or_else(invalid)?;
        for declaration in declarations {
            keys(
                declaration,
                &["name", "description", "parameters", "parametersJsonSchema"],
            )?;
            if result.len() >= 64
                || declaration.get("parameters").is_some()
                    && declaration.get("parametersJsonSchema").is_some()
            {
                return Err(invalid());
            }
            let parameters = match (
                declaration.get("parameters"),
                declaration.get("parametersJsonSchema"),
            ) {
                (Some(schema), None) => schema_node(schema, 0)?,
                (None, Some(schema)) => {
                    if schema.to_string().len() > 32768 || !schema.is_object() {
                        return Err(invalid());
                    }
                    reject_ordering(schema, 0)?;
                    schema.clone()
                }
                (None, None) => json!({"type":"object","properties":{}}),
                _ => return Err(invalid()),
            };
            if parameters["type"] != "object" {
                return Err(invalid());
            }
            let mut tool =
                json!({"type":"function","name":declaration["name"],"parameters":parameters});
            if let Some(description) = declaration.get("description") {
                tool["description"] = description.clone();
            }
            result.push(tool);
        }
    }
    Ok(json!(result))
}

fn schema_node(value: &Value, depth: usize) -> Result<Value, GatewayError> {
    if depth > 32 || value.to_string().len() > 32768 {
        return Err(invalid());
    }
    keys(
        value,
        &[
            "type",
            "description",
            "properties",
            "required",
            "items",
            "enum",
        ],
    )?;
    let kind = match value["type"].as_str() {
        Some("STRING") => "string",
        Some("NUMBER") => "number",
        Some("INTEGER") => "integer",
        Some("BOOLEAN") => "boolean",
        Some("ARRAY") => "array",
        Some("OBJECT") => "object",
        Some("NULL") => "null",
        _ => return Err(invalid()),
    };
    let mut result = json!({"type":kind});
    if let Some(description) = value.get("description") {
        if !description.is_string() {
            return Err(invalid());
        }
        result["description"] = description.clone();
    }
    if let Some(properties) = value.get("properties") {
        if kind != "object" {
            return Err(invalid());
        }
        let mut mapped = Map::new();
        for (name, schema) in properties.as_object().ok_or_else(invalid)? {
            mapped.insert(name.clone(), schema_node(schema, depth + 1)?);
        }
        result["properties"] = Value::Object(mapped);
    }
    if let Some(required) = value.get("required") {
        if kind != "object"
            || required
                .as_array()
                .is_none_or(|a| a.iter().any(|v| !v.is_string()))
        {
            return Err(invalid());
        }
        result["required"] = required.clone();
    }
    if let Some(items) = value.get("items") {
        if kind != "array" {
            return Err(invalid());
        }
        result["items"] = schema_node(items, depth + 1)?;
    }
    if let Some(values) = value.get("enum") {
        if kind != "string"
            || values
                .as_array()
                .is_none_or(|a| a.is_empty() || a.iter().any(|v| !v.is_string()))
        {
            return Err(invalid());
        }
        result["enum"] = values.clone();
    }
    Ok(result)
}

// Do not advertise Google's propertyOrdering extension as a Cursor constraint.
// Traverse schema-bearing keys only, never properties' names or literal values.
fn reject_ordering(schema: &Value, depth: usize) -> Result<(), GatewayError> {
    if depth > 32 {
        return Err(invalid());
    }
    let Some(object) = schema.as_object() else {
        return Ok(());
    };
    if object.contains_key("propertyOrdering") {
        return Err(invalid());
    }
    for (key, value) in object {
        match key.as_str() {
            "properties" | "patternProperties" | "$defs" | "definitions" | "dependentSchemas"
            | "dependencies" => {
                if let Some(map) = value.as_object() {
                    for child in map.values() {
                        reject_ordering(child, depth + 1)?;
                    }
                }
            }
            "items" if value.is_array() => {
                for child in value.as_array().unwrap() {
                    reject_ordering(child, depth + 1)?;
                }
            }
            "items"
            | "additionalItems"
            | "additionalProperties"
            | "contains"
            | "not"
            | "if"
            | "then"
            | "else"
            | "unevaluatedProperties"
            | "unevaluatedItems"
            | "propertyNames" => {
                reject_ordering(value, depth + 1)?;
            }
            "allOf" | "anyOf" | "oneOf" | "prefixItems" => {
                if let Some(array) = value.as_array() {
                    for child in array {
                        reject_ordering(child, depth + 1)?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}
