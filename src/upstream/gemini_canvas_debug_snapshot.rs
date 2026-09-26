use super::gemini_canvas_debug_redaction::sanitize_snapshot_pairs;
use rquest::header::HeaderMap;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};

#[cfg(test)]
pub(crate) fn gemini_canvas_debug_headers_snapshot_from_pairs(pairs: &[(String, String)]) -> Value {
    headers_snapshot(sanitize_snapshot_pairs(
        pairs.iter().map(|(n, v)| (n.as_str(), v.as_str())),
    ))
}

fn headers_snapshot(pairs: Vec<(String, String)>) -> Value {
    let mut raw = serde_json::Map::new();
    let mut parsed = serde_json::Map::new();
    for (name, value) in &pairs {
        raw.insert(name.clone(), Value::String(value.clone()));
        if name.eq_ignore_ascii_case("x-goog-ext-525001261-jspb")
            || name.eq_ignore_ascii_case("x-goog-ext-525005358-jspb")
            || name.eq_ignore_ascii_case("x-goog-ext-73010989-jspb")
            || name.eq_ignore_ascii_case("x-goog-ext-73010990-jspb")
        {
            parsed.insert(
                name.clone(),
                serde_json::from_str::<Value>(value)
                    .unwrap_or_else(|_| Value::String(value.clone())),
            );
        }
    }
    json!({
        "raw": raw,
        "parsed": parsed,
    })
}

pub(crate) fn gemini_canvas_debug_headers_snapshot_from_hash_map(
    headers: &HashMap<String, String>,
) -> Value {
    // Retain the first64 lexical keys without cloning unbounded raw values.
    let mut pairs = BTreeMap::new();
    for (name, value) in headers {
        pairs.insert(name.as_str(), value.as_str());
        if pairs.len() > 64 {
            pairs.pop_last();
        }
    }
    headers_snapshot(sanitize_snapshot_pairs(pairs))
}

pub(crate) fn gemini_canvas_debug_headers_snapshot_from_header_map(headers: &HeaderMap) -> Value {
    let pairs = headers
        .iter()
        .filter_map(|(name, value)| value.to_str().ok().map(|entry| (name.as_str(), entry)));
    headers_snapshot(sanitize_snapshot_pairs(pairs))
}

pub(crate) fn gemini_canvas_debug_query_snapshot(query: &[(String, String)]) -> Value {
    let query = sanitize_snapshot_pairs(query.iter().map(|(n, v)| (n.as_str(), v.as_str())));
    let items = query
        .iter()
        .map(|(name, value)| json!({ "name": name, "value": value }))
        .collect::<Vec<_>>();
    let mut object = serde_json::Map::new();
    for (name, value) in &query {
        object.insert(name.clone(), Value::String(value.clone()));
    }
    json!({
        "items": items,
        "object": object,
    })
}

pub(crate) fn gemini_canvas_debug_form_snapshot(form: &[(String, String)]) -> Value {
    let form = sanitize_snapshot_pairs(form.iter().map(|(n, v)| (n.as_str(), v.as_str())));
    let items = form
        .iter()
        .map(|(name, value)| json!({ "name": name, "value": value }))
        .collect::<Vec<_>>();
    let mut object = serde_json::Map::new();
    let mut parsed = serde_json::Map::new();
    for (name, value) in &form {
        object.insert(name.clone(), Value::String(value.clone()));
        if name == "f.req" {
            if let Ok(outer) = serde_json::from_str::<Value>(value) {
                parsed.insert("f.req.outer".to_string(), outer.clone());
                if let Some(inner_payload) = outer.get(1).and_then(Value::as_str) {
                    let inner = serde_json::from_str::<Value>(inner_payload)
                        .unwrap_or_else(|_| Value::String(inner_payload.to_string()));
                    parsed.insert("f.req.inner".to_string(), inner);
                }
            }
        }
    }
    json!({
        "items": items,
        "object": object,
        "parsed": parsed,
    })
}

#[cfg(test)]
#[path = "gemini_canvas_debug_snapshot_tests.rs"]
mod tests;
