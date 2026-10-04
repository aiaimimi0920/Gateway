use super::*;

impl UpstreamClient {
    pub async fn execute_stream(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<UpstreamStreamingResponse, GatewayError> {
        self.execute_stream_with_provider_account_id("", payload, req, model, extra_headers)
            .await
    }

    pub async fn execute_stream_with_provider_account_id(
        &self,
        provider_account_id: &str,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<UpstreamStreamingResponse, GatewayError> {
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
                .await
                .map(UpstreamStreamingResponse::Bytes);
        }

        if payload.adapter == "chatgpt_web_reverse_compatible" {
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

        if matches!(payload.adapter.as_str(), "gemini_web_compatible") {
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
            let browser_owned_payload =
                gemini_canvas_web_reverse_modular::force_browser_owned_payload(payload);
            debug!(
                model,
                endpoint = ?req.endpoint_kind,
                "sending upstream streaming request (gemini canvas browser relay modular: connected-canvas text -> fake openai sse)"
            );
            return self
                .execute_gemini_canvas_modular_browser_relay_text_stream(
                    &browser_owned_payload,
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
            build_upstream_headers_with(payload, extra_headers),
            payload.adapter.clone(),
        );

        debug!(url = %plan.url, model, "sending upstream streaming request");

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
        qwen_web_reverse_modular::execute(self, payload, req, model, extra_headers).await
    }

    pub(super) async fn execute_qwen_web_stream(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<
        std::pin::Pin<Box<dyn futures::Stream<Item = Result<bytes::Bytes, rquest::Error>> + Send>>,
        GatewayError,
    > {
        qwen_web_reverse_modular::execute_stream(self, payload, req, model, extra_headers).await
    }

    pub(crate) fn send_plan(
        &self,
        plan: &RequestPlan,
        headers: rquest::header::HeaderMap,
    ) -> RequestBuilder {
        build_request_builder_from_plan(&self.http, self.timeout, plan, headers)
    }
}
