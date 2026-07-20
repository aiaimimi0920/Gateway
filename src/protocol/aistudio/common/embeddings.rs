use base64::Engine;
use serde_json::{json, Value};

use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AIStudioEmbeddingsRequest {
    pub url: String,
    pub body: Value,
}

pub fn build_embeddings_request(
    req: &CanonicalRelayRequest,
    base_url: &str,
    model: &str,
) -> Result<AIStudioEmbeddingsRequest, GatewayError> {
    let raw_input = req
        .raw_body
        .get("input")
        .ok_or_else(|| {
            GatewayError::bad_request("AI Studio embeddings request is missing `input`.")
        })
        .and_then(parse_embeddings_input_texts)?;
    let output_dimensionality = req
        .raw_body
        .get("dimensions")
        .or_else(|| req.raw_body.get("outputDimensionality"))
        .and_then(Value::as_u64);
    let task_type = req
        .raw_body
        .get("task_type")
        .or_else(|| req.raw_body.get("taskType"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let title = req
        .raw_body
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let normalized_model = normalize_gemini_model_resource(model);
    let base_url = base_url.trim_end_matches('/');

    if raw_input.len() == 1 {
        let mut body = json!({
            "content": {
                "parts": [{
                    "text": raw_input[0],
                }]
            }
        });
        if let Some(dimensions) = output_dimensionality {
            body["outputDimensionality"] = json!(dimensions);
        }
        if let Some(task_type) = task_type {
            body["taskType"] = json!(task_type);
        }
        if let Some(title) = title {
            body["title"] = json!(title);
        }
        return Ok(AIStudioEmbeddingsRequest {
            url: format!("{base_url}/{normalized_model}:embedContent"),
            body,
        });
    }

    let requests = raw_input
        .into_iter()
        .map(|text| {
            let mut request = json!({
                "model": normalized_model,
                "content": {
                    "parts": [{
                        "text": text,
                    }]
                }
            });
            if let Some(dimensions) = output_dimensionality {
                request["outputDimensionality"] = json!(dimensions);
            }
            if let Some(task_type) = task_type.as_deref() {
                request["taskType"] = json!(task_type);
            }
            if let Some(title) = title.as_deref() {
                request["title"] = json!(title);
            }
            request
        })
        .collect::<Vec<_>>();
    Ok(AIStudioEmbeddingsRequest {
        url: format!("{base_url}/{normalized_model}:batchEmbedContents"),
        body: json!({ "requests": requests }),
    })
}

pub fn build_fixture_embeddings_response(
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<Value, GatewayError> {
    let inputs = req
        .raw_body
        .get("input")
        .ok_or_else(|| {
            GatewayError::bad_request("AI Studio fixture embeddings request is missing `input`.")
        })
        .and_then(parse_embeddings_input_texts)?;
    let encoding = requested_embedding_encoding(req);
    let data = inputs
        .iter()
        .enumerate()
        .map(|(index, input)| {
            let values = fixture_embedding_values(input);
            let embedding = serialize_embedding_values(&values, encoding.as_deref())?;
            Ok(json!({
                "object": "embedding",
                "index": index,
                "embedding": embedding,
            }))
        })
        .collect::<Result<Vec<_>, GatewayError>>()?;
    Ok(json!({
        "object": "list",
        "data": data,
        "model": model,
        "usage": {
            "prompt_tokens": inputs.len() as u64,
            "total_tokens": inputs.len() as u64,
        }
    }))
}

pub fn bridge_embeddings_response(
    req: &CanonicalRelayRequest,
    model: &str,
    response_value: &Value,
) -> Result<Value, GatewayError> {
    let encoding = requested_embedding_encoding(req);
    let embeddings = if let Some(single_entry) = response_value.get("embedding") {
        vec![extract_embedding_values(single_entry)?]
    } else if let Some(items) = response_value.get("embeddings").and_then(Value::as_array) {
        items
            .iter()
            .map(extract_embedding_values)
            .collect::<Result<Vec<_>, GatewayError>>()?
    } else {
        return Err(GatewayError::server_error(
            "AI Studio embeddings response did not include `embedding` or `embeddings` values.",
        )
        .with_code("aistudio_embeddings_missing_values"));
    };
    let data = embeddings
        .iter()
        .enumerate()
        .map(|(index, values)| {
            let embedding = serialize_embedding_values(values, encoding.as_deref())?;
            Ok(json!({
                "object": "embedding",
                "index": index,
                "embedding": embedding,
            }))
        })
        .collect::<Result<Vec<_>, GatewayError>>()?;
    let prompt_tokens = response_value
        .get("usageMetadata")
        .and_then(|usage| {
            usage
                .get("promptTokenCount")
                .or_else(|| usage.get("tokenCount"))
                .and_then(Value::as_u64)
        })
        .unwrap_or(embeddings.len() as u64);
    let total_tokens = response_value
        .get("usageMetadata")
        .and_then(|usage| usage.get("totalTokenCount").and_then(Value::as_u64))
        .unwrap_or(prompt_tokens);
    Ok(json!({
        "object": "list",
        "data": data,
        "model": model,
        "usage": {
            "prompt_tokens": prompt_tokens,
            "total_tokens": total_tokens,
        }
    }))
}

fn parse_embeddings_input_texts(input: &Value) -> Result<Vec<String>, GatewayError> {
    match input {
        Value::String(text) => Ok(vec![text.clone()]),
        Value::Array(items) => {
            let mut parsed = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    Value::String(text) => parsed.push(text.clone()),
                    _ => {
                        return Err(GatewayError::bad_request(
                            "AI Studio embeddings currently support only string or string-array `input` values.",
                        )
                        .with_code("aistudio_embeddings_unsupported_input"));
                    }
                }
            }
            if parsed.is_empty() {
                return Err(GatewayError::bad_request(
                    "AI Studio embeddings request `input` cannot be empty.",
                )
                .with_code("aistudio_embeddings_empty_input"));
            }
            Ok(parsed)
        }
        _ => Err(GatewayError::bad_request(
            "AI Studio embeddings currently support only string or string-array `input` values.",
        )
        .with_code("aistudio_embeddings_unsupported_input")),
    }
}

fn normalize_gemini_model_resource(model: &str) -> String {
    let trimmed = model.trim().trim_start_matches('/');
    if trimmed.starts_with("models/") {
        trimmed.to_string()
    } else {
        format!("models/{trimmed}")
    }
}

fn requested_embedding_encoding(req: &CanonicalRelayRequest) -> Option<&str> {
    req.raw_body
        .get("encoding_format")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn fixture_embedding_values(input: &str) -> Vec<f64> {
    let mut state = 0x811C9DC5u32;
    for byte in input.as_bytes() {
        state ^= u32::from(*byte);
        state = state.wrapping_mul(16777619);
    }
    (0..8)
        .map(|index| {
            let lane = state.rotate_left((index * 5) as u32);
            (f64::from((lane % 10_000) as u16) / 10_000.0) * 2.0 - 1.0
        })
        .collect()
}

fn extract_embedding_values(value: &Value) -> Result<Vec<f64>, GatewayError> {
    let values = value
        .get("values")
        .or_else(|| value.get("embedding").and_then(|inner| inner.get("values")))
        .and_then(Value::as_array)
        .ok_or_else(|| {
            GatewayError::server_error(
                "AI Studio embeddings response entry did not include `values`.",
            )
            .with_code("aistudio_embeddings_missing_values")
        })?;
    values
        .iter()
        .map(|entry| {
            entry.as_f64().ok_or_else(|| {
                GatewayError::server_error(
                    "AI Studio embeddings response included a non-numeric embedding value.",
                )
                .with_code("aistudio_embeddings_invalid_value")
            })
        })
        .collect()
}

fn serialize_embedding_values(
    values: &[f64],
    encoding: Option<&str>,
) -> Result<Value, GatewayError> {
    if encoding.is_some_and(|value| value.eq_ignore_ascii_case("base64")) {
        let mut bytes = Vec::with_capacity(values.len() * std::mem::size_of::<f32>());
        for value in values {
            bytes.extend_from_slice(&(*value as f32).to_le_bytes());
        }
        return Ok(json!(
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ));
    }

    let numbers = values
        .iter()
        .map(|value| {
            serde_json::Number::from_f64(*value)
                .map(Value::Number)
                .ok_or_else(|| {
                    GatewayError::server_error(
                        "AI Studio embeddings response produced a non-finite numeric value.",
                    )
                    .with_code("aistudio_embeddings_nonfinite_value")
                })
        })
        .collect::<Result<Vec<_>, GatewayError>>()?;
    Ok(Value::Array(numbers))
}
