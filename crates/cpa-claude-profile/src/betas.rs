//! Ordered beta selection for the custom API-key Messages compatibility path.
use serde_json::Value;

pub fn legacy(model: &str) -> bool {
    let model = model.trim().to_ascii_lowercase();
    matches!(
        model.rsplit('/').next().unwrap_or(""),
        "claude-3-5-haiku-20241022"
            | "claude-3-5-haiku-latest"
            | "claude-3-7-sonnet-20250219"
            | "claude-3-7-sonnet-latest"
            | "claude-haiku-4-5"
            | "claude-haiku-4-5-20251001"
            | "claude-opus-4"
            | "claude-opus-4-20250514"
            | "claude-opus-4-1"
            | "claude-opus-4-1-20250805"
            | "claude-opus-4-5"
            | "claude-opus-4-5-20251101"
            | "claude-opus-4-6"
            | "claude-opus-4-7"
            | "claude-sonnet-4"
            | "claude-sonnet-4-20250514"
            | "claude-sonnet-4-5"
            | "claude-sonnet-4-5-20250929"
            | "claude-sonnet-4-6"
    )
}

pub(crate) fn has_hour_cache(value: &Value) -> bool {
    match value {
        Value::Object(map) => {
            map.get("cache_control").is_some_and(|v| v["ttl"] == "1h")
                || map.values().any(has_hour_cache)
        }
        Value::Array(items) => items.iter().any(has_hour_cache),
        _ => false,
    }
}

/// This API intentionally takes no OAuth authority: identity shaping cannot enable refresh.
pub fn headers(body: &Value) -> String {
    select(body, "", false)
}

pub fn select(body: &Value, requested: &str, native: bool) -> String {
    let wants = |name: &str| requested.split(',').any(|v| v.trim() == name);
    let helper = super::request_kind::helper(body);
    let extended = !helper && (!super::request_kind::subagent(body) || has_hour_cache(body));
    // CPA's explicit CLI identity includes this beta even with an API key.
    // It is a wire capability label, not permission to start an OAuth lifecycle.
    let mut items = vec!["claude-code-20250219", "oauth-2025-04-20"];
    if wants("context-1m-2025-08-07") {
        items.push("context-1m-2025-08-07");
    }
    items.push("interleaved-thinking-2025-05-14");
    if body.pointer("/thinking/display").is_none() {
        items.push("redact-thinking-2026-02-12");
    }
    items.extend([
        "thinking-token-count-2026-05-13",
        "context-management-2025-06-27",
        "prompt-caching-scope-2026-01-05",
    ]);
    let model = body["model"].as_str().unwrap_or("").to_ascii_lowercase();
    let canonical = model.rsplit('/').next().unwrap_or("");
    let sonnet5 =
        canonical.starts_with("claude-sonnet-5") && !canonical.starts_with("claude-sonnet-5-5");
    let progress = canonical.starts_with("claude-opus-5-5")
        || canonical.contains("fable-5-1")
        || canonical.starts_with("claude-sonnet-5");
    let per_turn =
        canonical.starts_with("claude-opus-5-5") || canonical.starts_with("claude-fable-5-1");
    let messages: Vec<_> = body["messages"].as_array().into_iter().flatten().collect();
    let blocks: Vec<_> = messages
        .iter()
        .flat_map(|m| m["content"].as_array().into_iter().flatten())
        .collect();
    if !legacy(&model) {
        items.push("mid-conversation-system-2026-04-07");
    }
    if per_turn || wants("per-turn-control-2026-07-01") {
        items.push("per-turn-control-2026-07-01");
    }
    if wants("timing-2026-09-09")
        || ((per_turn || canonical.starts_with("claude-mythos-5-1"))
            && body.pointer("/output_config/timing").is_some())
    {
        items.push("timing-2026-09-09");
    }
    if !legacy(&model) {
        if !sonnet5 {
            items.push("mid-conversation-tool-changes-2026-07-01");
        }
        if wants("inline-tools-2026-09-15")
            || blocks
                .iter()
                .any(|b| b["type"] == "tool_addition" && b.pointer("/tool/definition").is_some())
        {
            items.push("inline-tools-2026-09-15");
        }
    }
    for (beta, enabled) in [
        (
            "advisor-tool-2026-03-01",
            body["tools"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|t| t["type"].as_str().is_some_and(|s| s.starts_with("advisor"))),
        ),
        (
            "advanced-tool-use-2025-11-20",
            body["tools"].as_array().into_iter().flatten().any(|t| {
                t.get("defer_loading").is_some()
                    || t.get("allowed_callers").is_some()
                    || t.get("input_examples").is_some()
            }),
        ),
        (
            "mid-conversation-system-clear-at-2026-08-21",
            !legacy(&model) && (progress || messages.iter().any(|m| m.get("clear_at").is_some())),
        ),
        (
            "dangerous-tool-use-2026-09-03",
            body.get("safeguards").is_some(),
        ),
    ] {
        if enabled || wants(beta) {
            items.push(beta);
        }
    }
    let effort = !helper
        && !model.contains("haiku")
        && body.pointer("/thinking/type").and_then(Value::as_str) != Some("disabled");
    if effort {
        items.push("effort-2025-11-24");
    }
    for (beta, enabled) in [
        (
            "server-side-fallback-2026-06-01",
            !helper && body.get("fallbacks").is_some(),
        ),
        (
            "fallback-credit-2026-06-01",
            body.get("fallback_credit_token").is_some() || body.get("fallbacks").is_some(),
        ),
        ("structured-outputs-2025-12-15", false),
        (
            "thinking-binding-controls-2026-08-01",
            body.pointer("/thinking/block_binding").is_some()
                || (progress
                    && body.pointer("/thinking/type").and_then(Value::as_str) == Some("adaptive")),
        ),
        (
            "thinking-display-updates-2026-08-18",
            !helper && body.pointer("/thinking/display").and_then(Value::as_str) == Some("updates"),
        ),
        ("thinking-resumption-2026-07-17", false),
        (
            "fast-mode-2026-02-01",
            body["speed"]
                .as_str()
                .is_some_and(|s| s.trim().eq_ignore_ascii_case("fast")),
        ),
        ("afk-mode-2026-01-31", false),
    ] {
        if enabled || wants(beta) {
            items.push(beta);
        }
    }
    if extended {
        items.push("extended-cache-ttl-2025-04-11");
    }
    if wants("prompt-caching-evict-2026-05-12")
        || body.to_string().contains("\"evict_on_complete\"")
    {
        items.push("prompt-caching-evict-2026-05-12");
    }
    if body["diagnostics"].is_object() {
        items.push("cache-diagnosis-2026-04-07");
    }
    if native && !requested.trim().is_empty() {
        items = requested
            .split(',')
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .collect();
    } else {
        for beta in requested
            .split(',')
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            if !items.contains(&beta) {
                items.push(beta);
            }
        }
    }
    items.retain(|beta| {
        (*beta != "effort-2025-11-24" || effort)
            && (!helper
                || !matches!(
                    *beta,
                    "server-side-fallback-2026-06-01"
                        | "thinking-display-updates-2026-08-18"
                        | "extended-cache-ttl-2025-04-11"
                ))
    });
    if extended && !items.contains(&"extended-cache-ttl-2025-04-11") {
        items.push("extended-cache-ttl-2025-04-11");
    }
    items.join(",")
}
