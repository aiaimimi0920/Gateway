//! Bounded cache-marker normalization; explicit TTLs are retained.
use serde_json::{json, Value};

pub fn apply(body: &mut Value) -> Result<(), &'static str> {
    let automatic = !explicit(body);
    if automatic {
        inject_message(body)?;
    }
    enforce_limit(body);
    if !automatic {
        body.as_object_mut().unwrap().remove("prompt_cache_options");
        return Ok(());
    }
    let mut seen_short = false;
    walk(body, &mut |block| {
        if !valid(block) {
            return;
        }
        let cache = block["cache_control"].as_object_mut().unwrap();
        if automatic {
            cache.entry("ttl").or_insert(json!("1h"));
        }
        if cache.get("ttl").and_then(Value::as_str) != Some("1h") {
            seen_short = true;
        } else if seen_short {
            // Anthropic rejects a longer cache TTL after a shorter breakpoint.
            cache.remove("ttl");
        }
    });
    body.as_object_mut().unwrap().remove("prompt_cache_options");
    Ok(())
}

pub fn explicit(body: &Value) -> bool {
    body.pointer("/prompt_cache_options/mode")
        .and_then(Value::as_str)
        .is_some_and(|mode| mode.trim().eq_ignore_ascii_case("explicit"))
}

pub fn preserve(body: &mut Value) {
    // Native cache placement is caller-owned; never synthesize a marker or upgrade its TTL.
    enforce_limit(body);
    body.as_object_mut().unwrap().remove("prompt_cache_options");
}

pub fn strip_ttl(body: &mut Value) {
    walk(body, &mut |block| {
        if let Some(cache) = block
            .get_mut("cache_control")
            .and_then(Value::as_object_mut)
        {
            cache.remove("ttl");
        }
    });
}

fn valid(block: &Value) -> bool {
    block
        .get("cache_control")
        .is_some_and(|cache| cache.is_object() && cache["type"] == "ephemeral")
}

fn inject_message(body: &mut Value) -> Result<(), &'static str> {
    if let Some(messages) = body["messages"].as_array_mut() {
        for message in messages.iter_mut().rev() {
            if let Some(text) = message["content"].as_str() {
                message["content"] = json!([{"type":"text","text":text}]);
            }
            if let Some(blocks) = message["content"].as_array_mut() {
                if blocks.iter().any(valid) {
                    break;
                }
                if let Some(last) = blocks
                    .iter_mut()
                    .rev()
                    .find(|b| !matches!(b["type"].as_str(), Some("thinking" | "redacted_thinking")))
                {
                    last.as_object_mut()
                        .ok_or("CPA CLI content blocks must be objects")?
                        .insert("cache_control".into(), json!({"type":"ephemeral"}));
                    break;
                }
            }
        }
    }
    Ok(())
}

fn strip(block: &mut Value, excess: &mut usize) {
    if *excess > 0 && valid(block) {
        block.as_object_mut().unwrap().remove("cache_control");
        *excess -= 1;
    }
}

fn enforce_limit(body: &mut Value) {
    let mut count: usize = 0;
    walk(body, &mut |block| {
        if valid(block) {
            count += 1;
        }
    });
    let maximum = if body.get("thread").is_some_and(|v| !v.is_null()) {
        3
    } else {
        4
    };
    let mut excess = count.saturating_sub(maximum);
    for name in ["system", "tools"] {
        if let Some(blocks) = body.get_mut(name).and_then(Value::as_array_mut) {
            let last = blocks.iter().rposition(valid);
            for (index, block) in blocks.iter_mut().enumerate() {
                if Some(index) != last {
                    strip(block, &mut excess);
                }
            }
        }
    }
    if let Some(messages) = body.get_mut("messages").and_then(Value::as_array_mut) {
        for message in messages {
            if let Some(blocks) = message.get_mut("content").and_then(Value::as_array_mut) {
                for block in blocks {
                    strip(block, &mut excess);
                }
            }
        }
    }
    for name in ["system", "tools"] {
        if let Some(blocks) = body.get_mut(name).and_then(Value::as_array_mut) {
            for block in blocks {
                strip(block, &mut excess);
            }
        }
    }
}

fn walk(body: &mut Value, visit: &mut impl FnMut(&mut Value)) {
    for name in ["tools", "system"] {
        if let Some(blocks) = body.get_mut(name).and_then(Value::as_array_mut) {
            for block in blocks {
                visit(block);
            }
        }
    }
    if let Some(messages) = body["messages"].as_array_mut() {
        for message in messages {
            if let Some(blocks) = message["content"].as_array_mut() {
                for block in blocks {
                    visit(block);
                }
            }
        }
    }
}
