//! Buffered response wire dispatch after successful HTTP admission.
use super::*;
pub(super) async fn read(
    response: rquest::Response,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    provider: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    if crate::protocol::sensenova::owns_payload(payload) {
        return crate::protocol::sensenova::read_response(response).await;
    }
    if crate::protocol::qwen::workspace_api::owns_payload(payload) {
        // The workspace product always speaks Chat, independently of the caller's endpoint.
        return crate::protocol::qwen::workspace_response::read(response).await;
    }

    let body: Value = if crate::protocol::minimax::owns_payload(payload) {
        crate::protocol::minimax::read_json(response).await?
    } else {
        response
            .json()
            .await
            .map_err(|e| classify_network_error(&e, Some(provider)))?
    };

    crate::protocol::minimax::validate_json_response(payload, &body)?;
    match payload.adapter.as_str() {
        "dashscope_compatible" | "dashscope_multimodal_compatible" => {
            crate::protocol::dashscope::response::unpack(&body, model)
        }
        "anthropic_compatible" if anthropic_messages_upstream::owns_payload(payload) => {
            anthropic_messages_upstream::unpack_response(&body)
                .or_else(|_| accio::unpack_accio_response(&body))
        }
        "bedrock_converse_compatible" => bedrock_converse_common::unpack_response(&body)
            .or_else(|_| openai::unpack_openai_response(&body))
            .or_else(|_| responses::unpack_responses_response(&body)),
        "cohere_compatible" => cohere_chat_common::unpack_response(&body)
            .or_else(|_| openai::unpack_openai_response(&body))
            .or_else(|_| responses::unpack_responses_response(&body)),
        _ if req.endpoint_kind == EndpointKind::Responses => {
            responses::unpack_responses_response(&body)
                .or_else(|_| openai::unpack_openai_response(&body))
                .or_else(|_| accio::unpack_accio_response(&body))
        }
        _ => openai::unpack_openai_response(&body)
            .or_else(|_| responses::unpack_responses_response(&body))
            .or_else(|_| accio::unpack_accio_response(&body)),
    }
}
