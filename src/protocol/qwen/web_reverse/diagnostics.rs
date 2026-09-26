use serde_json::Value;

pub(super) fn qwen_web_response_shape(body: &str) -> String {
    let mut paths = std::collections::BTreeSet::new();
    let mut json_frames = 0usize;
    for line in body.lines() {
        let normalized = line.trim_end_matches('\r');
        let candidate = normalized
            .strip_prefix("data:")
            .map(|value| value.strip_prefix(' ').unwrap_or(value).trim())
            .unwrap_or(normalized.trim());
        if candidate.is_empty() || candidate == "[DONE]" {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(candidate) {
            json_frames += 1;
            collect_qwen_web_json_paths(&value, "", &mut paths, 0);
        }
    }
    format!(
        "json_frames={json_frames}; key_paths={}",
        paths.into_iter().take(80).collect::<Vec<_>>().join(",")
    )
}

fn collect_qwen_web_json_paths(
    value: &Value,
    prefix: &str,
    paths: &mut std::collections::BTreeSet<String>,
    depth: usize,
) {
    if depth >= 6 || paths.len() >= 80 {
        return;
    }
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                paths.insert(path.clone());
                collect_qwen_web_json_paths(child, &path, paths, depth + 1);
            }
        }
        Value::Array(items) => {
            if let Some(first) = items.first() {
                let path = format!("{prefix}[]");
                paths.insert(path.clone());
                collect_qwen_web_json_paths(first, &path, paths, depth + 1);
            }
        }
        _ => {}
    }
}
