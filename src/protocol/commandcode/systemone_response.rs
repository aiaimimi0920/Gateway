//! 校验题目和答案的一一对应；仅从权威 usage 计费，不解释答案中的扩展字段。
use super::response_error;
use crate::{error::GatewayError, protocol::canonical::TokenUsage};
use serde_json::{Map, Value};

fn probability(value: &Value) -> bool {
    value
        .as_f64()
        .is_some_and(|number| number.is_finite() && (0.0..=1.0).contains(&number))
}

fn probabilities(value: &Value, criteria: &Map<String, Value>) -> bool {
    value.as_object().is_some_and(|values| {
        values.len() == criteria.len()
            && criteria
                .keys()
                .all(|key| values.get(key).is_some_and(probability))
    })
}

pub(super) fn usage(body: &Value) -> Result<TokenUsage, GatewayError> {
    let input = body["usage"]["input_tokens"]
        .as_u64()
        .ok_or_else(response_error)?;
    let output = body["usage"]["output_tokens"]
        .as_u64()
        .ok_or_else(response_error)?;
    Ok(TokenUsage {
        prompt_tokens: input,
        completion_tokens: output,
        total_tokens: input.checked_add(output).ok_or_else(response_error)?,
        cache_creation_input_tokens: None,
        cache_read_input_tokens: None,
    })
}

pub(super) fn validate(body: &Value, questions: &Map<String, Value>) -> Result<(), GatewayError> {
    if !body["model"]
        .as_str()
        .is_some_and(|model| !model.trim().is_empty())
    {
        return Err(response_error());
    }
    usage(body)?;
    let answers = body["answers"].as_object().ok_or_else(response_error)?;
    if answers.len() != questions.len() {
        return Err(response_error());
    }
    for (name, question) in questions {
        let answer = answers.get(name).ok_or_else(response_error)?;
        if answer["type"] != question["type"] {
            return Err(response_error());
        }
        let valid = match question["type"].as_str() {
            Some("noul") => probability(&answer["noul"]),
            Some("choice") => {
                let criteria = question["criteria"]
                    .as_object()
                    .ok_or_else(response_error)?;
                answer["choice"]
                    .as_str()
                    .is_some_and(|choice| criteria.contains_key(choice))
                    && probability(&answer["confidence"])
                    && probabilities(&answer["probabilities"], criteria)
            }
            Some("score") => {
                let criteria = question["criteria"].as_array().ok_or_else(response_error)?;
                let legend: Map<String, Value> = criteria
                    .iter()
                    .enumerate()
                    .map(|(index, value)| (index.to_string(), value.clone()))
                    .collect();
                answer["score"].as_f64().is_some_and(|score| {
                    score.is_finite() && score >= 0.0 && score <= (criteria.len() - 1) as f64
                }) && probability(&answer["confidence"])
                    && answer["legend"].as_object() == Some(&legend)
                    && probabilities(&answer["probabilities"], &legend)
            }
            _ => false,
        };
        // 不凭四舍五入的概率强制 sum=1 或重算 score，保留上游原始数值。
        if !valid {
            return Err(response_error());
        }
    }
    Ok(())
}
