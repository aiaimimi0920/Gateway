//! Caller tool declarations own the mapping; native MCP always carries an object.
//! Custom text uses exactly one string field, never a guessed or decoded text call.
use crate::error::GatewayError;
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolKind {
    Function,
    CustomText,
}

impl ToolKind {
    pub(crate) fn call_type(self) -> &'static str {
        match self {
            Self::Function => "function_call",
            Self::CustomText => "custom_tool_call",
        }
    }

    pub(crate) fn output_type(self) -> &'static str {
        match self {
            Self::Function => "function_call_output",
            Self::CustomText => "custom_tool_call_output",
        }
    }

    pub(crate) fn input_field(self) -> &'static str {
        match self {
            Self::Function => "arguments",
            Self::CustomText => "input",
        }
    }

    pub(crate) fn canonical_type(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::CustomText => "custom",
        }
    }

    pub(crate) fn native_input(self, arguments: String) -> Result<String, GatewayError> {
        if self == Self::Function {
            return Ok(arguments);
        }
        // Deserializing a struct also rejects duplicate input keys. Do not trim,
        // JSON-normalize or HTML-decode the inner string after removing the MCP envelope.
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct TextInput {
            input: String,
        }
        let value: TextInput = serde_json::from_str(&arguments)
            .map_err(|_| super::error("invalid_custom_tool_input"))?;
        Ok(value.input)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Definition {
    pub name: String,
    pub kind: ToolKind,
    pub wire: Value,
}

pub(super) fn invalid() -> GatewayError {
    crate::routing::config::cursor::stop(
        GatewayError::bad_request(
            "Cursor remote requires non-strict functions or custom text tools, exact live tool history and bounded text output",
        )
        .with_code("cursor_remote_unsupported_tool_request"),
    )
}

pub(super) fn definitions(value: &Value) -> Result<Vec<Definition>, GatewayError> {
    let tools = value.as_array().ok_or_else(invalid)?;
    if tools.is_empty() || tools.len() > 64 || value.to_string().len() > 96 * 1024 {
        return Err(invalid());
    }
    let mut names = std::collections::HashSet::new();
    tools
        .iter()
        .map(|tool| {
            let object = tool.as_object().ok_or_else(invalid)?;
            let name = tool["name"].as_str().ok_or_else(invalid)?;
            if name.is_empty()
                || name.len() > 64
                || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                || !name.as_bytes()[0].is_ascii_alphabetic() && name.as_bytes()[0] != b'_'
                || !names.insert(name)
            {
                return Err(invalid());
            }
            let description = match tool.get("description") {
                None => "",
                Some(value) => value.as_str().filter(|s| s.len() <= 32768).ok_or_else(invalid)?,
            };
            let (kind, parameters, allowed): (ToolKind, Value, &[&str]) =
                match tool["type"].as_str() {
                    Some("function") => {
                        if tool.get("strict").is_some_and(|v| v != false && !v.is_null())
                            || !tool["parameters"].is_object()
                            || tool["parameters"].to_string().len() > 32768
                        {
                            return Err(invalid());
                        }
                        (
                            ToolKind::Function,
                            tool["parameters"].clone(),
                            &["type", "name", "description", "strict", "parameters"],
                        )
                    }
                    Some("custom") => {
                        // Omitted format means unconstrained text; no grammar downgrade.
                        if tool.get("format").is_some_and(|v| v != &json!({"type":"text"})) {
                            return Err(invalid());
                        }
                        (
                            ToolKind::CustomText,
                            json!({"type":"object","properties":{"input":{"type":"string",
                                "description":"Exact raw input text for this custom tool. Preserve all whitespace and characters."}},
                                "required":["input"],"additionalProperties":false}),
                            &["type", "name", "description", "format"],
                        )
                    }
                    _ => return Err(invalid()),
                };
            if object.keys().any(|key| !allowed.contains(&key.as_str())) {
                return Err(invalid());
            }
            Ok(Definition {
                name: name.into(),
                kind,
                wire: json!({"name":name,"description":description,"parameters":parameters}),
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "tool_schema_tests.rs"]
mod tests;
