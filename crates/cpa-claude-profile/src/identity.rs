//! Stable credential identity and conversation projection; never emit the seed itself.
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub fn uuid_v5(name: &str) -> String {
    let namespace = uuid::Uuid::parse_str("6ba7b812-9dad-11d1-80b4-00c04fd430c8").unwrap();
    let mut hash = sha1::Sha1::new();
    hash.update(namespace.as_bytes());
    hash.update(name.as_bytes());
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&hash.finalize()[..16]);
    bytes[6] = (bytes[6] & 15) | 0x50;
    bytes[8] = (bytes[8] & 63) | 0x80;
    uuid::Uuid::from_bytes(bytes).to_string()
}

pub fn valid_label(value: &str) -> Option<&str> {
    if value.chars().any(char::is_control) {
        return None;
    }
    let value = value.trim();
    (!value.is_empty() && value.len() <= 256).then_some(value)
}

pub fn project_session(identity: &str) -> String {
    let value = identity.strip_prefix("claude:").unwrap_or(identity);
    uuid::Uuid::parse_str(value)
        .map(|id| id.to_string())
        .unwrap_or_else(|_| {
            uuid_v5(&format!(
                "cli-proxy-api\0claude\0agent-conversation\0{identity}"
            ))
        })
}

pub fn explicit_session(body: &Value) -> Option<String> {
    for (prefix, paths) in [
        ("geminicache", &["/cachedContent", "/cached_content"][..]),
        (
            "thread",
            &["/thread_id", "/threadId", "/metadata/thread_id"][..],
        ),
        (
            "session",
            &[
                "/session_id",
                "/sessionId",
                "/sessionID",
                "/child_session_id",
                "/childSessionId",
                "/metadata/session_id",
                "/metadata/sessionId",
                "/metadata/sessionID",
                "/metadata/child_session_id",
                "/extra_body/session_id",
                "/extra_body/sessionId",
                "/extra_body/sessionID",
            ][..],
        ),
        (
            "task",
            &["/task_id", "/taskId", "/action_id", "/actionId"][..],
        ),
        ("pck", &["/prompt_cache_key", "/promptCacheKey"][..]),
        (
            "conv",
            &[
                "/conversation/id",
                "/conversation",
                "/conversation_id",
                "/conversationId",
                "/chat_id",
                "/chatId",
                "/metadata/conversation_id",
                "/extra_body/conversation_id",
            ][..],
        ),
    ] {
        for path in paths {
            if let Some(value) = body
                .pointer(path)
                .and_then(Value::as_str)
                .and_then(valid_label)
            {
                return Some(format!("{prefix}:{value}"));
            }
        }
    }
    None
}

pub fn text_parts(value: &Value) -> Vec<String> {
    if let Some(text) = value.as_str() {
        return vec![text.to_owned()];
    }
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter(|v| v["type"] == "text")
        .filter_map(|v| v["text"].as_str().map(str::to_owned))
        .collect()
}

#[derive(Serialize)]
struct Root<'a> {
    version: &'static str,
    format: &'a str,
    caller_scope: &'a str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    instructions: Vec<String>,
    user: Vec<super::canonical::Part>,
}

#[cfg(test)]
pub fn session(body: &Value, hint: Option<&str>, caller_scope: &str) -> String {
    session_for(body, hint, caller_scope, "claude")
}

pub fn session_for(body: &Value, hint: Option<&str>, caller_scope: &str, format: &str) -> String {
    if let Some(identity) = hint
        .filter(|value| {
            !value.trim().is_empty() && value.len() <= 280 && !value.chars().any(char::is_control)
        })
        .map(str::to_owned)
        .or_else(|| explicit_session(body))
    {
        return project_session(&identity);
    }
    let mut instructions = Vec::new();
    let mut add = |value: &Value| {
        if let Some(text) = super::canonical::instruction(value) {
            instructions.push(text);
        }
    };
    if format == "claude" {
        add(&body["system"]);
    }
    let responses = matches!(format, "openai-response" | "codex");
    if responses {
        add(&body["instructions"]);
    }
    let mut user = Vec::new();
    let messages = if responses {
        &body["input"]
    } else {
        &body["messages"]
    };
    if responses && messages.is_string() {
        user = super::canonical::parts(messages);
    }
    for message in messages.as_array().into_iter().flatten() {
        match message["role"].as_str() {
            Some("system" | "developer") => add(&message["content"]),
            Some("user") => {
                user = super::canonical::parts(&message["content"]);
                if !user.is_empty() {
                    break;
                }
            }
            _ => {}
        }
    }
    if user.is_empty() {
        return uuid::Uuid::new_v4().to_string();
    }
    let root = Root {
        version: "cpa-session-root-v1",
        format,
        caller_scope: caller_scope.trim(),
        instructions,
        user,
    };
    let bytes = go_json(&serde_json::to_string(&root).unwrap());
    project_session(&format!(
        "derived:ctx:v1:{:x}",
        Sha256::digest(bytes.as_bytes())
    ))
}

pub(crate) fn go_json(text: &str) -> String {
    text.replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

pub fn user_id(seed: &str, session: &str, previous: Option<&str>) -> String {
    let seed = if seed.trim().is_empty() {
        "anonymous"
    } else {
        seed.trim()
    };
    let device = format!(
        "{:x}",
        Sha256::digest(format!("cpa-claude-code-cli-device|{seed}"))
    );
    let account = uuid_v5(&format!("cpa-claude-code-cli-account|{seed}"));
    let mut identity = json!({"device_id":device,"account_uuid":account,"session_id":session});
    if let Some(Value::Object(old)) = previous.and_then(|v| serde_json::from_str::<Value>(v).ok()) {
        for (key, value) in old {
            if !matches!(key.as_str(), "device_id" | "account_uuid" | "session_id") {
                identity[&key] = value;
            }
        }
    }
    serde_json::to_string(&identity).unwrap()
}
