use super::*;
#[path = "streaming/profiled_sender.rs"]
mod profiled_sender;

#[cfg(all(test, feature = "line-google-agent-platform-official"))]
pub(super) fn vertex_express_unboxed_streaming_size(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
) -> usize {
    super::vertex_express_future_tests::future_size(|| {
        client.execute_stream_with_provider_account_id_inner(
            "",
            payload,
            req,
            "gemini-contract",
            None,
        )
    })
}

impl UpstreamClient {
    pub async fn execute_stream(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<UpstreamStreamingResponse, GatewayError> {
        if !crate::protocol::lobsterai::owns_payload(payload) {
            crate::protocol::lobsterai::management_boundary::reject_generic(payload)?;
        }
        let flow = crate::protocol::call_flow::prepare(payload)?;
        let payload = flow.payload.as_ref();
        crate::protocol::bedrock_turn::validate_request(payload, req)?;
        if Self::uses_native_claude(payload, req, model) {
            return self
                .send_native_claude(payload, req, model, extra_headers, None)
                .await;
        }
        self.execute_stream_with_provider_account_id("", payload, req, model, extra_headers)
            .await
    }

    pub fn execute_stream_with_provider_account_id<'a>(
        &'a self,
        provider_account_id: &'a str,
        payload: &'a ProviderAccountPayload,
        req: &'a CanonicalRelayRequest,
        model: &'a str,
        extra_headers: Option<&'a std::collections::HashMap<String, String>>,
    ) -> std::pin::Pin<
        Box<
            impl std::future::Future<Output = Result<UpstreamStreamingResponse, GatewayError>> + 'a,
        >,
    > {
        // These bound Gemini products aggregate; do not nest the large dispatcher.
        Box::pin(async move {
            if crate::upstream::vertex_product::for_payload(payload).is_some() {
                let canonical = self
                    .execute_with_provider_account_id(
                        provider_account_id,
                        payload,
                        req,
                        model,
                        extra_headers,
                    )
                    .await?;
                return Ok(gemini_api_modular::build_fake_openai_sse_bridge(
                    req, model, &canonical,
                ));
            }
            Box::pin(self.execute_stream_with_provider_account_id_inner(
                provider_account_id,
                payload,
                req,
                model,
                extra_headers,
            ))
            .await
        })
    }

