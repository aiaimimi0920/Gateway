//! Native parameter subset, checked before quote. Unknown/unsupported fields never silently vanish.
use super::*;
pub(super) fn validate(body: &Value) -> Result<(), BusinessError> {
    for key in ["max_tokens", "max_completion_tokens"] {
        if body
            .get(key)
            .is_some_and(|v| v.as_u64().is_none_or(|n| n == 0))
        {
            return Err(BusinessError::InvalidInput);
        }
    }
    for (key, min, max) in [
        ("temperature", 0.0, 2.0),
        ("top_p", 0.0, 1.0),
        ("frequency_penalty", -2.0, 2.0),
        ("presence_penalty", -2.0, 2.0),
    ] {
        if body
            .get(key)
            .is_some_and(|v| v.as_f64().is_none_or(|n| !(min..=max).contains(&n)))
        {
            return Err(BusinessError::InvalidInput);
        }
    }
    if body
        .get("parallel_tool_calls")
        .is_some_and(|v| !v.is_boolean())
        || body.get("seed").is_some_and(|v| v.as_i64().is_none())
        || body.get("user").is_some_and(|v| !v.is_string())
        || body.get("reasoning_effort").is_some_and(|v| {
            !matches!(
                v.as_str(),
                Some("none" | "minimal" | "low" | "medium" | "high" | "xhigh")
            )
        })
    {
        return Err(BusinessError::InvalidInput);
    }
    if let Some(stop) = body.get("stop") {
        let text = |v: &Value| v.as_str().is_some_and(|s| !s.is_empty());
        if !text(stop)
            && !stop
                .as_array()
                .is_some_and(|values| (1..=4).contains(&values.len()) && values.iter().all(text))
        {
            return Err(BusinessError::InvalidInput);
        }
    }
    if let Some(choice) = body.get("tool_choice") {
        let tools = body["tools"].as_array();
        match choice.as_str() {
            Some("none") => {}
            Some("auto" | "required") if tools.is_some_and(|v| !v.is_empty()) => {}
            None if choice.is_object() => {
                fields(choice, &["type", "function"])?;
                fields(&choice["function"], &["name"])?;
                let name = choice
                    .pointer("/function/name")
                    .and_then(Value::as_str)
                    .ok_or(BusinessError::InvalidInput)?;
                if choice["type"] != "function"
                    || !tools.is_some_and(|values| {
                        values.iter().any(|tool| {
                            tool.pointer("/function/name").and_then(Value::as_str) == Some(name)
                        })
                    })
                {
                    return Err(BusinessError::InvalidInput);
                }
            }
            _ => return Err(BusinessError::InvalidInput),
        }
    }
    Ok(())
}
