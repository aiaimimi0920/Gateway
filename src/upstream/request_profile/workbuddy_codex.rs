//! Translate real Codex context into WorkBuddy correlation fields, never credentials/privacy.
use super::ProviderAccountPayload;
use crate::upstream::common::insert_header_map_value as set;
use rquest::header::HeaderMap;
use serde_json::Value;
use uuid::Uuid;

fn configured(payload: &ProviderAccountPayload, name: &str) -> bool {
    payload
        .auth_header_name
        .as_deref()
        .is_some_and(|key| key.eq_ignore_ascii_case(name))
        || payload
            .headers
            .keys()
            .any(|key| key.eq_ignore_ascii_case(name))
}

fn put(headers: &mut HeaderMap, payload: &ProviderAccountPayload, name: &str, value: &str) {
    if !configured(payload, name) && !headers.contains_key(name) {
        set(headers, name, value);
    }
}

fn value<'a>(
    headers: &'a HeaderMap,
    payload: &ProviderAccountPayload,
    name: &str,
) -> Option<&'a str> {
    if payload
        .auth_header_name
        .as_deref()
        .is_some_and(|key| key.eq_ignore_ascii_case(name))
    {
        return None;
    }
    headers.get(name)?.to_str().ok()
}

fn id(value: Option<&str>) -> Option<Uuid> {
    value
        .and_then(|v| Uuid::parse_str(v).ok())
        .filter(|v| !v.is_nil())
}

pub(super) fn sanitize_body_metadata(body: &mut Value) {
    // Keep protocol/business metadata, but not Codex installation/window identity.
    if let Some(metadata) = body
        .get_mut("client_metadata")
        .and_then(Value::as_object_mut)
    {
        metadata.retain(|key, _| !key.to_ascii_lowercase().starts_with("x-codex-"));
    }
}

pub(super) fn apply(headers: &mut HeaderMap, payload: &ProviderAccountPayload, native: bool) {
    if native {
        translate(headers, payload);
    }
    // These are caller identity/control metadata, including installation/sandbox details.
    // Explicit operator settings are separate authority; no wildcard forwarding is added.
    let remove: Vec<_> = headers
        .keys()
        .filter(|name| {
            let key = name.as_str();
            (key.starts_with("x-codex-")
                || key.starts_with("x-openai-")
                || matches!(key, "session-id" | "thread-id" | "x-client-request-id"))
                && !configured(payload, key)
        })
        .cloned()
        .collect();
    for name in remove {
        headers.remove(name);
    }
}

fn translate(headers: &mut HeaderMap, payload: &ProviderAccountPayload) {
    let metadata: Option<Value> = value(headers, payload, "x-codex-turn-metadata")
        .filter(|v| v.len() <= 16_384)
        .and_then(|v| serde_json::from_str(v).ok());
    let field = |name| {
        metadata
            .as_ref()
            .and_then(|m| m.get(name))
            .and_then(Value::as_str)
    };
    let candidates: Vec<_> = [
        value(headers, payload, "thread-id"),
        value(headers, payload, "session-id"),
        field("thread_id"),
        field("session_id"),
    ]
    .into_iter()
    .flatten()
    .collect();
    // A conflict must not bind one user's conversation to another label.
    let session = candidates
        .first()
        .and_then(|first| id(Some(first)))
        .filter(|first| candidates.iter().all(|v| id(Some(v)) == Some(*first)));
    if let Some(session) = session {
        put(headers, payload, "x-conversation-id", &session.to_string());
        let turn = id(field("turn_id"));
        if let Some(turn) = turn {
            put(
                headers,
                payload,
                "x-conversation-request-id",
                &turn.simple().to_string(),
            );
        }
        if let Some(root) = id(field("root_turn_id")) {
            put(
                headers,
                payload,
                "x-root-request-id",
                &root.simple().to_string(),
            );
        }
        let parent =
            id(value(headers, payload, "x-codex-parent-thread-id")).filter(|p| *p != session);
        if let Some(parent) = parent {
            put(
                headers,
                payload,
                "x-parent-conversation-id",
                &parent.to_string(),
            );
        }
        if field("request_kind") == Some("turn") {
            // WorkBuddy's normal coding conversation defaults, not compact/privacy claims.
            put(headers, payload, "x-agent-intent", "craft");
            put(headers, payload, "x-agent-purpose", "conversation");
            if parent.is_some() || field("agent_name") == Some("/root") {
                put(
                    headers,
                    payload,
                    "x-agent-type",
                    if parent.is_some() { "subagent" } else { "main" },
                );
            }
        }
    }
    // A message ID identifies this actual relay request, not an invented conversation.
    if !configured(payload, "x-request-id") {
        if let Some(request) = id(value(headers, payload, "x-request-id")) {
            set(headers, "x-request-id", &request.simple().to_string());
        }
    }
    if let Some(request) = value(headers, payload, "x-request-id").map(str::to_owned) {
        put(headers, payload, "x-conversation-message-id", &request);
    }
    if !configured(payload, "accept") {
        set(headers, "accept", "application/json");
    }
    trace(headers, payload);
}

fn trace(headers: &mut HeaderMap, payload: &ProviderAccountPayload) {
    if payload
        .auth_header_name
        .as_deref()
        .is_some_and(|key| key.eq_ignore_ascii_case("traceparent"))
    {
        return;
    }
    if !headers.contains_key("traceparent") {
        let trace = Uuid::new_v4().simple().to_string();
        let span = Uuid::new_v4().simple().to_string();
        set(
            headers,
            "traceparent",
            &format!("00-{trace}-{}-01", &span[..16]),
        );
    }
    let Some(parent) = value(headers, payload, "traceparent").map(str::to_owned) else {
        return;
    };
    let parts: Vec<_> = parent.split('-').collect();
    if parts.len() != 4
        || parts[0] != "00"
        || parts[1].len() != 32
        || parts[2].len() != 16
        || parts[3].len() != 2
        || !parts[1..]
            .iter()
            .all(|v| v.bytes().all(|b| b.is_ascii_hexdigit()))
        || parts[1].bytes().all(|b| b == b'0')
        || parts[2].bytes().all(|b| b == b'0')
    {
        return;
    }
    let sampled = if u8::from_str_radix(parts[3], 16).unwrap_or(0) & 1 == 1 {
        "1"
    } else {
        "0"
    };
    for (name, val) in [
        ("x-b3-traceid", parts[1]),
        ("x-trace-id", parts[1]),
        ("x-b3-spanid", parts[2]),
        ("x-b3-sampled", sampled),
    ] {
        put(headers, payload, name, val);
    }
    put(
        headers,
        payload,
        "b3",
        &format!("{}-{}-{sampled}", parts[1], parts[2]),
    );
}
