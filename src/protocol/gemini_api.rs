use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::canonical::{
    CanonicalRelayRequest, CanonicalRelayResponse, CanonicalToolCall, TokenUsage,
};
use crate::protocol::gemini::api as modular;

pub fn pack_gemini(req: &CanonicalRelayRequest, model: &str, stream: bool) -> Value {
    modular::pack_request(req, model, stream)
}

pub fn normalize_generate_content(
    body: Value,
    path_model: Option<String>,
    stream: bool,
) -> Result<CanonicalRelayRequest, GatewayError> {
    modular::normalize_generate_content(body, path_model, stream)
}

pub fn build_generate_content_success(
    model: &str,
    text: &str,
    usage: Option<&TokenUsage>,
    tool_calls: &[CanonicalToolCall],
    finish_reason: Option<&str>,
) -> Value {
    modular::build_generate_content_success(model, text, usage, tool_calls, finish_reason)
}

pub fn parse_generate_content_response(
    body: &Value,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    modular::parse_response(body, model)
}

pub async fn accumulate_gemini_stream(
    response: rquest::Response,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    modular::accumulate_gemini_stream(response, model).await
}

pub fn translate_openai_sse_to_gemini_stream(
    inner: impl futures::Stream<Item = Result<bytes::Bytes, rquest::Error>> + Send + 'static,
    model: String,
) -> impl futures::Stream<Item = Result<bytes::Bytes, rquest::Error>> + Send + 'static {
    modular::translate_openai_sse_to_gemini_stream(inner, model)
}

pub fn default_path(model: &str, stream: bool) -> String {
    modular::default_path(model, stream)
}

pub fn default_query(stream: bool) -> Vec<(String, String)> {
    modular::default_query(stream)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::{
        CanonicalMessage, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
    };
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn legacy_facade_still_delegates_request_and_transport() {
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some("gemini-2.5-pro".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "hi".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        };

        let body = pack_gemini(&req, "gemini-2.5-pro", false);
        assert_eq!(body["model"], "gemini-2.5-pro");
        assert_eq!(body["contents"][0]["parts"][0]["text"], "hi");
        assert_eq!(
            default_path("gemini-2.5-pro", true),
            "/models/gemini-2.5-pro:streamGenerateContent"
        );
        assert_eq!(
            default_query(true),
            vec![("alt".to_string(), "sse".to_string())]
        );
    }
}
