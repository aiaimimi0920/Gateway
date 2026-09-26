//! LumaLabs signed-output URL recovery from nested event payloads.

use regex::Regex;
use serde_json::Value;

pub fn extract_signed_url_from_event_payload(output_id: &str, payload: &str) -> Option<String> {
    if let Ok(value) = serde_json::from_str::<Value>(payload) {
        if let Some(url) = find_signed_url_in_value(output_id, &value) {
            return Some(url);
        }
    }

    let normalized = normalize_event_string(payload);
    extract_signed_url_from_text(output_id, &normalized)
}

fn find_signed_url_in_value(output_id: &str, value: &Value) -> Option<String> {
    match value {
        Value::String(text) => {
            let normalized = normalize_event_string(text);
            if let Some(url) = extract_signed_url_from_text(output_id, &normalized) {
                return Some(url);
            }
            if normalized.starts_with('{') || normalized.starts_with('[') {
                if let Ok(inner) = serde_json::from_str::<Value>(&normalized) {
                    return find_signed_url_in_value(output_id, &inner);
                }
            }
            None
        }
        Value::Array(values) => values
            .iter()
            .find_map(|entry| find_signed_url_in_value(output_id, entry)),
        Value::Object(map) => {
            for key in &[
                "url",
                "src",
                "imageUrl",
                "image_url",
                "downloadUrl",
                "download_url",
                "signedUrl",
                "signed_url",
                "cdnUrl",
                "cdn_url",
            ] {
                if let Some(url) = map
                    .get(*key)
                    .and_then(|entry| find_signed_url_in_value(output_id, entry))
                {
                    return Some(url);
                }
            }

            map.values()
                .find_map(|entry| find_signed_url_in_value(output_id, entry))
        }
        _ => None,
    }
}

fn extract_signed_url_from_text(output_id: &str, text: &str) -> Option<String> {
    static URL_REGEX: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let regex = URL_REGEX.get_or_init(|| {
        Regex::new(r#"https://[^\s"'<>\\]+"#).expect("lumalabs url regex must compile")
    });

    regex
        .find_iter(text)
        .map(|m| cleanup_url_candidate(m.as_str()))
        .find(|candidate| is_signed_output_url(output_id, candidate))
}

fn is_signed_output_url(output_id: &str, candidate: &str) -> bool {
    candidate.contains(output_id)
        && candidate.starts_with("https://")
        && (candidate.contains("cdn.")
            || candidate.contains("Key-Pair-Id=")
            || candidate.contains("Policy="))
}

fn cleanup_url_candidate(candidate: &str) -> String {
    candidate
        .trim_matches(|ch| matches!(ch, '"' | '\'' | ',' | ';' | ')' | ']' | '}'))
        .replace("\\\\u0026", "&")
        .replace("\\\\u003d", "=")
        .replace("\\\\/", "/")
        .replace("\\u0026", "&")
        .replace("\\u003d", "=")
        .replace("\\/", "/")
}

fn normalize_event_string(value: &str) -> String {
    value
        .replace("\\\\u0026", "&")
        .replace("\\\\u003d", "=")
        .replace("\\\\/", "/")
        .replace("\\u0026", "&")
        .replace("\\u003d", "=")
        .replace("\\/", "/")
}
