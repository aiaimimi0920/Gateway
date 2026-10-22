//! Software identity measured from WorkBuddy AI 5.8.4 with its bundled CLI 2.161.0.
//! Authentication, privacy flags, agent intent, sessions and traces are not software identity.
use crate::upstream::common::insert_header_map_value as set;
use rquest::header::HeaderMap;

use crate::protocol::workbuddy_wire::{HEADER_ORDER, USER_AGENT};

pub(crate) fn is_identity(headers: &HeaderMap) -> bool {
    headers.get("user-agent").is_some_and(|v| v == USER_AGENT)
        && headers.get("x-ide-type").is_some_and(|v| v == "WorkBuddy")
        && headers.get("x-codebuddy-request").is_some_and(|v| v == "1")
}

pub(super) fn apply(headers: &mut HeaderMap) {
    for (name, value) in [
        ("user-agent", USER_AGENT),
        ("x-requested-with", "XMLHttpRequest"),
        ("x-ide-type", "WorkBuddy"),
        ("x-ide-name", "WorkBuddy"),
        ("x-ide-version", "5.8.4"),
        ("x-product", "SaaS"),
        ("x-codebuddy-request", "1"),
        ("x-stainless-lang", "js"),
        ("x-stainless-runtime", "node"),
        ("x-stainless-runtime-version", "v22.21.1"),
        ("x-stainless-package-version", "6.25.0"),
        ("x-stainless-retry-count", "0"),
        ("x-stainless-os", crate::protocol::claude_wire::platform()),
        (
            "x-stainless-arch",
            crate::protocol::claude_wire::architecture(),
        ),
    ] {
        set(headers, name, value);
    }
    // Do not override an explicit protocol/operator Accept preference.
    if !headers.contains_key("accept") {
        set(headers, "accept", "application/json");
    }
    // Do not advertise Node's legacy compress decoder: Gateway does not implement it.
    if !headers.contains_key("accept-encoding") {
        set(headers, "accept-encoding", "gzip, deflate, br");
    }
    // This is our connection-pool policy, not a forwarded hop-by-hop instruction.
    if !headers.contains_key("connection") {
        set(headers, "connection", "keep-alive");
    }
}

pub(crate) fn order(headers: HeaderMap) -> (HeaderMap, rquest::header::OrigHeaderMap) {
    let mut remaining = headers;
    let mut ordered = HeaderMap::new();
    let mut original = rquest::header::OrigHeaderMap::new();
    for name in HEADER_ORDER {
        original.insert(*name);
        if let Some(value) = remaining.remove(*name) {
            ordered.insert(
                rquest::header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                value,
            );
        }
    }
    ordered.extend(remaining);
    (ordered, original)
}

pub(super) fn preserve_message_context(
    req: &super::CanonicalRelayRequest,
    body: &mut serde_json::Value,
) {
    if req.endpoint_kind != super::EndpointKind::ChatCompletions {
        return;
    }
    let Some(raw) = req
        .raw_body
        .get("messages")
        .and_then(serde_json::Value::as_array)
    else {
        return;
    };
    let Some(planned) = body
        .get_mut("messages")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return;
    };
    if raw.len() != planned.len() {
        return;
    }
    for (source, target) in raw.iter().zip(planned) {
        let Some(mut clean) = source.as_object().cloned() else {
            continue;
        };
        let context: Vec<_> = ["agent", "conversationRequestId"]
            .into_iter()
            .filter_map(|name| clean.remove(name).map(|value| (name, value)))
            .collect();
        // Do not resurrect policy-modified messages or override explicit planned extensions.
        if target.as_object() != Some(&clean) {
            continue;
        }
        for (name, value) in context {
            if value
                .as_str()
                .is_some_and(crate::protocol::workbuddy_wire::valid)
            {
                target[name] = value;
            }
        }
    }
}
