use super::*;

impl UpstreamClient {
    pub(super) async fn execute_gemini_canvas_modular_browser_relay_text(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        _extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<CanonicalRelayResponse, GatewayError> {
        let request_timeout = self.timeout.max(Duration::from_secs(300));
        let locale = gemini_canvas::locale_from_payload(payload);
        let provider = payload.adapter.as_str();
        let prompt = gemini_canvas::prompt_for_text_request(
            req,
            "Gemini Canvas program relay text requests require a prompt.",
            "missing_gemini_canvas_program_text_prompt",
        )?;
        let model = gemini_canvas::resolve_text_model(model)?;
        let program_owned = payload.adapter == "gemini_canvas_program_web_reverse_compatible";
        let effective_payload = if program_owned {
            self.ensure_gemini_canvas_program_payload_handle(
                payload,
                "text",
                &locale,
                request_timeout,
            )
            .await?
        } else {
            payload.clone()
        };
        let runtime = gemini_canvas::runtime_from_payload(&effective_payload)?;
        if program_owned {
            let canonical = self
                .execute_gemini_canvas_direct_http_stream_generate_text(
                    &effective_payload,
                    req,
                    model,
                    &runtime,
                    &prompt,
                    request_timeout,
                )
                .await?;
            if gemini_canvas_text_response_is_generic_welcome(&prompt, &canonical.text) {
                return Err(gemini_canvas_generic_welcome_response_error(provider));
            }
            return Ok(canonical);
        }
        let browser_runtime_state_object_key =
            resolved_gemini_canvas_browser_runtime_state_object_key(&effective_payload, &runtime);
        let browser_cookie_header = gemini_canvas::browser_cookie_header(&effective_payload);
        let request_body = gemini_canvas::build_text_request_body(req, model);
        let request_url = gemini_canvas::build_text_fetch_url(&runtime, model);
        let browser_pool_base_url = self.ensure_gemini_canvas_browser_pool(provider).await?;
        let body = self
            .execute_gemini_canvas_connected_fetch_json_with_mode(
                provider,
                &browser_pool_base_url,
                payload.base_url.trim_end_matches('/'),
                &runtime.share_id,
                &browser_runtime_state_object_key,
                gemini_canvas::browser_cdp_url(payload).as_deref(),
                browser_cookie_header.as_deref(),
                &request_url,
                &request_body,
                "canvas_proxy",
                request_timeout,
            )
            .await?;
        let canonical = gemini_api_modular::parse_generate_content_response(&body, model)?;
        if gemini_canvas_text_response_is_generic_welcome(&prompt, &canonical.text) {
            return Err(gemini_canvas_generic_welcome_response_error(provider));
        }
        Ok(canonical)
    }

    pub(super) async fn execute_gemini_canvas_modular_browser_relay_text_stream(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        _extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<
        std::pin::Pin<Box<dyn futures::Stream<Item = Result<bytes::Bytes, rquest::Error>> + Send>>,
        GatewayError,
    > {
        let canonical = self
            .execute_gemini_canvas_modular_browser_relay_text(payload, req, model, None)
            .await?;
        let sse_bytes = canonical_response_to_openai_sse_bytes(req, model, &canonical)
            .into_iter()
            .map(Ok);
        Ok(Box::pin(futures::stream::iter(sse_bytes)))
    }

    pub(super) async fn execute_gemini_canvas_modular_browser_relay_tts(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        _extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<BinaryUpstreamResponse, GatewayError> {
        let locale = gemini_canvas::locale_from_payload(payload);
        let request_timeout = self.timeout.max(Duration::from_secs(300));
        let provider = payload.adapter.as_str();
        let prompt = gemini_canvas::prompt_for_text_request(
            req,
            "Gemini Canvas browser relay TTS requests require a prompt.",
            "missing_gemini_canvas_modular_tts_prompt",
        )?;
        let program_owned = payload.adapter == "gemini_canvas_program_web_reverse_compatible";
        let effective_payload = if program_owned {
            self.ensure_gemini_canvas_program_payload_handle(
                payload,
                "tts",
                &locale,
                request_timeout,
            )
            .await?
        } else {
            payload.clone()
        };
        let runtime = gemini_canvas::runtime_from_payload(&effective_payload)?;
        let locale = gemini_canvas::locale_from_payload(payload);
        let program_config = if effective_payload.adapter
            == "gemini_canvas_program_web_reverse_compatible"
        {
            ensure_gemini_canvas_program_payload_avoids_official_api_identity(&effective_payload)?;
            None
        } else {
            None
        };
        if program_owned {
            if gemini_canvas::pure_http_enabled(&effective_payload) {
                return self
                    .execute_gemini_canvas_direct_http_tts(
                        &effective_payload,
                        req,
                        model,
                        &runtime,
                        request_timeout,
                    )
                    .await;
            }
            return Err(gemini_canvas_program_tts_pure_http_required_error(provider));
        }
        let result = self
            .execute_gemini_canvas_owned_browser_invocation(
                &effective_payload,
                provider,
                &self.ensure_gemini_canvas_browser_pool(provider).await?,
                effective_payload.base_url.trim_end_matches('/'),
                &runtime,
                program_config.as_ref(),
                "tts",
                &prompt,
                &locale,
                request_timeout,
            )
            .await?;

        let audio =
            gemini_canvas_web_reverse_modular::decode_modular_tts_audio_payload(&result, provider)?;

        Ok(BinaryUpstreamResponse {
            body: bytes::Bytes::from(audio.bytes),
            content_type: Some(audio.mime_type),
            extra_headers: Vec::new(),
        })
    }

    pub(super) async fn execute_gemini_canvas_program_pure_http_image(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        prompt: &str,
        timeout: Duration,
    ) -> Result<Value, GatewayError> {
        let provider = "gemini_canvas_program_web_reverse_compatible";
        let runtime = gemini_canvas::runtime_from_payload(payload)?;
        ensure_gemini_canvas_program_payload_avoids_official_api_identity(payload)?;
        if gemini_canvas::pure_http_enabled(payload) {
            return self
                .execute_gemini_canvas_direct_http_image(
                    payload,
                    req,
                    model,
                    &runtime,
                    prompt.to_string(),
                    timeout,
                )
                .await;
        }
        Err(gemini_canvas_program_image_pure_http_required_error(
            provider,
        ))
    }

    pub(super) async fn execute_gemini_canvas_modular_browser_relay_media(
        &self,
        provider_account_id: &str,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        _extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        let locale = gemini_canvas::locale_from_payload(payload);
        let provider = payload.adapter.as_str();
        let bootstrap_operation = match req.endpoint_kind {
            EndpointKind::ImagesGenerations => "image",
            EndpointKind::MusicGenerations => "music",
            EndpointKind::VideosGenerations => "video",
            _ => "image",
        };
        let bootstrap_timeout = match req.endpoint_kind {
            EndpointKind::ImagesGenerations => self.timeout.max(Duration::from_secs(240)),
            EndpointKind::MusicGenerations => self.timeout.max(Duration::from_secs(480)),
            EndpointKind::VideosGenerations => self.timeout.max(Duration::from_secs(720)),
            _ => self.timeout.max(Duration::from_secs(240)),
        };
        let program_owned = payload.adapter == "gemini_canvas_program_web_reverse_compatible";

        match req.endpoint_kind {
            EndpointKind::ImagesGenerations => {
                let _ = gemini_canvas::resolve_image_model(model)?;
                if gemini_canvas::requested_output_count(req) > 1 {
                    return Err(gemini_canvas::unsupported_modular_image_count_error(
                        provider,
                    ));
                }
                let prompt =
                    gemini_web_reverse_modular::prompt_for_legacy_mixed_lane_media_request(
                        req,
                        gemini_canvas::GeminiCanvasMediaOperation::Image,
                    )?;
                let request_timeout = self.timeout.max(Duration::from_secs(240));
                if program_owned {
                    if let Some(body) = self
                        .maybe_execute_gemini_canvas_program_modular_media_direct_http(
                            payload,
                            req,
                            model,
                            &prompt,
                            request_timeout,
                            gemini_canvas::GeminiCanvasMediaOperation::Image,
                        )
                        .await?
                    {
                        return Ok(body);
                    }
                }
                let effective_payload = if program_owned {
                    self.ensure_gemini_canvas_program_payload_handle(
                        payload,
                        bootstrap_operation,
                        &locale,
                        bootstrap_timeout,
                    )
                    .await?
                } else {
                    payload.clone()
                };
                let runtime = gemini_canvas::runtime_from_payload(&effective_payload)?;
                let base_url = effective_payload.base_url.trim_end_matches('/');
                let program_config = if program_owned {
                    Some(
                        gemini_canvas_program_web_reverse_modular::relay_config_from_payload(
                            &effective_payload,
                        )?,
                    )
                } else {
                    None
                };
                let result = self
                    .execute_gemini_canvas_modular_media_browser_result(
                        provider_account_id,
                        &effective_payload,
                        provider,
                        req.endpoint_kind,
                        base_url,
                        &runtime,
                        program_config.as_ref(),
                        "image",
                        &prompt,
                        &locale,
                        request_timeout,
                    )
                    .await?;

                gemini_canvas_web_reverse_modular::build_image_generation_response_from_invocation(
                    &self.http,
                    provider,
                    req,
                    &prompt,
                    &result,
                    request_timeout,
                )
                .await
            }
            EndpointKind::ImagesEdits => Err(gemini_canvas::unsupported_modular_image_edits_error(
                provider,
            )),
            EndpointKind::MusicGenerations => {
                let _ = gemini_canvas::resolve_music_model(model)?;
                let prompt =
                    gemini_web_reverse_modular::prompt_for_legacy_mixed_lane_media_request(
                        req,
                        gemini_canvas::GeminiCanvasMediaOperation::Music,
                    )?;
                let request_timeout = self.timeout.max(Duration::from_secs(480));
                let effective_payload = if program_owned {
                    self.ensure_gemini_canvas_program_payload_handle(
                        payload,
                        bootstrap_operation,
                        &locale,
                        bootstrap_timeout,
                    )
                    .await?
                } else {
                    payload.clone()
                };
                let runtime = gemini_canvas::runtime_from_payload(&effective_payload)?;
                let base_url = effective_payload.base_url.trim_end_matches('/');
                let program_config = if program_owned {
                    Some(
                        gemini_canvas_program_web_reverse_modular::relay_config_from_payload(
                            &effective_payload,
                        )?,
                    )
                } else {
                    None
                };
                if program_owned {
                    if let Some(body) = self
                        .maybe_execute_gemini_canvas_program_modular_media_direct_http(
                            &effective_payload,
                            req,
                            model,
                            &prompt,
                            request_timeout,
                            gemini_canvas::GeminiCanvasMediaOperation::Music,
                        )
                        .await?
                    {
                        return Ok(body);
                    }
                }
                let result = self
                    .execute_gemini_canvas_modular_media_browser_result(
                        provider_account_id,
                        &effective_payload,
                        provider,
                        req.endpoint_kind,
                        base_url,
                        &runtime,
                        program_config.as_ref(),
                        "music",
                        &prompt,
                        &locale,
                        request_timeout,
                    )
                    .await?;

                gemini_canvas_web_reverse_modular::build_music_generation_response_from_invocation(
                    model, &prompt, &result, provider,
                )
            }
            EndpointKind::VideosGenerations => {
                let _ = gemini_canvas::resolve_video_model(model)?;
                if gemini_canvas::requested_output_count(req) > 1 {
                    return Err(gemini_canvas_modular_video_unsupported_count_error(
                        provider,
                    ));
                }

                let prompt =
                    gemini_web_reverse_modular::prompt_for_legacy_mixed_lane_media_request(
                        req,
                        gemini_canvas::GeminiCanvasMediaOperation::Video,
                    )?;
                let request_timeout = self.timeout.max(Duration::from_secs(720));
                let direct_http_attempt_timeout = request_timeout.min(Duration::from_secs(300));
                let program_direct_http_payload = if program_owned {
                    Some(
                        self.ensure_gemini_canvas_program_payload_handle(
                            payload,
                            bootstrap_operation,
                            &locale,
                            bootstrap_timeout,
                        )
                        .await?,
                    )
                } else {
                    None
                };
                if program_owned {
                    if let Some(body) = self
                        .maybe_execute_gemini_canvas_program_modular_media_direct_http(
                            program_direct_http_payload.as_ref().unwrap_or(payload),
                            req,
                            model,
                            &prompt,
                            direct_http_attempt_timeout,
                            gemini_canvas::GeminiCanvasMediaOperation::Video,
                        )
                        .await?
                    {
                        return Ok(body);
                    }
                    return Err(
                        gemini_canvas_program_video_browser_fallback_forbidden_error(provider),
                    );
                }
                let effective_payload = if let Some(ensured_payload) = program_direct_http_payload {
                    ensured_payload
                } else {
                    payload.clone()
                };
                let runtime = gemini_canvas::runtime_from_payload(&effective_payload)?;
                let base_url = effective_payload.base_url.trim_end_matches('/');
                let program_config = if program_owned {
                    Some(
                        gemini_canvas_program_web_reverse_modular::relay_config_from_payload(
                            &effective_payload,
                        )?,
                    )
                } else {
                    None
                };
                let result = self
                    .execute_gemini_canvas_modular_media_browser_result(
                        provider_account_id,
                        &effective_payload,
                        provider,
                        req.endpoint_kind,
                        base_url,
                        &runtime,
                        program_config.as_ref(),
                        "video",
                        &prompt,
                        &locale,
                        request_timeout,
                    )
                    .await?;

                gemini_canvas_web_reverse_modular::build_video_generation_response_from_invocation(
                    model, &prompt, &result, provider,
                )
            }
            _ => Err(gemini_canvas::unsupported_modular_endpoint_error(provider)),
        }
    }
}
