use crate::error::sanitize_provider_error_message;
use serde_json::Value;

const REDACTED: &str = "<redacted>";
const OMITTED: &str = "<omitted: diagnostic budget>";
const MAX_FIELDS: usize = 64;
const MAX_DEPTH: usize = 12;
const MAX_INPUT_BYTES: usize = 64 * 1024;

pub(super) fn sanitize_debug_snapshot(value: &Value) -> Value {
    SnapshotRedactor { remaining: 512 }.value(value, 0)
}

pub(super) fn sanitize_debug_text(text: &str) -> String {
    SnapshotRedactor { remaining: 512 }.text(text, 0)
}

#[cfg(test)]
#[path = "gemini_canvas_debug_redaction_tests.rs"]
mod tests;

pub(super) fn sanitize_snapshot_pairs<'a>(
    pairs: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Vec<(String, String)> {
    let mut redactor = SnapshotRedactor { remaining: 512 };
    pairs
        .into_iter()
        .take(MAX_FIELDS)
        .map(|(name, value)| {
            let safe_value = if sensitive_name(name) {
                REDACTED.to_string()
            } else {
                redactor.text(value, 0)
            };
            (safe_name(name), safe_value)
        })
        .collect()
}

fn sensitive_name(name: &str) -> bool {
    if name.len() > 512 {
        return true;
    }
    let normalized: String = name
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    matches!(
        normalized.as_str(),
        "at" | "key" | "sid" | "sapisid" | "hsid" | "ssid"
    ) || [
        "authorization",
        "cookie",
        "apikey",
        "token",
        "secret",
        "password",
        "credential",
        "session",
    ]
    .iter()
    .any(|part| normalized.contains(part))
}

fn safe_name(name: &str) -> String {
    if name.len() > 512 {
        return OMITTED.to_string();
    }
    sanitize_provider_error_message(name)
        .chars()
        .take(128)
        .collect()
}

struct SnapshotRedactor {
    remaining: usize,
}

fn exceeds_json_depth(text: &str, limit: usize) -> bool {
    let (mut depth, mut quoted, mut escaped) = (0usize, false, false);
    for byte in text.bytes() {
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
        } else {
            match byte {
                b'"' => quoted = true,
                b'[' | b'{' => {
                    depth += 1;
                    if depth >= limit {
                        return true;
                    }
                }
                b']' | b'}' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
    }
    false
}

impl SnapshotRedactor {
    fn enter(&mut self, depth: usize) -> bool {
        if depth >= MAX_DEPTH || self.remaining == 0 {
            return false;
        }
        self.remaining -= 1;
        true
    }

    fn text(&mut self, text: &str, depth: usize) -> String {
        // Never cut encoded credentials first: oversized inputs are omitted entirely.
        if text.len() > MAX_INPUT_BYTES || !self.enter(depth) {
            return OMITTED.to_string();
        }
        if exceeds_json_depth(text, MAX_DEPTH - depth) {
            return OMITTED.to_string();
        }
        if let Ok(mut url) = url::Url::parse(text) {
            if matches!(url.scheme(), "http" | "https") {
                if !url.username().is_empty() {
                    let _ = url.set_username(REDACTED);
                }
                let _ = url.set_password(None);
                url.set_fragment(None);
                if url.query().is_some() {
                    let pairs: Vec<_> = url
                        .query_pairs()
                        .take(MAX_FIELDS)
                        .map(|(name, value)| {
                            let value = if sensitive_name(&name) {
                                REDACTED.to_string()
                            } else {
                                self.text(&value, depth + 1)
                            };
                            (safe_name(&name), value)
                        })
                        .collect();
                    url.query_pairs_mut().clear().extend_pairs(pairs);
                }
                return url.to_string();
            }
        }
        if let Ok(value) = serde_json::from_str::<Value>(text) {
            return self.value(&value, depth + 1).to_string();
        }
        // Malformed structured payloads cannot safely fall back to a partial regex policy.
        if text.trim_start().starts_with(['{', '[', '"']) {
            return OMITTED.to_string();
        }
        sanitize_provider_error_message(text)
    }

    fn value(&mut self, value: &Value, depth: usize) -> Value {
        if !self.enter(depth) {
            return Value::String(OMITTED.to_string());
        }
        match value {
            Value::String(text) => Value::String(self.text(text, depth + 1)),
            Value::Array(items) => Value::Array(
                items
                    .iter()
                    .take(MAX_FIELDS)
                    .map(|v| self.value(v, depth + 1))
                    .collect(),
            ),
            Value::Object(fields) => Value::Object(
                fields
                    .iter()
                    .take(MAX_FIELDS)
                    .map(|(name, value)| {
                        let paired_secret = name == "value"
                            && fields
                                .get("name")
                                .and_then(Value::as_str)
                                .is_some_and(sensitive_name);
                        let value = if sensitive_name(name) || paired_secret {
                            Value::String(REDACTED.to_string())
                        } else {
                            self.value(value, depth + 1)
                        };
                        (safe_name(name), value)
                    })
                    .collect(),
            ),
            other => other.clone(),
        }
    }
}
