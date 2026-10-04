use super::*;

impl UpstreamClient {
    pub async fn execute_with_provider_account_id(
        &self,
        provider_account_id: &str,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<CanonicalRelayResponse, GatewayError> {
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

        if payload.adapter == "chatgpt_web_reverse_compatible" {
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

        if matches!(payload.adapter.as_str(), "gemini_web_compatible") {
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
            let browser_owned_payload =
                gemini_canvas_web_reverse_modular::force_browser_owned_payload(payload);
            debug!(
                model,
                endpoint = ?req.endpoint_kind,
                "sending upstream request (gemini canvas browser relay modular: connected-canvas text accumulate)"
            );
            return self
                .execute_gemini_canvas_modular_browser_relay_text(
                    &browser_owned_payload,
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
            let headers = build_upstream_headers_with(payload, extra_headers);
            let provider = &payload.adapter;

            debug!(
                url = %plan.url,
                model,
                endpoint = ?req.endpoint_kind,
                "sending upstream request (openai-family text bridge: forced streaming + accumulate)"
            );

            let response = self
                .send_plan(&plan, headers)
                .send()
                .await
                .map_err(|e| classify_network_error(&e, Some(provider)))?;

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
            let plan = context.plan;
            let headers = context.headers;

            debug!(
                url = %plan.url,
                model,
                adapter = %provider,
                "sending upstream request (gemini api: forced streaming + accumulate)"
            );

            return gemini_api_modular::execute_forced_streaming_accumulate(
                self.send_plan(&plan, headers),
                provider,
                model,
            )
            .await;
        }

        // Accio-style adapters return stream/event-oriented payloads even for
        // nominally non-streaming requests, so force streaming and accumulate.
        if payload.adapter == "accio_compatible" {
            let plan = Self::build_request_plan(payload, req, model, true)?;
            let headers = build_upstream_headers_with(payload, extra_headers);
            let provider = &payload.adapter;

            debug!(url = %plan.url, model, adapter = %payload.adapter, "sending upstream request (streaming adapter: forced streaming + accumulate)");

            let response = self
                .send_plan(&plan, headers)
                .send()
                .await
                .map_err(|e| classify_network_error(&e, Some(provider)))?;

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
            let headers = sign_bedrock_runtime_headers_if_needed(
                payload,
                &plan,
                build_upstream_headers_with(payload, extra_headers),
            )?;
            let provider = &payload.adapter;

            debug!(url = %plan.url, model, adapter = %payload.adapter, "sending upstream request (bedrock converse: forced streaming + accumulate)");

            let response = self
                .send_plan(&plan, headers)
                .send()
                .await
                .map_err(|e| classify_network_error(&e, Some(provider)))?;

            if !response.status().is_success() {
                return Err(crate::upstream::response_error::classify_response_error(
                    response,
                    provider,
                    "upstream HTTP error body",
                )
                .await);
            }

            return bedrock_converse_common::accumulate_stream(response, model).await;
        }

        if payload.adapter == "cohere_compatible" {
            let plan = Self::build_request_plan(payload, req, model, true)?;
            let headers = build_upstream_headers_with(payload, extra_headers);
            let provider = &payload.adapter;

            debug!(url = %plan.url, model, adapter = %payload.adapter, "sending upstream request (cohere chat: forced streaming + accumulate)");

            let response = self
                .send_plan(&plan, headers)
                .send()
                .await
                .map_err(|e| classify_network_error(&e, Some(provider)))?;

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
            let headers = build_upstream_headers_with(payload, extra_headers);
            let provider = &payload.adapter;

            debug!(url = %plan.url, model, "sending upstream request (grok: forced streaming + accumulate)");

            let response = self
                .send_plan(&plan, headers)
                .send()
                .await
                .map_err(|e| classify_network_error(&e, Some(provider)))?;

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
            let headers = build_upstream_headers_with(payload, extra_headers);
            let provider = &payload.adapter;

            debug!(url = %plan.url, model, "sending upstream request (kiro: forced streaming + accumulate)");

            let response = self
                .send_plan(&plan, headers)
                .send()
                .await
                .map_err(|e| classify_network_error(&e, Some(provider)))?;

            if !response.status().is_success() {
                return Err(crate::upstream::response_error::classify_response_error(
                    response,
                    provider,
                    "upstream HTTP error body",
                )
                .await);
            }

            return kiro_upstream::accumulate_stream(response, model, req).await;
        }

        if anthropic_messages_upstream::owns_payload(payload) {
            let plan = Self::build_request_plan(payload, req, model, true)?;
            let headers = build_upstream_headers_with(payload, extra_headers);
            let provider = &payload.adapter;

            debug!(
                url = %plan.url,
                model,
                "sending upstream request (anthropic: forced streaming + accumulate)"
            );

            let response = self
                .send_plan(&plan, headers)
                .send()
                .await
                .map_err(|e| classify_network_error(&e, Some(provider)))?;

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
            build_upstream_headers_with(payload, extra_headers),
            payload.adapter.clone(),
        );

        debug!(url = %plan.url, model, "sending upstream request");

        let response = self
            .send_plan(&plan, headers)
            .send()
            .await
            .map_err(|e| classify_network_error(&e, Some(provider.as_str())))?;

        if !response.status().is_success() {
            return Err(crate::upstream::response_error::classify_response_error(
                response,
                provider.as_str(),
                "upstream HTTP error body",
            )
            .await);
        }

        let body: Value = response
            .json()
            .await
            .map_err(|e| classify_network_error(&e, Some(provider.as_str())))?;

        let canonical = match payload.adapter.as_str() {
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
        }?;

        Ok(canonical)
    }
}
