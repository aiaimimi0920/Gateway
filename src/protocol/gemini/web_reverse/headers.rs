use std::collections::HashMap;

use serde_json::{Map, Value};

use super::{
    GEMINI_WEB_MODEL_HEADER_2_KEY, GEMINI_WEB_MODEL_HEADER_3_KEY, GEMINI_WEB_MODEL_HEADER_KEY,
    GEMINI_WEB_REQUEST_CONTEXT_HEADER_KEY,
};

pub fn extract_model_headers(
    extra_body: Option<&HashMap<String, Value>>,
    model: &str,
) -> Vec<(String, String)> {
    let Some(extra_body) = extra_body else {
        return Vec::new();
    };

    let mut headers = Vec::new();
    if let Some(model_headers) = extra_body.get("modelHeaders").and_then(Value::as_object) {
        if let Some(model_specific) = model_headers.get(model).and_then(Value::as_object) {
            headers.extend(extract_header_object(model_specific));
        }
    }
    if headers.is_empty() {
        if let Some(model_header) = extra_body
            .get("modelHeader")
            .or_else(|| extra_body.get("model_header"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            headers.push((
                GEMINI_WEB_MODEL_HEADER_KEY.to_string(),
                model_header.to_string(),
            ));
        }
        if let Some(model_header_2) = extra_body
            .get("modelHeader2")
            .or_else(|| extra_body.get("model_header_2"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            headers.push((
                GEMINI_WEB_MODEL_HEADER_2_KEY.to_string(),
                model_header_2.to_string(),
            ));
        }
        if let Some(model_header_3) = extra_body
            .get("modelHeader3")
            .or_else(|| extra_body.get("model_header_3"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            headers.push((
                GEMINI_WEB_MODEL_HEADER_3_KEY.to_string(),
                model_header_3.to_string(),
            ));
        }
    }
    if let Some(request_context) = extra_body
        .get("requestContextHeader")
        .or_else(|| extra_body.get("request_context_header"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        headers.push((
            GEMINI_WEB_REQUEST_CONTEXT_HEADER_KEY.to_string(),
            request_context.to_string(),
        ));
    }
    headers
}

fn extract_header_object(object: &Map<String, Value>) -> Vec<(String, String)> {
    object
        .iter()
        .filter_map(|(key, value)| {
            value
                .as_str()
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .map(|entry| (key.clone(), entry.to_string()))
        })
        .collect()
}
