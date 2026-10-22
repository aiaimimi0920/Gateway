//! Explicit non-native CLI body shaping. Caller instructions and generation choices are retained.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub fn billing(text: &str) -> String {
    let units: Vec<_> = text.encode_utf16().collect();
    let sample: Vec<_> = [4, 7, 20]
        .into_iter()
        .map(|i| units.get(i).copied().unwrap_or(b'0' as u16))
        .collect();
    let sample = String::from_utf16_lossy(&sample);
    let hash = format!(
        "{:x}",
        Sha256::digest(format!("59cf53e54c78{sample}2.1.280"))
    );
    format!(
        "x-anthropic-billing-header: cc_version=2.1.280.{}; cc_entrypoint=cli;",
        &hash[..3]
    )
}

fn blocks(value: Value) -> Result<Vec<Value>, &'static str> {
    match value {
        Value::Null => Ok(Vec::new()),
        Value::String(text) => Ok(vec![json!({"type":"text","text":text})]),
        Value::Array(items) => Ok(items),
        _ => Err("CPA CLI content must be text or a block array"),
    }
}

pub fn shape(body: &mut Value, date: &str) -> Result<(), &'static str> {
    let automatic_cache = !super::cache::explicit(body);
    let original_system = body
        .as_object()
        .ok_or("CPA CLI body must be an object")?
        .get("system")
        .cloned();
    let mut system = blocks(original_system.unwrap_or(Value::Null))?;
    if system.iter().any(|block| block["type"] != "text") {
        return Err("CPA CLI system supports text blocks only");
    }
    system.retain(|block| {
        block["text"].as_str().is_some_and(|text| {
            !text.trim().is_empty()
                && !text.starts_with("x-anthropic-billing-header:")
                && text != "You are Claude Code, Anthropic's official CLI for Claude."
        })
    });
    for block in &mut system {
        if automatic_cache {
            block["cache_control"] = json!({"type":"ephemeral"});
        }
    }
    let model = body["model"].as_str().unwrap_or("").to_owned();
    let messages = body["messages"]
        .as_array_mut()
        .ok_or("CPA CLI messages must be an array")?;
    let index = messages
        .iter()
        .position(|m| m["role"] == "user")
        .ok_or("CPA CLI requires a user message")?;
    let first_text = super::identity::text_parts(&messages[index]["content"])
        .into_iter()
        .next()
        .unwrap_or_default();
    let mut content = blocks(messages[index]["content"].take())?;
    let reminder = format!("<system-reminder>\nAs you answer the user's questions, you can use the following context:\n# currentDate\nToday's date is {date}.\n\n      IMPORTANT: this context may or may not be relevant to your tasks. You should not respond to this context unless it is highly relevant to your task.\n</system-reminder>\n\n");
    content.retain(|block| {
        !block["text"].as_str().is_some_and(|s| {
            s.starts_with("<system-reminder>\nAs you answer") && s.contains("# currentDate")
        })
    });
    if let Some(text) = content
        .iter_mut()
        .find(|b| automatic_cache && b["type"] == "text")
    {
        text.as_object_mut()
            .ok_or("invalid text block")?
            .entry("cache_control")
            .or_insert(json!({"type":"ephemeral"}));
    }
    let insertion = content
        .iter()
        .take_while(|b| b["type"] == "tool_result")
        .count();
    content.insert(insertion, json!({"type":"text","text":reminder}));
    if !system.is_empty() && super::betas::legacy(&model) {
        let text = system
            .iter()
            .filter_map(|b| b["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        content.insert(
            insertion + 1,
            json!({"type":"text","text":format!("<system-reminder>\n{text}\n</system-reminder>")}),
        );
    }
    messages[index]["content"] = Value::Array(content);
    let mut top_level = Vec::new();
    if !system.is_empty() && !super::betas::legacy(&model) {
        let after = index
            + messages[index..]
                .iter()
                .take_while(|m| {
                    m["role"] == "user"
                        || (m["role"] == "system"
                            && m.get("output_config").is_some()
                            && m["content"].as_array().is_some_and(Vec::is_empty))
                })
                .count();
        if after == messages.len() || after > index + 1 {
            top_level = system;
        } else {
            let already_present = system.iter().enumerate().all(|(offset, block)| {
                messages.get(after + offset).is_some_and(|m| {
                    m["role"] == "system"
                        && super::identity::text_parts(&m["content"]).join("\n")
                            == block["text"].as_str().unwrap_or("")
                })
            });
            if !already_present {
                for (offset, block) in system.into_iter().enumerate() {
                    messages.insert(after + offset, json!({"role":"system","content":[block]}));
                }
            }
        }
    }
    body["system"] = json!([
        {"type":"text","text":billing(&first_text)},
        {"type":"text","text":"You are Claude Code, Anthropic's official CLI for Claude.","cache_control":{"type":"ephemeral"}}
    ]);
    body["system"].as_array_mut().unwrap().extend(top_level);
    if !automatic_cache {
        body["system"][1]
            .as_object_mut()
            .unwrap()
            .remove("cache_control");
    }
    super::cache::apply(body)?;
    Ok(())
}
