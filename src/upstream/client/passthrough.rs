use super::*;

impl UpstreamClient {
    pub async fn execute_json_passthrough(
        &self,
        provider_account_id: &str,
        execution_mode: ProviderExecutionMode,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        crate::protocol::lobsterai::management_boundary::reject_generic(payload)?;
        let flow = crate::protocol::call_flow::prepare(payload)?;
        let payload = flow.payload.as_ref();
        crate::protocol::bedrock_turn::validate_request(payload, req)?;
        if crate::protocol::commandcode::owns_payload(payload) {
            return crate::protocol::commandcode::native_json::execute(self, payload, req, model)
                .await;
        }
        if crate::protocol::sensenova::native_selected(payload, req.endpoint_kind) {
            return Box::pin(crate::protocol::sensenova::execute_native(
                self, payload, req, model,
            ))
            .await;
        }
        if crate::protocol::sap_aicore::native_responses::selected(payload, req.endpoint_kind) {
            return Box::pin(crate::protocol::sap_aicore::native_responses::execute(
                self, payload, req, model,
            ))
            .await;
        }
        if crate::protocol::sap_aicore::orchestration::native_selected(payload, req.endpoint_kind) {
            return Box::pin(crate::protocol::sap_aicore::orchestration::execute_native(
                self, payload, req, model,
            ))
            .await;
        }
        if flow.executor == crate::protocol::luma_agents::ADAPTER {
            return crate::upstream::luma_agents::execute(
                &self.http,
                self.timeout,
                payload,
                req,
                model,
            )
            .await;
        }
        self.execute_prepared_json_passthrough(
            provider_account_id,
            execution_mode,
            payload,
            &flow.executor,
            req,
            model,
            extra_headers,
        )
        .await
    }

