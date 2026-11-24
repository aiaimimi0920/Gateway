use super::*;

#[cfg(all(test, feature = "line-google-agent-platform-official"))]
pub(super) fn vertex_express_unboxed_buffered_size(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
) -> usize {
    super::vertex_express_future_tests::future_size(|| {
        client.execute_with_provider_account_id_inner("", payload, req, "gemini-contract", None)
    })
}

impl UpstreamClient {
    pub fn execute_with_provider_account_id<'a>(
        &'a self,
        provider_account_id: &'a str,
        payload: &'a ProviderAccountPayload,
        req: &'a CanonicalRelayRequest,
        model: &'a str,
        extra_headers: Option<&'a std::collections::HashMap<String, String>>,
    ) -> std::pin::Pin<
        Box<impl std::future::Future<Output = Result<CanonicalRelayResponse, GatewayError>> + 'a>,
    > {
        // Construct the multi-provider future outside callers' poll frames. No task or cancellation owner changes.
        Box::pin(self.execute_with_provider_account_id_inner(
            provider_account_id,
            payload,
            req,
            model,
            extra_headers,
        ))
    }

    async fn execute_with_provider_account_id_inner(
        &self,
        provider_account_id: &str,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<CanonicalRelayResponse, GatewayError> {
        if crate::protocol::lobsterai::owns_payload(payload) {
            return crate::protocol::lobsterai::runtime::execute(self, payload, req, model).await;
        }
        crate::protocol::lobsterai::management_boundary::reject_generic(payload)?;
        let flow = crate::protocol::call_flow::prepare(payload)?;
        let payload = flow.payload.as_ref();
        if crate::protocol::commandcode::owns_payload(payload) {
            return crate::protocol::commandcode::runtime::execute(self, payload, req, model).await;
        }
        if crate::protocol::sap_aicore::owns_payload(payload) {
            return crate::protocol::sap_aicore::runtime::execute(self, payload, req, model).await;
        }
        if crate::protocol::quick_oauth::owns_payload(payload) {
            return crate::protocol::quick_oauth::runtime::execute(self, payload, req, model).await;
        }
        crate::protocol::bedrock_turn::validate_request(payload, req)?;
        if crate::upstream::grok_web::owns_payload(payload) {
            return crate::upstream::grok_web::execute(
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
            // Product-owned Cookie/UA only; caller headers are intentionally not forwarded.
            return crate::upstream::perplexity_web::execute(
                &self.http,
                self.timeout,
                payload,
                req,
                model,
            )
            .await;
        }
        if flow.executor == crate::protocol::antigravity::ADAPTER {
            return crate::upstream::antigravity::execute(
                &self.http,
                self.timeout,
                payload,
                req,
                model,
            )
            .await;
        }
        if flow.executor == "stitch_web_compatible" {
            return crate::upstream::stitch_web::execute(
                &self.http,
                self.timeout,
                payload,
                req,
                model,
            )
            .await;
        }
        if flow.executor == crate::protocol::chatgpt::basispoints::ADAPTER {
            return crate::upstream::basispoints::execute(
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
                "sending upstream request (xfyun websocket: stream-only upstream + accumulate)"
            );
            return xfyun_websocket::execute_nonstream(payload, req, model).await;
        }

        if payload.adapter == "qwen_web_compatible" {
            debug!(
                model,
                "sending upstream request (qwen web: create chat + accumulate)"
            );
            return self
                .execute_qwen_web(payload, req, model, extra_headers)
                .await;
        }

        if flow.executor == "chatgpt_web_reverse_compatible" {
            debug!(
                model,
                "sending upstream request (chatgpt web reverse: bootstrap + sentinel + conversation accumulate)"
            );
            return chatgpt_upstream::execute(
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
                "sending upstream request (aistudio web reverse: browser-owned generateContent accumulate)"
            );
            return self
                .execute_aistudio_web(provider_account_id, payload, req, model, extra_headers)
                .await;
        }

        if flow.executor == "gemini_web_compatible" {
            debug!(
                model,
                "sending upstream request (gemini web: bootstrap app + StreamGenerate accumulate)"
            );
            return gemini_web_reverse_modular::execute(
                &self.http,
                self.timeout,
                payload,
                req,
                model,
                extra_headers,
            )
            .await;
        }
        if let Some(legacy_route) = gemini_web_reverse_modular::legacy_mixed_lane_execution_route(
            payload,
            req.endpoint_kind,
            false,
        ) {
            if legacy_route.kind
                == gemini_web_reverse_modular::LegacyMixedLaneExecutionKind::TextAccumulate
            {
                debug!(
                    model,
                    endpoint = ?req.endpoint_kind,
                    "sending upstream request (gemini web reverse modular: legacy mixed-lane text accumulate)"
                );
                return gemini_web_reverse_modular::execute_legacy_text(
                    self,
                    &legacy_route.payload,
                    req,
                    model,
                    extra_headers,
                )
                .await;
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
                "sending upstream request (gemini canvas: browser-backed reverse-web text accumulate)"
            );
            return self
                .execute_gemini_canvas_text(payload, req, model, extra_headers)
                .await;
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
                "sending upstream request (gemini canvas HTTP-only host text accumulate)"
            );
            return self
                .execute_gemini_canvas_modular_browser_relay_text(
                    &http_owned_payload,
                    req,
                    model,
                    extra_headers,
                )
                .await;
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
                "sending upstream request (gemini canvas program modular: share-seeded concrete-program text accumulate)"
            );
            return self
                .execute_gemini_canvas_modular_browser_relay_text(
                    &program_owned_payload,
                    req,
                    model,
                    extra_headers,
                )
                .await;
        }

        let force_streaming_responses_bridge =
            payload.prefers_forced_streaming_responses(req.endpoint_kind);

        if chatgpt_official_api_modular::supports_forced_streaming_accumulate(
            payload,
            req.endpoint_kind,
        ) {
            return self
                .execute_chatgpt_official_forced_streaming_accumulate(
                    payload,
                    req,
                    model,
                    extra_headers,
                )
                .await;
        }

        if force_streaming_responses_bridge {
            let plan = Self::build_request_plan(payload, req, model, true)?;
            let headers = build_inference_headers(payload, req, model, extra_headers);
            let provider = &payload.adapter;

            debug!(
                url = %plan.url,
                model,
                endpoint = ?req.endpoint_kind,
                "sending upstream request (openai-family text bridge: forced streaming + accumulate)"
            );

            let response = self
                .send_profiled_plan(payload, &plan, headers)?
                .send()
                .await?;

            if !response.status().is_success() {
                return Err(crate::upstream::response_error::classify_response_error(
                    response,
                    provider,
                    "upstream HTTP error body",
                )
                .await);
            }

            return responses::accumulate_responses_stream(response, model).await;
        }

        if gemini_api_modular::supports_forced_streaming_accumulate(payload.adapter.as_str()) {
            let context = gemini_api_modular::prepare_forced_streaming_execute_context(
                payload,
                req,
                model,
                extra_headers,
            )?;
            let provider = context.provider;
            let plan =
                crate::upstream::vertex_history::prepare_plan(payload, req, context.plan, model)?;
            let headers = context.headers;

            debug!(
                url = %plan.url,
                model,
                adapter = %provider,
                "sending upstream request (gemini api: forced streaming + accumulate)"
            );

            let request = self.send_profiled_plan(payload, &plan, headers)?.send();
            if let Some(contract) = crate::upstream::vertex_product::for_payload(payload) {
                let verified = Box::pin(contract.read(request)).await?;
                return contract.project(verified, model);
            }
            return gemini_api_modular::execute_forced_streaming_accumulate(
                request, provider, model,
            )
            .await;
        }

        // Accio-style adapters return stream/event-oriented payloads even for
        // nominally non-streaming requests, so force streaming and accumulate.
        if payload.adapter == "accio_compatible" {
            let plan = Self::build_request_plan(payload, req, model, true)?;
            let headers = build_inference_headers(payload, req, model, extra_headers);
            let provider = &payload.adapter;

            debug!(url = %plan.url, model, adapter = %payload.adapter, "sending upstream request (streaming adapter: forced streaming + accumulate)");

            let response = self
                .send_profiled_plan(payload, &plan, headers)?
                .send()
                .await?;

            if !response.status().is_success() {
                return Err(crate::upstream::response_error::classify_response_error(
                    response,
                    provider,
                    "upstream HTTP error body",
                )
                .await);
            }

            return accio::accumulate_accio_stream(response, model).await;
        }

        if payload.adapter == "bedrock_converse_compatible" {
            let plan = Self::build_request_plan(payload, req, model, true)?;
            let headers = build_inference_headers(payload, req, model, extra_headers);
            let provider = &payload.adapter;

            debug!(url = %plan.url, model, adapter = %payload.adapter, "sending upstream request (bedrock converse: forced streaming + accumulate)");

            let response = self
                .send_profiled_plan(payload, &plan, headers)?
                .send()
                .await?;

            if !response.status().is_success() {
                return Err(crate::upstream::response_error::classify_response_error(
                    response,
                    provider,
                    "upstream HTTP error body",
                )
                .await);
            }

            return bedrock_converse_common::accumulate_for_request(response, model, &plan).await;
        }

        if payload.adapter == "cohere_compatible" {
            let plan = Self::build_request_plan(payload, req, model, true)?;
            let mut headers = build_inference_headers(payload, req, model, extra_headers);
            headers.insert(
                rquest::header::ACCEPT,
                rquest::header::HeaderValue::from_static("text/event-stream"),
            );
            let provider = &payload.adapter;

            debug!(url = %plan.url, model, adapter = %payload.adapter, "sending upstream request (cohere chat: forced streaming + accumulate)");

            let response = self
                .send_profiled_plan(payload, &plan, headers)?
                .send()
                .await?;

            if !response.status().is_success() {
                return Err(crate::upstream::response_error::classify_response_error(
                    response,
                    provider,
                    "upstream HTTP error body",
                )
                .await);
            }

            return cohere_chat_common::accumulate_stream(response, model).await;
        }

        // Grok always returns NDJSON, so force streaming and accumulate.
        if payload.adapter == "grok_compatible" {
            let plan = Self::build_request_plan(payload, req, model, true)?;
            let headers = build_inference_headers(payload, req, model, extra_headers);
            let provider = &payload.adapter;

            debug!(url = %plan.url, model, "sending upstream request (grok: forced streaming + accumulate)");

            let response = self
                .send_profiled_plan(payload, &plan, headers)?
                .send()
                .await?;

            if !response.status().is_success() {
                return Err(crate::upstream::response_error::classify_response_error(
                    response,
                    provider,
                    "upstream HTTP error body",
                )
                .await);
            }

            return grok::accumulate_grok_stream(response, model).await;
        }

        if payload.adapter == "kiro_compatible" {
            let plan = Self::build_request_plan(payload, req, model, true)?;
            let headers = build_inference_headers(payload, req, model, extra_headers);
            let provider = &payload.adapter;

            debug!(url = %plan.url, model, "sending upstream request (kiro: forced streaming + accumulate)");

            let response = crate::upstream::kiro_oauth_runtime::receive_response(
                payload,
                self.send_profiled_plan(payload, &plan, headers)?
                    .send()
                    .await,
            )?;

            if !response.status().is_success() {
                return Err(crate::upstream::response_error::classify_response_error(
                    response,
                    provider,
                    "upstream HTTP error body",
                )
                .await);
            }

            return if crate::protocol::kiro_oauth::runtime::owns_payload(payload) {
                crate::upstream::kiro_oauth_runtime::accumulate(response, model, req).await
            } else if crate::protocol::kiro_api_key::owns_payload(payload) {
                crate::upstream::kiro_api_key::accumulate(response, model, req).await
            } else {
                kiro_upstream::accumulate_stream(response, model, req).await
            };
        }

        if anthropic_messages_upstream::owns_payload(payload)
            && !crate::protocol::minimax::owns_payload(payload)
            && payload.request_client_profile
                != Some(crate::protocol::request_profile::RequestClientProfile::CpaClaudeCode)
        {
            let plan = Self::build_request_plan(payload, req, model, true)?;
            let headers = build_inference_headers(payload, req, model, extra_headers);
            let provider = &payload.adapter;

            debug!(
                url = %plan.url,
                model,
                "sending upstream request (anthropic: forced streaming + accumulate)"
            );

            let response = self
                .send_profiled_plan(payload, &plan, headers)?
                .send()
                .await?;

            if !response.status().is_success() {
                return Err(crate::upstream::response_error::classify_response_error(
                    response,
                    provider,
                    "upstream HTTP error body",
                )
                .await);
            }

            return anthropic_messages_upstream::accumulate_stream(response, model).await;
        }

        if chatgpt_official_api_modular::owns_payload(payload) {
            return self
                .execute_chatgpt_official_nonstreaming(payload, req, model, extra_headers)
                .await;
        }

        let (plan, headers, provider) = (
            Self::build_request_plan(payload, req, model, false)?,
            build_inference_headers(payload, req, model, extra_headers),
            payload.adapter.clone(),
        );

        debug!(url = %plan.url, model, "sending upstream request");

        let response = self
            .send_profiled_plan(payload, &plan, headers)?
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(crate::upstream::response_error::classify_response_error(
                response,
                provider.as_str(),
                "upstream HTTP error body",
            )
            .await);
        }

        super::buffered_response::read(response, payload, req, model, &provider).await
    }
}