    async fn execute_stream_with_provider_account_id_inner(
        &self,
        provider_account_id: &str,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<UpstreamStreamingResponse, GatewayError> {
        if !crate::protocol::lobsterai::owns_payload(payload) {
            crate::protocol::lobsterai::management_boundary::reject_generic(payload)?;
        }
        let flow = crate::protocol::call_flow::prepare(payload)?;
        let payload = flow.payload.as_ref();
        if crate::protocol::lobsterai::owns_payload(payload) {
            return crate::protocol::lobsterai::runtime::execute_stream(self, payload, req, model)
                .await;
        }
        if crate::protocol::commandcode::owns_payload(payload) {
            return crate::protocol::commandcode::runtime::execute_stream(
                self, payload, req, model,
            )
            .await;
        }
        if crate::protocol::sap_aicore::owns_payload(payload) {
            return crate::protocol::sap_aicore::runtime::execute_stream(self, payload, req, model)
                .await;
        }
        if crate::protocol::quick_oauth::owns_payload(payload) {
            return crate::protocol::quick_oauth::runtime::execute_stream(
                self, payload, req, model,
            )
            .await;
        }
        crate::protocol::bedrock_turn::validate_request(payload, req)?;
        if crate::upstream::grok_web::owns_payload(payload) {
            return crate::upstream::grok_web::execute_stream(
                &self.http,
                self.timeout,
                payload,
                req,
                model,
            )
            .await;
        }
        crate::protocol::vertex_express::validate_request(payload, req, model)?;
        crate::protocol::vertex_oauth::business::validate_request(payload, req, model)?;
        if flow.executor == crate::protocol::perplexity_web::material::ADAPTER {
            return crate::upstream::perplexity_web::execute_stream(
                &self.http,
                self.timeout,
                payload,
                req,
                model,
            )
            .await;
        }
        if matches!(
            flow.executor,
            "stitch_web_compatible" | "antigravity_oauth_compatible"
        ) {
            // Bounded native accumulation then canonical SSE, not token-by-token forwarding.
            let response = self
                .execute_with_provider_account_id(
                    provider_account_id,
                    payload,
                    req,
                    model,
                    extra_headers,
                )
                .await?;
            let frames = crate::upstream::canonical_sse::canonical_response_to_openai_sse_bytes(
                req, model, &response,
            );
            return Ok(UpstreamStreamingResponse::Bytes(Box::pin(
                futures::stream::iter(frames.into_iter().map(Ok)),
            )));
        }
        if flow.executor == crate::protocol::chatgpt::basispoints::ADAPTER {
            return crate::upstream::basispoints::execute_stream(
                &self.http,
                self.timeout,
                payload,
                req,
                model,
            )
            .await;
        }
        if payload.canonical_adapter() == "xfyun_websocket_compatible" {
            let _ = extra_headers;
            debug!(
                model,
                "sending upstream streaming request (xfyun websocket native)"
            );
            return xfyun_websocket::execute_stream_as_openai_sse(payload, req, model)
                .await
                .map(UpstreamStreamingResponse::Bytes);
        }

        if payload.adapter == "qwen_web_compatible" {
            debug!(
                model,
                "sending upstream streaming request (qwen web: create chat + translate)"
            );
            return self
                .execute_qwen_web_stream(payload, req, model, extra_headers)
                .await;
        }

        if flow.executor == "chatgpt_web_reverse_compatible" {
            debug!(
                model,
                "sending upstream streaming request (chatgpt web reverse: bootstrap + sentinel + conversation -> fake openai sse)"
            );
            return chatgpt_upstream::execute_stream(
                &self.http,
                self.timeout,
                payload,
                req,
                model,
                extra_headers,
            )
            .await;
        }

        if aistudio_web_reverse_modular::is_aistudio_web_reverse_adapter(&payload.adapter) {
            debug!(
                model,
                "sending upstream streaming request (aistudio web reverse: browser-owned generateContent -> fake openai sse)"
            );
            return self
                .execute_aistudio_web_stream(
                    provider_account_id,
                    payload,
                    req,
                    model,
                    extra_headers,
                )
                .await
                .map(UpstreamStreamingResponse::Bytes);
        }

        if flow.executor == "gemini_web_compatible" {
            debug!(
                model,
                "sending upstream streaming request (gemini web: bootstrap app + StreamGenerate -> fake openai sse)"
            );
            return gemini_web_reverse_modular::execute_stream(
                &self.http,
                self.timeout,
                payload,
                req,
                model,
                extra_headers,
            )
            .await
            .map(UpstreamStreamingResponse::Bytes);
        }
        if let Some(legacy_route) = gemini_web_reverse_modular::legacy_mixed_lane_execution_route(
            payload,
            req.endpoint_kind,
            true,
        ) {
            if legacy_route.kind
                == gemini_web_reverse_modular::LegacyMixedLaneExecutionKind::TextStream
            {
                debug!(
                    model,
                    endpoint = ?req.endpoint_kind,
                    "sending upstream streaming request (gemini web reverse modular: legacy mixed-lane text -> fake openai sse)"
                );
                return gemini_web_reverse_modular::execute_legacy_text_stream(
                    self,
                    &legacy_route.payload,
                    req,
                    model,
                    extra_headers,
                )
                .await
                .map(UpstreamStreamingResponse::Bytes);
            }
        }

        if payload.adapter == "gemini_canvas_compatible"
            && matches!(
                req.endpoint_kind,
                EndpointKind::ChatCompletions
                    | EndpointKind::Messages
                    | EndpointKind::Responses
                    | EndpointKind::Completions
            )
        {
            debug!(
                model,
                endpoint = ?req.endpoint_kind,
                "sending upstream streaming request (gemini canvas: browser-backed reverse-web text -> fake openai sse)"
            );
            return self
                .execute_gemini_canvas_text_stream(payload, req, model, extra_headers)
                .await
                .map(UpstreamStreamingResponse::Bytes);
        }
        if payload.adapter == "gemini_canvas_web_reverse_compatible"
            && matches!(
                req.endpoint_kind,
                EndpointKind::ChatCompletions
                    | EndpointKind::Messages
                    | EndpointKind::Responses
                    | EndpointKind::Completions
            )
        {
            let http_owned_payload =
                gemini_canvas_web_reverse_modular::force_http_owned_payload(payload);
            debug!(
                model,
                endpoint = ?req.endpoint_kind,
                "sending upstream streaming request (gemini canvas HTTP-only host text -> buffered openai sse)"
            );
            return self
                .execute_gemini_canvas_modular_browser_relay_text_stream(
                    &http_owned_payload,
                    req,
                    model,
                    extra_headers,
                )
                .await
                .map(UpstreamStreamingResponse::Bytes);
        }
        if payload.adapter == "gemini_canvas_program_web_reverse_compatible"
            && matches!(
                req.endpoint_kind,
                EndpointKind::ChatCompletions
                    | EndpointKind::Messages
                    | EndpointKind::Responses
                    | EndpointKind::Completions
            )
        {
            let program_owned_payload =
                gemini_canvas_program_web_reverse_modular::force_program_owned_payload(payload);
            debug!(
                model,
                endpoint = ?req.endpoint_kind,
                "sending upstream streaming request (gemini canvas program modular: concrete-program text -> fake openai sse)"
            );
            return self
                .execute_gemini_canvas_modular_browser_relay_text_stream(
                    &program_owned_payload,
                    req,
                    model,
                    extra_headers,
                )
                .await
                .map(UpstreamStreamingResponse::Bytes);
        }

        if gemini_api_modular::supports_fake_openai_sse_bridge(
            payload.adapter.as_str(),
            req.endpoint_kind,
        ) {
            let canonical = self
                .execute_with_provider_account_id(
                    provider_account_id,
                    payload,
                    req,
                    model,
                    extra_headers,
                )
                .await?;
            return Ok(gemini_api_modular::build_fake_openai_sse_bridge(
                req, model, &canonical,
            ));
        }

        if chatgpt_official_api_modular::owns_payload(payload) {
            return self
                .execute_chatgpt_official_streaming(payload, req, model, extra_headers)
                .await;
        }

        let (plan, headers, provider) = (
            Self::build_request_plan(payload, req, model, true)?,
            build_inference_headers(payload, req, model, extra_headers),
            payload.adapter.clone(),
        );

        debug!(url = %plan.url, model, "sending upstream streaming request");

        let mut headers = headers;
        crate::upstream::request_profile::apply_cpa_transport_headers(
            &mut headers,
            payload,
            req,
            model,
            true,
        );
        // Cohere's native SSE decoder owns this representation, not the inbound Accept.
        if payload.canonical_adapter() == "cohere_compatible" {
            headers.insert(
                rquest::header::ACCEPT,
                rquest::header::HeaderValue::from_static("text/event-stream"),
            );
        }
        if matches!(
            payload.adapter.as_str(),
            "dashscope_compatible" | "dashscope_multimodal_compatible"
        ) {
            headers.insert(
                "x-dashscope-sse",
                rquest::header::HeaderValue::from_static("enable"),
            );
        }

        let response = crate::upstream::kiro_oauth_runtime::receive_response(
            payload,
            self.send_profiled_plan(payload, &plan, headers)?
                .send()
                .await,
        )?;

        if !response.status().is_success() {
            return Err(crate::upstream::response_error::classify_response_error(
                response,
                provider.as_str(),
                "upstream HTTP error body",
            )
            .await);
        }

        Ok(UpstreamStreamingResponse::Http(response))
    }

    // ── helpers ───────────────────────────────────────────────────────────

    pub(super) async fn execute_qwen_web(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<CanonicalRelayResponse, GatewayError> {
        if crate::upstream::qwen::web_session::owns_payload(payload) {
            return crate::upstream::qwen::web_session::execute(
                &self.http,
                self.timeout,
                payload,
                req,
                model,
            )
            .await;
        }
        qwen_web_reverse_modular::execute(self, payload, req, model, extra_headers).await
    }

    pub(super) async fn execute_qwen_web_stream(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<UpstreamStreamingResponse, GatewayError> {
        if crate::upstream::qwen::web_session::owns_payload(payload) {
            return crate::upstream::qwen::web_session::execute_stream(
                &self.http,
                self.timeout,
                payload,
                req,
                model,
            )
            .await;
        }
        qwen_web_reverse_modular::execute_stream(self, payload, req, model, extra_headers)
            .await
            .map(UpstreamStreamingResponse::Bytes)
    }

    pub(crate) fn send_plan(
        &self,
        plan: &RequestPlan,
        headers: rquest::header::HeaderMap,
    ) -> RequestBuilder {
        self.send_plan_with_body(plan, headers, None)
    }

    fn send_plan_with_body(
        &self,
        plan: &RequestPlan,
        headers: rquest::header::HeaderMap,
        prepared_body: Option<Vec<u8>>,
    ) -> RequestBuilder {
        let http = if crate::request_wait::transport::managed() {
            self.wait_http.get_or_init(|| {
                crate::http_client::builder()
                    .emulation(Emulation::Chrome136)
                    .connect_timeout(self.timeout.min(Duration::from_secs(15)))
                    .build()
                    .expect("failed to build recovery HTTP client")
            })
        } else {
            &self.http
        };
        crate::upstream::common::build_request_builder_with_body(
            http,
            self.timeout,
            plan,
            headers,
            prepared_body,
        )
    }
}
