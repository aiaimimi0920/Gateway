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
        if let Some(legacy_route) = gemini_web_reverse_modular::legacy_mixed_lane_execution_route(
            payload,
            req.endpoint_kind,
            false,
        ) {
            if legacy_route.kind == gemini_web_reverse_modular::LegacyMixedLaneExecutionKind::Media
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
        if payload.adapter == "gemini_canvas_web_reverse_compatible" {
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
            return match aistudio_web_reverse_modular::json_passthrough_route(req.endpoint_kind) {
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
        let headers = build_upstream_headers_with(payload, extra_headers);
        let provider = &payload.adapter;

        debug!(url = %plan.url, model, "sending upstream passthrough request");

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

        response
            .json()
            .await
            .map_err(|e| classify_network_error(&e, Some(provider)))
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
        if aistudio_web_reverse_modular::is_aistudio_web_reverse_adapter(&payload.adapter)
            && aistudio_web_reverse_modular::supports_binary_passthrough_endpoint(req.endpoint_kind)
        {
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
        let headers = build_upstream_headers_with(payload, extra_headers);
        let provider = &payload.adapter;

        debug!(url = %plan.url, model, "sending upstream binary passthrough request");

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
