//! Request-local developer override. Public release binaries always reject it.
use crate::error::GatewayError;
use axum::http::HeaderMap;

pub const HEADER: &str = "x-gateway-debug-upstream-protocol";
pub fn requested_target(headers: &HeaderMap) -> Result<Option<String>, GatewayError> {
    let Some(value) = headers.get(HEADER) else {
        return Ok(None);
    };
    if !cfg!(all(feature = "dev-protocol-override", debug_assertions)) {
        return Err(GatewayError::bad_request(
            "Upstream protocol override is disabled in this build.",
        )
        .with_code("debug_protocol_override_disabled"));
    }
    if headers.get_all(HEADER).iter().count() != 1 {
        return Err(GatewayError::bad_request("Specify one upstream protocol.")
            .with_code("invalid_debug_upstream_protocol"));
    }
    let target = value.to_str().unwrap_or_default().trim();
    if !matches!(
        target,
        "openai_chat"
            | "anthropic_messages"
            | "openai_responses"
            | "openai_legacy_completions"
            | "gemini_generate_content"
            | "cohere_chat"
            | "dashscope_text"
            | "dashscope_multimodal"
            | "bedrock_converse"
    ) {
        return Err(
            GatewayError::bad_request("Unknown debug upstream protocol.")
                .with_code("invalid_debug_upstream_protocol"),
        );
    }
    Ok(Some(target.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_header_is_inert_and_release_or_default_build_is_closed() {
        let mut headers = HeaderMap::new();
        assert!(requested_target(&headers).unwrap().is_none());
        headers.insert(HEADER, "anthropic_messages".parse().unwrap());
        let result = requested_target(&headers);
        if cfg!(all(feature = "dev-protocol-override", debug_assertions)) {
            assert_eq!(result.unwrap().as_deref(), Some("anthropic_messages"));
            headers.insert(HEADER, "invalid".parse().unwrap());
            assert!(requested_target(&headers).is_err());
        } else {
            assert_eq!(
                result.unwrap_err().code.as_deref(),
                Some("debug_protocol_override_disabled")
            );
        }
    }
}
