//! Capture bounded, untrusted correlation labels separately from the forwarded header map.
use super::*;

pub(super) fn capture(ctx: &mut PipelineContext, headers: &HeaderMap) {
    // Validate aliases together before filtering, including entrances without
    // HTTP Responses normalization. A discarded bad alias must not look absent.
    let ambiguous_session = ["session-id", "session_id"].iter().any(|name| {
        headers.get_all(*name).iter().count() > 1
            || headers.get(*name).is_some_and(|value| !valid(value))
    }) || headers
        .get("session-id")
        .zip(headers.get("session_id"))
        .is_some_and(|(a, b)| a != b);
    ctx.client_session_hints.remove("session-id");
    ctx.client_session_hints.remove("session_id");
    for name in [
        "session-id",
        "session_id",
        "x-http-session-id",
        "x-session-id",
        "x-session-affinity",
        "x-client-request-id",
    ] {
        if headers.get_all(name).iter().count() != 1
            || (ambiguous_session && matches!(name, "session-id" | "session_id"))
        {
            continue;
        }
        if let Some(value) = headers.get(name).filter(|value| valid(value)) {
            if let Ok(value) = value.to_str() {
                ctx.client_session_hints
                    .insert(name.into(), value.trim().into());
            }
        }
    }
}

fn valid(value: &axum::http::HeaderValue) -> bool {
    value.to_str().is_ok_and(|value| {
        !value.chars().any(char::is_control)
            && !value.trim().is_empty()
            && value.trim().len() <= 256
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cpa_session_hint_is_captured_without_forwarding_or_authentication() {
        let req =
            crate::protocol::responses::normalize_responses(serde_json::json!({"input":"hello"}))
                .unwrap();
        let mut ctx = PipelineContext::new(req, None);
        let mut headers = HeaderMap::new();
        headers.insert("x-session-id", "fixture-session".parse().unwrap());
        capture(&mut ctx, &headers);
        assert_eq!(ctx.client_session_hints["x-session-id"], "fixture-session");
        assert!(!ctx.request_headers.contains_key("x-session-id"));
        assert!(ctx.session.is_none());
    }

    #[test]
    fn conflicting_invalid_or_duplicate_root_aliases_are_discarded_together() {
        for name in ["session-id", "session_id"] {
            let alias = if name == "session-id" {
                "session_id"
            } else {
                "session-id"
            };
            for kind in ["duplicate", "oversized", "empty", "control", "conflict"] {
                let req = crate::protocol::responses::normalize_responses(
                    serde_json::json!({"input":"hello"}),
                )
                .unwrap();
                let mut ctx = PipelineContext::new(req, None);
                ctx.client_session_hints
                    .insert("session-id".into(), "stale-root".into());
                ctx.client_session_hints
                    .insert("session_id".into(), "stale-root".into());
                let mut headers = HeaderMap::new();
                let value = match kind {
                    "oversized" => "x".repeat(257),
                    "empty" => String::new(),
                    "control" => "x\ty".into(),
                    _ => "different-root".into(),
                };
                headers.insert(name, value.parse().unwrap());
                if kind == "duplicate" {
                    headers.append(name, "different-root".parse().unwrap());
                }
                headers.insert(alias, "shared-root".parse().unwrap());
                headers.insert("x-session-affinity", "unchanged".parse().unwrap());
                capture(&mut ctx, &headers);
                assert!(
                    !ctx.client_session_hints.contains_key(name),
                    "{name}:{kind}"
                );
                assert!(
                    !ctx.client_session_hints.contains_key(alias),
                    "{name}:{kind}"
                );
                assert_eq!(ctx.client_session_hints["x-session-affinity"], "unchanged");
            }
        }
    }

    #[test]
    fn root_session_labels_survive_independent_child_thread_ids() {
        for (id, both) in [("child-a", false), ("child-b", true)] {
            let req = crate::protocol::responses::normalize_responses(
                serde_json::json!({"input":"hello"}),
            )
            .unwrap();
            let mut ctx = PipelineContext::new(req, None);
            let mut headers = HeaderMap::new();
            headers.insert("thread-id", id.parse().unwrap());
            headers.insert("session-id", "same-root-session".parse().unwrap());
            if both {
                headers.insert("session_id", "same-root-session".parse().unwrap());
            }
            capture(&mut ctx, &headers);
            assert_eq!(ctx.client_session_hints["session-id"], "same-root-session");
            assert_eq!(ctx.client_session_hints.contains_key("session_id"), both);
            assert!(!ctx.request_headers.contains_key("session-id"));
            assert!(ctx.session.is_none());
        }
    }
}