    // Construct the other media state machines outside the async dispatcher's poll frame.
    // Otherwise even an early Luma return reserves their large stack temporaries.
    // The returned future stays owned by the caller; cancellation never detaches I/O.
    fn execute_prepared_json_passthrough<'a>(
        &'a self,
        provider_account_id: &'a str,
        execution_mode: ProviderExecutionMode,
        payload: &'a ProviderAccountPayload,
        executor: &'a str,
        req: &'a CanonicalRelayRequest,
        model: &'a str,
        extra_headers: Option<&'a std::collections::HashMap<String, String>>,
    ) -> std::pin::Pin<
        Box<impl std::future::Future<Output = Result<Value, GatewayError>> + Send + 'a>,
    > {
        Box::pin(async move {
            crate::protocol::vertex_express::validate_request(payload, req, model)?;
            crate::protocol::vertex_oauth::business::validate_request(payload, req, model)?;
            if executor == crate::protocol::perplexity_web::material::ADAPTER {
                return Err(crate::upstream::perplexity_web::unsupported_request_plan_error());
            }
            if payload.adapter == "flow_web_compatible" {
                return Err(crate::upstream::flow_web::generation_unavailable());
            }
            if executor == crate::protocol::chatgpt::basispoints::ADAPTER {
                return crate::upstream::basispoints::exchange(
                    &self.http,
                    self.timeout,
                    payload,
                    req,
                    model,
                )
                .await;
            }
            if payload.adapter == "gemini_business_compatible" {
                return self
                    .execute_gemini_business_images(payload, req, model, extra_headers)
                    .await;
            }
            if payload.adapter == "chataibot_compatible" {
                return self
                    .execute_chataibot_images(payload, req, model, extra_headers)
                    .await;
            }
            if payload.adapter == "lumalabs_compatible" {
                return self
                    .execute_lumalabs_media(provider_account_id, payload, req, model, extra_headers)
                    .await;
            }
            if payload.adapter == "gemini_canvas_compatible" {
                return self
                    .execute_gemini_canvas_media(
                        provider_account_id,
                        payload,
                        req,
                        model,
                        extra_headers,
                    )
                    .await;
            }
            if let Some(legacy_route) =
                gemini_web_reverse_modular::legacy_mixed_lane_execution_route(
                    payload,
                    req.endpoint_kind,
                    false,
                )
            {
                if legacy_route.kind
                    == gemini_web_reverse_modular::LegacyMixedLaneExecutionKind::Media
                {
                    return gemini_web_reverse_modular::execute_legacy_media(
                        self,
                        provider_account_id,
                        &legacy_route.payload,
                        req,
                        model,
                        extra_headers,
                    )
                    .await;
                }
            }
            if matches!(
                payload.adapter.as_str(),
                "gemini_web_compatible" | "gemini_web_reverse_modular_compatible"
            ) && req.endpoint_kind == EndpointKind::AudioTranscriptions
            {
                let text = gemini_web_reverse_modular::execute_transcription(
                    &self.http,
                    self.timeout,
                    payload,
                    req,
                    model,
                    extra_headers,
                )
                .await?;
                return Ok(json!({"text":text}));
            }
            if payload.adapter == "gemini_canvas_web_reverse_compatible" {
                if req.endpoint_kind == EndpointKind::AudioTranscriptions {
                    let text = self
                        .execute_gemini_canvas_host_transcription(payload, req, model)
                        .await?;
                    return Ok(json!({"text":text}));
                }
                return self
                    .execute_gemini_canvas_modular_browser_relay_media(
                        provider_account_id,
                        payload,
                        req,
                        model,
                        extra_headers,
                    )
                    .await;
            }
            if gemini_api_modular::is_official_adapter(payload.adapter.as_str())
                && gemini_api_modular::supports_media_endpoint(req.endpoint_kind)
                && req.endpoint_kind != EndpointKind::AudioSpeech
            {
                return gemini_api_modular::execute_official_media(
                    &self.http,
                    self.timeout,
                    payload,
                    req,
                    model,
                    extra_headers,
                )
                .await;
            }
            if payload.adapter == "gemini_canvas_program_web_reverse_compatible" {
                let program_owned_payload =
                    gemini_canvas_program_web_reverse_modular::force_program_owned_payload(payload);
                return self
                    .execute_gemini_canvas_modular_browser_relay_media(
                        provider_account_id,
                        &program_owned_payload,
                        req,
                        model,
                        extra_headers,
                    )
                    .await;
            }
            if aistudio_web_reverse_modular::is_aistudio_web_reverse_adapter(&payload.adapter) {
                return match aistudio_web_reverse_modular::json_passthrough_route(req.endpoint_kind)
                {
                    Ok(aistudio_web_reverse_modular::JsonPassthroughRoute::Embeddings) => {
                        self.execute_aistudio_web_embeddings(
                            provider_account_id,
                            payload,
                            req,
                            model,
                            extra_headers,
                        )
                        .await
                    }
                    Ok(aistudio_web_reverse_modular::JsonPassthroughRoute::ImagesGenerations) => {
                        self.execute_aistudio_web_images(
                            provider_account_id,
                            payload,
                            req,
                            model,
                            extra_headers,
                        )
                        .await
                    }
                    Ok(aistudio_web_reverse_modular::JsonPassthroughRoute::AudioTranscriptions) => {
                        let text = self
                            .execute_aistudio_http_transcription(payload, req, model)
                            .await?;
                        Ok(json!({"text":text}))
                    }
                    Err(err) => Err(err),
                };
            }
            if payload.adapter == "producer_compatible" {
                return if execution_mode == ProviderExecutionMode::BrowserBacked {
                    self.execute_producer_browser_backed(
                        provider_account_id,
                        payload,
                        req,
                        model,
                        extra_headers,
                    )
                    .await
                } else {
                    self.execute_producer_media(payload, req, model, extra_headers)
                        .await
                };
            }
            if payload.adapter == "suno_compatible" {
                return if execution_mode == ProviderExecutionMode::BrowserBacked {
                    self.execute_suno_browser_backed(
                        provider_account_id,
                        payload,
                        req,
                        model,
                        extra_headers,
                    )
                    .await
                } else {
                    self.execute_suno_media(payload, req, model, extra_headers)
                        .await
                };
            }
            if payload.adapter == "udio_compatible" {
                return self
                    .execute_udio_media(provider_account_id, payload, req, model, extra_headers)
                    .await;
            }

            let plan = Self::build_request_plan(payload, req, model, false)?;
            let headers = build_inference_headers(payload, req, model, extra_headers);
            let provider = &payload.adapter;

            debug!(url = %plan.url, model, "sending upstream passthrough request");

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

            let body: Value = if crate::protocol::luma_inference::owns_payload(payload) {
                crate::protocol::luma_inference::read_json(response).await?
            } else if crate::protocol::minimax::owns_payload(payload) {
                crate::protocol::minimax::read_json(response).await?
            } else if payload.canonical_adapter() == "search_api_compatible"
                && req.endpoint_kind == EndpointKind::Fetch
            {
                crate::upstream::search_fetch_response::read_json(response, payload, req).await?
            } else {
                response
                    .json()
                    .await
                    .map_err(|e| classify_network_error(&e, Some(provider)))?
            };
            if matches!(
                payload.adapter.as_str(),
                "dashscope_compatible" | "dashscope_multimodal_compatible"
            ) {
                crate::protocol::dashscope::response::observe_native(&body, model)?;
            }
            crate::protocol::agnes::validate_json_response(payload, req, &body)?;
            crate::protocol::minimax::validate_json_response(payload, &body)?;
            Ok(body)
        })
    }

    pub async fn execute_binary_passthrough(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<BinaryUpstreamResponse, GatewayError> {
        self.execute_binary_passthrough_with_provider_account_id(
            "",
            payload,
            req,
            model,
            extra_headers,
        )
        .await
    }

    pub async fn execute_binary_passthrough_with_provider_account_id(
        &self,
        provider_account_id: &str,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<BinaryUpstreamResponse, GatewayError> {
        crate::protocol::lobsterai::management_boundary::reject_generic(payload)?;
        let flow = crate::protocol::call_flow::prepare(payload)?;
        let payload = flow.payload.as_ref();
        crate::protocol::bedrock_turn::validate_request(payload, req)?;
        crate::protocol::vertex_express::validate_request(payload, req, model)?;
        crate::protocol::vertex_oauth::business::validate_request(payload, req, model)?;
        if flow.executor == crate::protocol::perplexity_web::material::ADAPTER {
            return Err(crate::upstream::perplexity_web::unsupported_request_plan_error());
        }
        if gemini_api_modular::is_official_adapter(payload.adapter.as_str())
            && req.endpoint_kind == EndpointKind::AudioSpeech
        {
            return gemini_api_modular::execute_official_tts(
                &self.http,
                self.timeout,
                payload,
                req,
                model,
                extra_headers,
            )
            .await;
        }
        if payload.adapter == "gemini_canvas_compatible"
            && req.endpoint_kind == EndpointKind::AudioSpeech
        {
            return self
                .execute_gemini_canvas_tts(payload, req, model, extra_headers)
                .await;
        }
        if payload.adapter == "gemini_web_compatible"
            && req.endpoint_kind == EndpointKind::AudioSpeech
        {
            return gemini_web_reverse_modular::execute_speech(
                &self.http,
                self.timeout,
                payload,
                req,
                model,
                extra_headers,
            )
            .await;
        }
        if aistudio_web_reverse_modular::is_aistudio_web_reverse_adapter(&payload.adapter)
            && aistudio_web_reverse_modular::supports_binary_passthrough_endpoint(req.endpoint_kind)
        {
            if req.endpoint_kind == EndpointKind::AudioTranscriptions {
                let text = self
                    .execute_aistudio_http_transcription(payload, req, model)
                    .await?;
                return Ok(BinaryUpstreamResponse {
                    body: text.into_bytes().into(),
                    content_type: Some("text/plain; charset=utf-8".to_string()),
                    extra_headers: Vec::new(),
                });
            }
            return self
                .execute_aistudio_web_tts(provider_account_id, payload, req, model, extra_headers)
                .await;
        }
        if let Some(legacy_route) = gemini_web_reverse_modular::legacy_mixed_lane_execution_route(
            payload,
            req.endpoint_kind,
            false,
        ) {
            if legacy_route.kind == gemini_web_reverse_modular::LegacyMixedLaneExecutionKind::Tts {
                return gemini_web_reverse_modular::execute_legacy_tts(
                    self,
                    &legacy_route.payload,
                    req,
                    model,
                    extra_headers,
                )
                .await;
            }
        }
        if matches!(
            payload.adapter.as_str(),
            "gemini_web_compatible" | "gemini_web_reverse_modular_compatible"
        ) && req.endpoint_kind == EndpointKind::AudioTranscriptions
        {
            let text = gemini_web_reverse_modular::execute_transcription(
                &self.http,
                self.timeout,
                payload,
                req,
                model,
                extra_headers,
            )
            .await?;
            return Ok(BinaryUpstreamResponse {
                body: text.into_bytes().into(),
                content_type: Some("text/plain; charset=utf-8".into()),
                extra_headers: vec![],
            });
        }
        if payload.adapter == "gemini_canvas_web_reverse_compatible"
            && req.endpoint_kind == EndpointKind::AudioTranscriptions
        {
            let text = self
                .execute_gemini_canvas_host_transcription(payload, req, model)
                .await?;
            return Ok(BinaryUpstreamResponse {
                body: text.into_bytes().into(),
                content_type: Some("text/plain; charset=utf-8".into()),
                extra_headers: vec![],
            });
        }
        if payload.adapter == "gemini_canvas_web_reverse_compatible"
            && req.endpoint_kind == EndpointKind::AudioSpeech
        {
            return self
                .execute_gemini_canvas_modular_browser_relay_tts(payload, req, model, extra_headers)
                .await;
        }
        if payload.adapter == "gemini_canvas_program_web_reverse_compatible"
            && req.endpoint_kind == EndpointKind::AudioSpeech
        {
            let program_owned_payload =
                gemini_canvas_program_web_reverse_modular::force_program_owned_payload(payload);
            return self
                .execute_gemini_canvas_tts(&program_owned_payload, req, model, extra_headers)
                .await;
        }

        let plan = Self::build_request_plan(payload, req, model, false)?;
        let headers = build_inference_headers(payload, req, model, extra_headers);
        let provider = &payload.adapter;

        debug!(url = %plan.url, model, "sending upstream binary passthrough request");

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

        let content_type = response
            .headers()
            .get(rquest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let mut extra_headers_out = Vec::new();
        if let Some(content_disposition) = response
            .headers()
            .get(rquest::header::CONTENT_DISPOSITION)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string)
        {
            extra_headers_out.push(("content-disposition".to_string(), content_disposition));
        }

        let body = response
            .bytes()
            .await
            .map_err(|e| classify_network_error(&e, Some(provider)))?;

        Ok(BinaryUpstreamResponse {
            body,
            content_type,
            extra_headers: extra_headers_out,
        })
    }
}
