//! Preserve bounded native context only; selecting a style never invents a conversation.
use super::*;
use crate::protocol::workbuddy_wire as wire;

pub(super) fn capture(ctx: &mut PipelineContext, headers: &HeaderMap) -> Result<(), GatewayError> {
    if headers
        .get("user-agent")
        .is_none_or(|v| v != wire::USER_AGENT)
        || headers.get("x-ide-type").is_none_or(|v| v != "WorkBuddy")
        || headers.get("x-codebuddy-request").is_none_or(|v| v != "1")
    {
        return Ok(());
    }
    for name in wire::CONTEXT_HEADERS {
        if let Some(value) = headers.get(*name) {
            let value = value
                .to_str()
                .map_err(|_| GatewayError::bad_request("Invalid WorkBuddy context"))?;
            if !wire::valid(value) || headers.get_all(*name).iter().count() != 1 {
                return Err(GatewayError::bad_request(
                    "Invalid or repeated WorkBuddy context",
                ));
            }
            ctx.request_headers
                .insert((*name).to_owned(), value.to_owned());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (PipelineContext, HeaderMap) {
        let req = crate::protocol::openai::normalize_chat_completions(
            serde_json::json!({"messages":[],"model":"fixture"}),
        )
        .unwrap();
        let mut headers = HeaderMap::new();
        for (name, value) in [
            ("user-agent", wire::USER_AGENT),
            ("x-ide-type", "WorkBuddy"),
            ("x-codebuddy-request", "1"),
            ("x-conversation-id", "real-session"),
            ("x-private-data", "false"),
            ("x-user-id", "private-user"),
            ("authorization", "Bearer caller-secret"),
            ("cookie", "private-cookie"),
        ] {
            headers.insert(
                axum::http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                axum::http::HeaderValue::from_str(value).unwrap(),
            );
        }
        (PipelineContext::new(req, None), headers)
    }

    #[test]
    fn workbuddy_capture_keeps_real_context_without_identity_or_credentials() {
        let (mut ctx, headers) = fixture();
        capture(&mut ctx, &headers).unwrap();
        assert_eq!(ctx.request_headers.len(), 2);
        assert_eq!(ctx.request_headers["x-conversation-id"], "real-session");
        assert_eq!(ctx.request_headers["x-private-data"], "false");
    }

    #[test]
    fn workbuddy_capture_rejects_duplicate_and_oversize_context() {
        let (mut ctx, mut headers) = fixture();
        headers.append("x-conversation-id", "conflict".parse().unwrap());
        assert!(capture(&mut ctx, &headers).is_err());
        headers.insert("x-conversation-id", "x".repeat(513).parse().unwrap());
        assert!(capture(&mut ctx, &headers).is_err());
    }

    #[test]
    fn workbuddy_capture_does_not_invent_or_capture_other_client_state() {
        let (mut ctx, mut headers) = fixture();
        headers.remove("x-ide-type");
        capture(&mut ctx, &headers).unwrap();
        assert!(ctx.request_headers.is_empty());
    }
}
