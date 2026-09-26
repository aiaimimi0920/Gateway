mod body;

use std::collections::HashMap;

use rquest::header::HeaderMap;
use rquest::RequestBuilder;

use crate::error::{classify_network_error, classify_upstream_error, GatewayError};
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse, EndpointKind};
use crate::protocol::chatgpt::official_api as surface;
use crate::protocol::responses;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::client::UpstreamClient;
use crate::upstream::common::RequestPlan;
use crate::upstream::headers::build_upstream_headers_with;
use crate::upstream::response_types::UpstreamStreamingResponse;

use super::request_plan::{build_request_plan, owns_payload};
use super::response::unpack_nonstreaming_response;

#[derive(Debug)]
pub struct OfficialExecuteContext<'a> {
    pub provider: &'a str,
    pub plan: RequestPlan,
    pub headers: HeaderMap,
}

pub fn supports_forced_streaming_accumulate(
    payload: &ProviderAccountPayload,
    endpoint_kind: EndpointKind,
) -> bool {
    owns_payload(payload) && payload.prefers_forced_streaming_responses(endpoint_kind)
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
        let body_text = body::read_error(response, provider).await?;
        return Err(classify_upstream_error(
            status.into(),
            &body_text,
            Some(provider),
        ));
    }

    responses::accumulate_responses_stream(response, model).await
}

