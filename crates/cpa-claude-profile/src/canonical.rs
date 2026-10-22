//! Conversation-root content normalization, independent of generated CLI identity blocks.
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
pub struct Part {
    kind: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    mime: String,
    value: String,
}

impl Part {
    fn new(kind: &str, mime: &str, value: &str) -> Self {
        Self {
            kind: kind.into(),
            mime: mime.into(),
            value: value.into(),
        }
    }
}

fn field<'a>(value: &'a Value, names: &[&str]) -> &'a str {
    names
        .iter()
        .filter_map(|key| value.get(*key)?.as_str())
        .map(str::trim)
        .find(|text| !text.is_empty())
        .unwrap_or("")
}

fn normalized(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<_> = map
                .keys()
                .filter(|key| !key.trim().eq_ignore_ascii_case("cache_control"))
                .collect();
            keys.sort();
            Value::Object(
                keys.into_iter()
                    .map(|key| (key.clone(), normalized(&map[key])))
                    .collect(),
            )
        }
        Value::Array(items) => Value::Array(items.iter().map(normalized).collect()),
        other => other.clone(),
    }
}

fn media(parts: &mut Vec<Part>, kind: &str, value: &Value, fallback: &str) {
    let kind = if kind.trim().is_empty() {
        "media"
    } else {
        kind.trim()
    };
    match value {
        Value::String(text) if !text.is_empty() => parts.push(Part::new(kind, fallback, text)),
        Value::Object(_) => {
            let mime = field(value, &["mimeType", "mime_type", "media_type"]);
            let text = field(value, &["url", "uri", "fileUri", "file_uri", "data"]);
            if !text.is_empty() {
                parts.push(Part::new(
                    kind,
                    if mime.is_empty() { fallback } else { mime },
                    text,
                ));
            }
        }
        Value::String(_) => {}
        _ => append(parts, value),
    }
}

fn append(parts: &mut Vec<Part>, value: &Value) {
    match value {
        Value::Null => {}
        Value::String(text) => {
            if !text.is_empty() {
                parts.push(Part::new("text", "", text));
            }
        }
        Value::Array(items) => {
            for item in items {
                append(parts, item);
            }
        }
        Value::Object(map) => {
            if let Some(text) = map.get("text").filter(|v| v.is_string()) {
                append(parts, text);
                return;
            }
            for key in ["content", "parts"] {
                if let Some(child) = map.get(key) {
                    append(parts, child);
                    return;
                }
            }
            for (keys, kind) in [
                (&["image_url"][..], "image"),
                (&["inlineData", "inline_data"][..], "inline_data"),
                (&["fileData", "file_data"][..], "file"),
            ] {
                if let Some(child) = keys.iter().find_map(|key| map.get(*key)) {
                    media(parts, kind, child, "");
                    return;
                }
            }
            if let Some(source) = map.get("source") {
                media(
                    parts,
                    field(value, &["type"]),
                    source,
                    field(value, &["media_type"]),
                );
                return;
            }
            parts.push(Part::new(
                "json",
                "",
                &super::identity::go_json(&normalized(value).to_string()),
            ));
        }
        _ => parts.push(Part::new("json", "", &value.to_string())),
    }
}

pub fn parts(value: &Value) -> Vec<Part> {
    let mut result = Vec::new();
    append(&mut result, value);
    result
}

pub fn instruction(value: &Value) -> Option<String> {
    let text = parts(value)
        .into_iter()
        .filter(|part| part.kind == "text")
        .map(|part| part.value)
        .collect::<Vec<_>>()
        .join("\n");
    (!text.is_empty()).then(|| text.chars().take(50).collect())
}
