//! CPA helper classification is payload semantics, never an authentication decision.
use serde_json::Value;

fn texts(value: &Value) -> Vec<String> {
    super::identity::text_parts(value)
}
fn title(text: &str) -> bool {
    [
        "naming a coding session",
        "Return a short title",
        "Write the title in the predominant language",
    ]
    .iter()
    .any(|pattern| text.contains(pattern))
}

pub fn helper(body: &Value) -> bool {
    let messages = body["messages"].as_array();
    if body["max_tokens"].as_i64() == Some(1) && body["tools"].as_array().is_none_or(Vec::is_empty)
    {
        if messages.is_none_or(Vec::is_empty) {
            return true;
        }
        let messages = messages.unwrap();
        if messages.len() == 1 && messages[0]["role"] == "user" {
            let content = &messages[0]["content"];
            if content
                .as_str()
                .is_some_and(|v| matches!(v.trim(), "quota" | "test" | "." | "probe"))
            {
                return true;
            }
            if let Some(blocks) = content.as_array() {
                let blocks: Vec<_> = blocks
                    .iter()
                    .filter(|b| {
                        !b["text"]
                            .as_str()
                            .unwrap_or("")
                            .contains("<system-reminder>")
                    })
                    .collect();
                if blocks.len() == 1 {
                    let text = blocks[0]["text"].as_str().unwrap_or("").trim();
                    if matches!(text, "quota" | "test" | "." | "probe")
                        || (text == "Hi" && blocks[0].get("cache_control").is_some())
                    {
                        return true;
                    }
                }
            }
        }
    }
    let schema_title = body
        .pointer("/output_config/format/schema/properties")
        .and_then(Value::as_object)
        .is_some_and(|p| p.len() == 1 && p.contains_key("title"));
    let matches = |text: &str| title(text) || (schema_title && text.contains("<session>"));
    if texts(&body["system"]).iter().any(|text| matches(text)) {
        return true;
    }
    messages.into_iter().flatten().any(|m| {
        (schema_title || m["role"] == "system")
            && texts(&m["content"]).iter().any(|text| matches(text))
    })
}

pub fn subagent(body: &Value) -> bool {
    let parent = body
        .pointer("/metadata/user_id")
        .and_then(Value::as_str)
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .is_some_and(|v| v.get("parent_session_id").is_some());
    parent
        || texts(&body["system"])
            .first()
            .is_some_and(|s| s.contains("cc_is_subagent=true"))
}
