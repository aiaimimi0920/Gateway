//! Shared document admission for Web uploads and Canvas inline inputs, not the official API.
//! Caller filenames are metadata only: no local reads, remote fetches or document execution.
use serde_json::{Map, Value};

pub(crate) fn mime_and_extension(mime: &str) -> Option<(&'static str, &'static str)> {
    match mime {
        "application/pdf" => Some(("application/pdf", "pdf")),
        "text/plain" => Some(("text/plain", "txt")),
        "text/markdown" => Some(("text/markdown", "md")),
        "text/csv" => Some(("text/csv", "csv")),
        "text/html" => Some(("text/html", "html")),
        "text/xml" => Some(("text/xml", "xml")),
        "application/xml" => Some(("application/xml", "xml")),
        "application/json" => Some(("application/json", "json")),
        _ => None,
    }
}

// Minimum envelope admission only; never decompress, render or execute an uploaded document.
pub(crate) fn valid_bytes(mime: &str, bytes: &[u8]) -> bool {
    if bytes.is_empty() || mime_and_extension(mime).is_none() {
        return false;
    }
    if mime == "application/pdf" {
        return bytes.starts_with(b"%PDF-")
            && bytes[bytes.len().saturating_sub(1024)..]
                .windows(5)
                .any(|window| window == b"%%EOF");
    }
    std::str::from_utf8(bytes).is_ok_and(|text| !text.contains('\0'))
}

fn allowed(object: &Map<String, Value>, fields: &[&str]) -> bool {
    object.keys().all(|key| fields.contains(&key.as_str()))
}

/// Recognize Chat Completions `file` and Responses `input_file` without resolving file IDs.
/// Base64 decoding and aggregate admission belong to the caller, before any network action.
pub(crate) fn openai_file(value: &Value) -> Result<Option<(&str, &str)>, ()> {
    let Some(kind @ ("file" | "input_file")) = value.get("type").and_then(Value::as_str) else {
        return Ok(None);
    };
    let object = value.as_object().ok_or(())?;
    let file = if kind == "file" {
        if !allowed(object, &["type", "file"]) {
            return Err(());
        }
        let file = object.get("file").and_then(Value::as_object).ok_or(())?;
        if !allowed(file, &["filename", "file_data"]) {
            return Err(());
        }
        file
    } else {
        if !allowed(object, &["type", "filename", "file_data"]) {
            return Err(());
        }
        object
    };
    if let Some(name) = file.get("filename") {
        if !name.as_str().is_some_and(|name| name.len() <= 1024) {
            return Err(());
        }
    }
    let (header, data) = file
        .get("file_data")
        .and_then(Value::as_str)
        .and_then(|data| data.strip_prefix("data:"))
        .and_then(|data| data.split_once(','))
        .ok_or(())?;
    let mime = header.strip_suffix(";base64").ok_or(())?;
    if data.is_empty() || mime_and_extension(&mime.to_ascii_lowercase()).is_none() {
        return Err(());
    }
    Ok(Some((mime, data)))
}

#[cfg(test)]
#[path = "document_input/tests.rs"]
mod tests;
