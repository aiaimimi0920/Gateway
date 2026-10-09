use std::collections::HashMap;

use futures::stream;
use rquest::header::HeaderMap;
use rquest::RequestBuilder;
use serde_json::Value;

use crate::error::{classify_network_error, classify_upstream_error, GatewayError};
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse, EndpointKind};
use crate::protocol::gemini_api as legacy_surface;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::canonical_sse::canonical_response_to_openai_sse_bytes;
use crate::upstream::common::RequestPlan;
use crate::upstream::headers::build_upstream_headers_with;
use crate::upstream::response_types::UpstreamStreamingResponse;

pub const GEMINI_API_LEGACY_ADAPTER: &str = "gemini_api_compatible";
pub const GEMINI_API_MODULAR_ADAPTER: &str = "gemini_api_modular_compatible";

#[derive(Debug)]
pub struct OfficialExecuteContext<'a> {
    pub provider: &'a str,
    pub plan: RequestPlan,
    pub headers: HeaderMap,
}

pub fn is_official_adapter(adapter: &str) -> bool {
    matches!(
        adapter,
        GEMINI_API_LEGACY_ADAPTER | GEMINI_API_MODULAR_ADAPTER
    )
}

pub fn supports_forced_streaming_accumulate(adapter: &str) -> bool {
    is_official_adapter(adapter)
}

pub async fn execute_forced_streaming_accumulate(
    request: RequestBuilder,
    provider: &str,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let response = request
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(provider)))?;

    let status = response.status();
    if !status.is_success() {
        let body_text = response
            .text()
            .await
            .unwrap_or_else(|_| String::from("<unreadable body>"));
        return Err(classify_upstream_error(
            status.into(),
            &body_text,
            Some(provider),
        ));
    }

    legacy_surface::accumulate_gemini_stream(response, model).await
}

pub fn parse_generate_content_response(
    body: &Value,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    legacy_surface::parse_generate_content_response(body, model)
}

pub fn supports_fake_openai_sse_bridge_endpoint(endpoint_kind: EndpointKind) -> bool {
    matches!(
        endpoint_kind,
        EndpointKind::ChatCompletions
            | EndpointKind::Messages
            | EndpointKind::Responses
            | EndpointKind::Completions
    )
}

pub fn supports_fake_openai_sse_bridge(adapter: &str, endpoint_kind: EndpointKind) -> bool {
    supports_forced_streaming_accumulate(adapter)
        && supports_fake_openai_sse_bridge_endpoint(endpoint_kind)
}

pub fn prepare_execute_context<'a>(
    payload: &'a ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
    stream: bool,
) -> Result<OfficialExecuteContext<'a>, GatewayError> {
    Ok(OfficialExecuteContext {
        provider: payload.adapter.as_str(),
        plan: super::build_request_plan(payload, req, model, stream)?,
        headers: build_upstream_headers_with(payload, extra_headers),
    })
}

pub fn prepare_forced_streaming_execute_context<'a>(
    payload: &'a ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<OfficialExecuteContext<'a>, GatewayError> {
    prepare_execute_context(payload, req, model, extra_headers, true)
}

pub fn prepare_nonstreaming_execute_context<'a>(
    payload: &'a ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<OfficialExecuteContext<'a>, GatewayError> {
    prepare_execute_context(payload, req, model, extra_headers, false)
}

pub fn prepare_forced_streaming_request(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<(RequestPlan, HeaderMap), GatewayError> {
    let context = prepare_forced_streaming_execute_context(payload, req, model, extra_headers)?;
    Ok((context.plan, context.headers))
}

pub fn build_fake_openai_sse_bridge(
    req: &CanonicalRelayRequest,
    model: &str,
    canonical: &CanonicalRelayResponse,
) -> UpstreamStreamingResponse {
    let sse_bytes = canonical_response_to_openai_sse_bytes(req, model, canonical)
        .into_iter()
        .map(Ok);
    UpstreamStreamingResponse::Bytes(Box::pin(stream::iter(sse_bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::{CanonicalMessage, ContentPart, MessageRole, ProtocolFamily};
    use crate::protocol::gemini::api as surface;

    fn make_payload(adapter: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
            discovered_protocols: Vec::new(),
            adapter: adapter.to_string(),
            base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
            api_key: "sk-test".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: None,
            headers: HashMap::new(),
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: None,
            chat_completions_path: None,
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            extra_body: None,
            session_auth: None,
            keepalive: None,
        }
    }

    fn make_request(endpoint_kind: EndpointKind) -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind,
            requested_model: Some("gemini-2.5-pro".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "hello modular gemini".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: serde_json::json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }

    #[test]
    fn official_adapter_detection_covers_legacy_and_modular_aliases() {
        assert!(is_official_adapter(GEMINI_API_LEGACY_ADAPTER));
        assert!(is_official_adapter(GEMINI_API_MODULAR_ADAPTER));
        assert!(!is_official_adapter("openai_compatible"));
    }

    #[test]
    fn fake_openai_sse_bridge_requires_official_adapter_and_supported_endpoint() {
        assert!(supports_fake_openai_sse_bridge(
            GEMINI_API_LEGACY_ADAPTER,
            EndpointKind::ChatCompletions
        ));
        assert!(supports_fake_openai_sse_bridge(
            GEMINI_API_MODULAR_ADAPTER,
            EndpointKind::Responses
        ));
        assert!(!supports_fake_openai_sse_bridge(
            "openai_compatible",
            EndpointKind::ChatCompletions
        ));
        assert!(!supports_fake_openai_sse_bridge(
            GEMINI_API_LEGACY_ADAPTER,
            EndpointKind::Embeddings
        ));
    }

    #[test]
    fn prepare_execute_context_builds_stream_and_nonstream_variants() {
        let payload = make_payload(GEMINI_API_MODULAR_ADAPTER);
        let request = make_request(EndpointKind::ChatCompletions);

        let streaming =
            prepare_forced_streaming_execute_context(&payload, &request, "gemini-2.5-pro", None)
                .expect("streaming context");
        assert_eq!(streaming.provider, GEMINI_API_MODULAR_ADAPTER);
        assert!(streaming.plan.url.contains(":streamGenerateContent"));
        assert_eq!(streaming.plan.query, surface::default_query(true));

        let nonstreaming =
            prepare_nonstreaming_execute_context(&payload, &request, "gemini-2.5-pro", None)
                .expect("nonstreaming context");
        assert_eq!(nonstreaming.provider, GEMINI_API_MODULAR_ADAPTER);
        assert!(nonstreaming.plan.url.contains(":generateContent"));
        assert_eq!(nonstreaming.plan.query, surface::default_query(false));
    }
}
