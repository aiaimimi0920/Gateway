//! Capture bounded, untrusted correlation labels separately from the forwarded header map.
use super::*;

pub(super) fn capture(ctx: &mut PipelineContext, headers: &HeaderMap) {
    for name in ["session-id", "session_id", "x-http-session-id", "x-session-id",
        "x-session-affinity", "x-client-request-id"] {
        if headers.get_all(name).iter().count() != 1 { continue; }
        if let Some(value) = headers.get(name).and_then(|v| v.to_str().ok()) {
            if !value.chars().any(char::is_control) && !value.trim().is_empty() && value.trim().len() <= 256 {
                ctx.client_session_hints.insert(name.into(), value.trim().into());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cpa_session_hint_is_captured_without_forwarding_or_authentication() {
        let req = crate::protocol::responses::normalize_responses(serde_json::json!({"input":"hello"})).unwrap();
        let mut ctx = PipelineContext::new(req, None);
        let mut headers = HeaderMap::new();
        headers.insert("x-session-id", "fixture-session".parse().unwrap());
        capture(&mut ctx, &headers);
        assert_eq!(ctx.client_session_hints["x-session-id"], "fixture-session");
        assert!(!ctx.request_headers.contains_key("x-session-id"));
        assert!(ctx.session.is_none());
    }
}
