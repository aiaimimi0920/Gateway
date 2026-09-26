use serde_json::Value;
use std::collections::HashSet;

fn looks_like_stream_generate_id(value: &str, prefix: &str) -> bool {
    let candidate = value.trim();
    candidate.starts_with(prefix)
        && candidate.len() > prefix.len()
        && candidate[prefix.len()..]
            .chars()
            .all(|ch| ch.is_ascii_hexdigit())
}

pub(super) fn summarize_stream_generate_media_debug_summary(frames: &[Value]) -> String {
    let mut ids = Vec::new();
    let mut ids_seen = HashSet::new();
    let mut urls = Vec::new();
    let mut urls_seen = HashSet::new();
    let mut texts = Vec::new();
    let mut texts_seen = HashSet::new();

    for frame in frames {
        collect_stream_generate_media_debug_values(
            frame,
            &mut ids,
            &mut ids_seen,
            &mut urls,
            &mut urls_seen,
            &mut texts,
            &mut texts_seen,
        );
    }

    format!(
        "detected_ids={}; detected_urls={}; detected_text={}",
        if ids.is_empty() {
            "<none>".to_string()
        } else {
            ids.join(",")
        },
        if urls.is_empty() {
            "<none>".to_string()
        } else {
            urls.join(",")
        },
        if texts.is_empty() {
            "<none>".to_string()
        } else {
            texts.join(" | ")
        }
    )
}

fn collect_stream_generate_media_debug_values(
    value: &Value,
    ids: &mut Vec<String>,
    ids_seen: &mut HashSet<String>,
    urls: &mut Vec<String>,
    urls_seen: &mut HashSet<String>,
    texts: &mut Vec<String>,
    texts_seen: &mut HashSet<String>,
) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_stream_generate_media_debug_values(
                    item, ids, ids_seen, urls, urls_seen, texts, texts_seen,
                );
            }
        }
        Value::Object(map) => {
            for item in map.values() {
                collect_stream_generate_media_debug_values(
                    item, ids, ids_seen, urls, urls_seen, texts, texts_seen,
                );
            }
        }
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return;
            }

            if (looks_like_stream_generate_id(trimmed, "r_")
                || looks_like_stream_generate_id(trimmed, "c_")
                || (trimmed.starts_with("rc_")
                    && trimmed.len() >= 6
                    && trimmed[3..].chars().all(|ch| ch.is_ascii_hexdigit())))
                && ids_seen.insert(trimmed.to_string())
                && ids.len() < 12
            {
                ids.push(trimmed.to_string());
            }

            if (trimmed.starts_with("https://")
                || trimmed.starts_with("http://")
                || trimmed.starts_with("data:image/")
                || trimmed.starts_with("data:audio/")
                || trimmed.starts_with("data:video/")
                || trimmed.starts_with("blob:"))
                && urls_seen.insert(trimmed.to_string())
                && urls.len() < 12
            {
                urls.push(trimmed.to_string());
            }

            if trimmed.len() >= 24
                && trimmed.contains(' ')
                && trimmed.chars().any(|ch| ch.is_ascii_alphabetic())
                && !trimmed.starts_with('[')
                && !trimmed.starts_with('{')
            {
                let compact = if trimmed.chars().count() > 140 {
                    format!(
                        "{}...(truncated)",
                        trimmed.chars().take(140).collect::<String>()
                    )
                } else {
                    trimmed.to_string()
                };
                if texts_seen.insert(compact.clone()) && texts.len() < 8 {
                    texts.push(compact);
                }
            }

            if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
                collect_stream_generate_media_debug_values(
                    &parsed, ids, ids_seen, urls, urls_seen, texts, texts_seen,
                );
            }
        }
        _ => {}
    }
}
