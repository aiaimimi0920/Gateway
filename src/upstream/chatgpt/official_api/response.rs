use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayResponse, EndpointKind};
use crate::protocol::{openai, responses};

pub fn unpack_nonstreaming_response(
    endpoint_kind: EndpointKind,
    body: &Value,
) -> Result<CanonicalRelayResponse, GatewayError> {
    match endpoint_kind {
        EndpointKind::Responses => responses::unpack_responses_response(body)
            .or_else(|_| openai::unpack_openai_response(body)),
        _ => openai::unpack_openai_response(body)
            .or_else(|_| responses::unpack_responses_response(body)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn responses_endpoint_prefers_responses_shape() {
        let body = json!({
            "id": "resp_test",
            "model": "gpt-4o",
            "status": "completed",
            "output": [{
                "type": "message",
                "role": "assistant",
                "content": [{
                    "type": "output_text",
                    "text": "Hello"
                }]
            }]
        });
        let response =
            unpack_nonstreaming_response(EndpointKind::Responses, &body).expect("response");
        assert_eq!(response.text, "Hello");
        assert_eq!(response.model, "gpt-4o");
    }

    #[test]
    fn chat_endpoint_prefers_openai_shape() {
        let body = json!({
            "id": "chatcmpl-123",
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "Hello there!"},
                "finish_reason": "stop"
            }]
        });
        let response =
            unpack_nonstreaming_response(EndpointKind::ChatCompletions, &body).expect("response");
        assert_eq!(response.text, "Hello there!");
        assert_eq!(response.finish_reason.as_deref(), Some("stop"));
    }

    #[test]
    fn official_api_unpack_does_not_accept_accio_only_shape() {
        let body = json!({
            "message": {
                "content": [{"text": "Let me call a tool"}],
                "tool_calls": [{
                    "id": "call_1",
                    "function": {
                        "name": "weather",
                        "arguments": {"city": "Hangzhou"}
                    }
                }]
            },
            "finish_reason": "tool_call",
            "model": "command-r"
        });

        assert!(unpack_nonstreaming_response(EndpointKind::ChatCompletions, &body).is_err());
    }
}
