//! System One 请求边界；仅覆盖 Command Code 公开的三类问题，不推断 TypeSafe beta 权限。
use crate::error::GatewayError;
use serde_json::{Map, Value};

fn invalid() -> GatewayError {
    GatewayError::bad_request("Invalid Command Code System One request.")
        .with_code("invalid_commandcode_systemone")
}

fn structured(value: &Value) -> bool {
    matches!(value, Value::String(_) | Value::Object(_) | Value::Array(_))
}

fn nullable_structured(value: &Value) -> bool {
    value.is_null() || structured(value)
}

pub(super) fn validate(body: &Value) -> Result<&Map<String, Value>, GatewayError> {
    let object = body.as_object().ok_or_else(invalid)?;
    if !body["model"].as_str().is_some_and(|s| !s.trim().is_empty())
        || !structured(&body["state"])
        || object
            .keys()
            .any(|key| !matches!(key.as_str(), "model" | "state" | "questions" | "stream"))
        || object.get("stream").is_some_and(|value| value != false)
    {
        return Err(invalid());
    }
    let questions = body["questions"].as_object().ok_or_else(invalid)?;
    if !(1..=100).contains(&questions.len()) {
        return Err(invalid());
    }
    for question in questions.values() {
        let object = question.as_object().ok_or_else(invalid)?;
        if object
            .keys()
            .any(|key| !matches!(key.as_str(), "type" | "instructions" | "criteria"))
            || !nullable_structured(&question["instructions"])
        {
            return Err(invalid());
        }
        let criteria = &question["criteria"];
        let valid = match question["type"].as_str() {
            Some("noul") => {
                criteria.is_null()
                    || criteria.as_object().is_some_and(|values| {
                        values.iter().all(|(key, value)| {
                            matches!(key.as_str(), "true" | "false") && nullable_structured(value)
                        })
                    })
            }
            Some("choice") => criteria.as_object().is_some_and(|values| {
                (1..=255).contains(&values.len()) && values.values().all(nullable_structured)
            }),
            // Provider 文档要求至少两级；TypeSafe 直连的单级 schema 不覆盖该产品约束。
            Some("score") => criteria.as_array().is_some_and(|values| {
                (2..=10).contains(&values.len()) && values.iter().all(structured)
            }),
            _ => false,
        };
        if !valid {
            return Err(invalid());
        }
    }
    Ok(questions)
}

/// 只用于过滤与预估；发送路径必须继续使用结构化 state/questions，不能发送合成聊天历史。
pub(super) fn projection(body: &Value) -> String {
    serde_json::json!({"state": body["state"], "questions": body["questions"]}).to_string()
}
