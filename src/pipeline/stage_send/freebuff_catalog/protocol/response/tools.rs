//! Explicit function conversion: no XML parsing, default arguments, opaque extension loss or tools.
use super::*;
pub(super) fn project(
    call: Value,
    messages: bool,
    ids: &mut std::collections::HashSet<String>,
) -> Result<(Value, Value), BusinessError> {
    let invalid = || BusinessError::InvalidResponse;
    let Value::Object(mut extra) = call else {
        return Err(invalid());
    };
    let id = extra
        .remove("id")
        .and_then(|v| v.as_str().map(str::to_owned))
        .filter(|s| !s.is_empty())
        .ok_or_else(invalid)?;
    if !ids.insert(id.clone()) {
        return Err(invalid());
    }
    if extra
        .remove("type")
        .is_some_and(|v| !v.is_null() && v != "function")
    {
        return Err(invalid());
    }
    let Value::Object(mut function) = extra.remove("function").ok_or_else(invalid)? else {
        return Err(invalid());
    };
    let name = function
        .remove("name")
        .filter(|v| v.as_str().is_some_and(|s| !s.is_empty()))
        .ok_or_else(invalid)?;
    let arguments = function
        .remove("arguments")
        .filter(Value::is_string)
        .ok_or_else(invalid)?;
    let mut part = if messages {
        let input: Value = serde_json::from_str(arguments.as_str().expect("checked arguments"))
            .map_err(|_| invalid())?;
        if !input.is_object() {
            return Err(invalid());
        }
        // Anthropic input is JSON, not the original argument string. Keep lexical native bytes too.
        function.insert("arguments".into(), arguments);
        let mut part = json!({"type":"tool_use"});
        part["input"] = input;
        part
    } else {
        let mut part = json!({"type":"function_call","call_id":id});
        part["arguments"] = arguments;
        part
    };
    part["id"] = Value::String(id);
    part["name"] = name;
    if !function.is_empty() {
        extra.insert("function".into(), Value::Object(function));
    }
    Ok((part, Value::Object(extra)))
}