pub fn prepare_execute_context<'a>(
    payload: &'a ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
    stream: bool,
) -> Result<OfficialExecuteContext<'a>, GatewayError> {
    Ok(OfficialExecuteContext {
        provider: surface::provider_line_name(payload),
        plan: build_request_plan(payload, req, model, stream)?,
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

impl UpstreamClient {
    pub(crate) async fn execute_chatgpt_official_forced_streaming_accumulate(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<CanonicalRelayResponse, GatewayError> {
        let context = prepare_forced_streaming_execute_context(payload, req, model, extra_headers)?;
        execute_forced_streaming_accumulate(
            self.send_plan(&context.plan, context.headers),
            context.provider,
            model,
        )
        .await
    }

    pub(crate) async fn execute_chatgpt_official_nonstreaming(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<CanonicalRelayResponse, GatewayError> {
        let context = prepare_nonstreaming_execute_context(payload, req, model, extra_headers)?;
        let response = self
            .send_plan(&context.plan, context.headers)
            .send()
            .await
            .map_err(|error| classify_network_error(&error, Some(context.provider)))?;

        let status = response.status().as_u16();
        if !response.status().is_success() {
            let body_text = body::read_error(response, context.provider).await?;
            return Err(classify_upstream_error(
                status,
                &body_text,
                Some(context.provider),
            ));
        }

        let body = body::read_json(response, context.provider).await?;

        unpack_nonstreaming_response(req.endpoint_kind, &body)
    }

    pub(crate) async fn execute_chatgpt_official_streaming(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<UpstreamStreamingResponse, GatewayError> {
        let context = prepare_execute_context(payload, req, model, extra_headers, true)?;
        let response = self
            .send_plan(&context.plan, context.headers)
            .send()
            .await
            .map_err(|error| classify_network_error(&error, Some(context.provider)))?;

        let status = response.status().as_u16();
        if !response.status().is_success() {
            let body_text = body::read_error(response, context.provider).await?;
            return Err(classify_upstream_error(
                status,
                &body_text,
                Some(context.provider),
            ));
        }

        Ok(UpstreamStreamingResponse::Http(response))
    }
}

#[cfg(test)]
mod tests {
    mod body_limits;
    mod body_preservation;
    mod body_targets;
    mod body_wire;

    use super::*;
    use crate::protocol::canonical::{CanonicalMessage, ContentPart, MessageRole, ProtocolFamily};
    use axum::http::StatusCode;
    use axum::{routing::post, Json, Router};
    use futures::StreamExt;
    use tokio::net::TcpListener;

    fn make_payload(base_url: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
            adapter: "openai_compatible".to_string(),
            base_url: base_url.to_string(),
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
            requested_model: Some("gpt-5.4".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "hello chatgpt official".to_string(),
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
    fn supports_forced_streaming_accumulate_only_for_owned_payloads() {
        let mut official = make_payload("https://api.openai.com");
        official.responses_path = Some("/v1/responses".to_string());
        assert!(supports_forced_streaming_accumulate(
            &official,
            EndpointKind::ChatCompletions
        ));

        let mut generic = make_payload("https://api.deepseek.com");
        generic.responses_path = Some("/v1/responses".to_string());
        assert!(!supports_forced_streaming_accumulate(
            &generic,
            EndpointKind::ChatCompletions
        ));
    }

    #[test]
    fn prepare_execute_context_builds_official_plan_and_headers() {
        let payload = make_payload("https://api.openai.com");
        let req = make_request(EndpointKind::ChatCompletions);
        let mut extra_headers = HashMap::new();
        extra_headers.insert("x-fixture-header".to_string(), "fixture".to_string());

        let context =
            prepare_nonstreaming_execute_context(&payload, &req, "gpt-5.4", Some(&extra_headers))
                .expect("context");

        assert_eq!(context.provider, surface::CHATGPT_OFFICIAL_API_PROFILE);
        assert_eq!(
            context.plan.url,
            "https://api.openai.com/v1/chat/completions"
        );
        assert!(context.headers.contains_key("x-fixture-header"));
    }

    #[test]
    fn prepare_execute_context_uses_codex_provider_line_name() {
        let mut payload = make_payload("https://chatgpt.com/backend-api/codex");
        payload
            .headers
            .insert("Originator".to_string(), "codex_cli_rs".to_string());
        let req = make_request(EndpointKind::ChatCompletions);

        let context =
            prepare_nonstreaming_execute_context(&payload, &req, "gpt-5.4", None).expect("ctx");

        assert_eq!(context.provider, surface::CHATGPT_CODEX_BACKEND_PROFILE);
        assert!(context.plan.url.contains("/backend-api/codex"));
    }

    #[tokio::test]
    async fn execute_chatgpt_official_nonstreaming_returns_canonical_response() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("addr");
        let app = Router::new().route(
            "/api.openai.com/v1/chat/completions",
            post(|| async move {
                (
                    StatusCode::OK,
                    Json(serde_json::json!({
                        "id": "chatcmpl-123",
                        "model": "gpt-4o",
                        "choices": [{
                            "index": 0,
                            "message": {"role": "assistant", "content": "Hello there!"},
                            "finish_reason": "stop"
                        }]
                    })),
                )
            }),
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve");
        });

        let payload = make_payload(&format!("http://{}/api.openai.com", address));
        let req = make_request(EndpointKind::ChatCompletions);
        let client = UpstreamClient::new(5);

        let canonical = client
            .execute_chatgpt_official_nonstreaming(&payload, &req, "gpt-4o", None)
            .await
            .expect("canonical");

        assert_eq!(canonical.text, "Hello there!");
        assert_eq!(canonical.model, "gpt-4o");
        assert_eq!(canonical.finish_reason.as_deref(), Some("stop"));
    }

    #[tokio::test]
    async fn execute_chatgpt_official_forced_streaming_accumulate_returns_canonical_response() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("addr");
        let app = Router::new().route(
            "/api.openai.com/v1/responses",
            post(|| async move {
                (
                    StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
                    concat!(
                        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hello\"}\n\n",
                        "data: {\"type\":\"response.output_text.delta\",\"delta\":\" there\"}\n\n",
                        "data: {\"type\":\"response.completed\"}\n\n",
                        "data: [DONE]\n\n"
                    ),
                )
            }),
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve");
        });

        let mut payload = make_payload(&format!("http://{}/api.openai.com", address));
        payload.responses_path = Some("/v1/responses".to_string());
        let req = make_request(EndpointKind::ChatCompletions);
        let client = UpstreamClient::new(5);

        let canonical = client
            .execute_chatgpt_official_forced_streaming_accumulate(&payload, &req, "gpt-4o", None)
            .await
            .expect("canonical");

        assert_eq!(canonical.text, "Hello there");
    }

    #[tokio::test]
    async fn execute_chatgpt_official_streaming_returns_http_response() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("addr");
        let app = Router::new().route(
            "/api.openai.com/v1/chat/completions",
            post(|| async move {
                (
                    StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
                    "data: {\"id\":\"chunk-1\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hi\"},\"finish_reason\":null}]}\n\n",
                )
            }),
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve");
        });

        let payload = make_payload(&format!("http://{}/api.openai.com", address));
        let req = make_request(EndpointKind::ChatCompletions);
        let client = UpstreamClient::new(5);

        let stream = client
            .execute_chatgpt_official_streaming(&payload, &req, "gpt-4o", None)
            .await
            .expect("stream");

        let UpstreamStreamingResponse::Http(response) = stream else {
            panic!("expected http response");
        };
        let bytes = response
            .bytes_stream()
            .next()
            .await
            .expect("chunk")
            .expect("ok");
        let text = String::from_utf8(bytes.to_vec()).expect("utf8");
        assert!(text.contains("\"content\":\"hi\""));
    }
}
