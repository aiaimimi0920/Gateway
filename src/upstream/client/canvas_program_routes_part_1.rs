use super::*;

impl UpstreamClient {
    pub(super) async fn maybe_execute_gemini_canvas_program_modular_media_direct_http(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        prompt: &str,
        request_timeout: Duration,
        operation: gemini_canvas::GeminiCanvasMediaOperation,
    ) -> Result<Option<Value>, GatewayError> {
        if !should_attempt_gemini_canvas_program_modular_media_direct_http(payload, operation) {
            return Ok(None);
        }

        let provider = payload.adapter.as_str();
        let result = match operation {
            gemini_canvas::GeminiCanvasMediaOperation::Image => {
                self.execute_gemini_canvas_program_pure_http_image(
                    payload,
                    req,
                    model,
                    prompt,
                    request_timeout,
                )
                .await
            }
            gemini_canvas::GeminiCanvasMediaOperation::Music
            | gemini_canvas::GeminiCanvasMediaOperation::Video => {
                self.execute_gemini_canvas_media_direct_http(
                    payload,
                    req,
                    model,
                    operation,
                    prompt.to_string(),
                    request_timeout,
                )
                .await
            }
        };

        match result {
            Ok(body) => Ok(Some(body)),
            Err(error) => {
                if should_treat_gemini_canvas_program_modular_media_direct_http_as_authoritative(
                    payload,
                ) {
                    return Err(error);
                }
                let operation_label = match operation {
                    gemini_canvas::GeminiCanvasMediaOperation::Image => "image",
                    gemini_canvas::GeminiCanvasMediaOperation::Music => "music",
                    gemini_canvas::GeminiCanvasMediaOperation::Video => "video",
                };
                debug!(
                    provider,
                    error = %summarize_gateway_error(&error),
                    "gemini canvas program modular pure HTTP {} lane failed; falling back to current program-owned browser execution",
                    operation_label,
                );
                Ok(None)
            }
        }
    }

    pub(crate) async fn execute_gemini_canvas_tts(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        _extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<BinaryUpstreamResponse, GatewayError> {
        let request_timeout = self.timeout.max(Duration::from_secs(300));
        let runtime = gemini_canvas::runtime_from_payload(payload)?;
        if payload.adapter == "gemini_canvas_program_web_reverse_compatible" {
            ensure_gemini_canvas_program_payload_avoids_official_api_identity(payload)?;
            if gemini_canvas::pure_http_enabled(payload) {
                return self
                    .execute_gemini_canvas_direct_http_tts(
                        payload,
                        req,
                        model,
                        &runtime,
                        request_timeout,
                    )
                    .await;
            }
            return Err(gemini_canvas_program_tts_pure_http_required_error(
                payload.adapter.as_str(),
            ));
        }
        if gemini_canvas::pure_http_enabled(payload) {
            return self
                .execute_gemini_canvas_direct_http_tts(
                    payload,
                    req,
                    model,
                    &runtime,
                    request_timeout,
                )
                .await;
        }

        self.execute_gemini_canvas_browser_backed_tts(payload, req, model, None)
            .await
    }

    pub(crate) async fn execute_gemini_canvas_browser_backed_tts(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        _model: &str,
        _extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<BinaryUpstreamResponse, GatewayError> {
        let request_timeout = self.timeout.max(Duration::from_secs(300));
        let runtime = gemini_canvas::runtime_from_payload(payload)?;
        let browser_runtime_state_object_key =
            resolved_gemini_canvas_browser_runtime_state_object_key(payload, &runtime);
        let browser_cdp_url = gemini_canvas::browser_cdp_url(payload);
        let browser_cookie_header = gemini_canvas::browser_cookie_header(payload);
        let provider = "gemini_canvas_compatible";
        let locale = gemini_canvas::locale_from_payload(payload);
        let prompt = gemini_canvas::prompt_for_text_request(
            req,
            "Gemini Canvas TTS requests require a prompt.",
            "missing_gemini_canvas_tts_prompt",
        )?;
        let browser_pool_base_url = self.ensure_gemini_canvas_browser_pool(provider).await?;
        let result = self
            .execute_gemini_canvas_browser_request_with_recovery(
                provider,
                &browser_pool_base_url,
                payload.base_url.trim_end_matches('/'),
                &runtime.share_id,
                &browser_runtime_state_object_key,
                browser_cdp_url.as_deref(),
                browser_cookie_header.as_deref(),
                "tts",
                &prompt,
                &locale,
                request_timeout,
            )
            .await?;

        let audio =
            gemini_canvas_web_reverse_modular::decode_browser_tts_audio_payload(&result, provider)?;
        let (body, content_type) = gemini_canvas::build_audio_binary_response(req, &audio)?;
        Ok(BinaryUpstreamResponse {
            body: bytes::Bytes::from(body),
            content_type: Some(content_type),
            extra_headers: Vec::new(),
        })
    }

    pub(crate) async fn execute_gemini_canvas_direct_http_tts(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        timeout: Duration,
    ) -> Result<BinaryUpstreamResponse, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let resolved_model = gemini_canvas::resolve_text_model(model)?;
        let prompt = gemini_canvas::prompt_for_text_request(
            req,
            "Gemini Canvas TTS requests require a prompt.",
            "missing_gemini_canvas_tts_prompt",
        )?;
        let (session, bootstrap, batchexecute_header_id) = self
            .prepare_gemini_canvas_direct_http_text_context(
                payload,
                resolved_model,
                runtime,
                timeout,
            )
            .await?;
        let stream_response = self
            .execute_gemini_canvas_direct_http_stream_generate_response_with_text_context(
                payload,
                resolved_model,
                &prompt,
                &session,
                &bootstrap,
                timeout,
            )
            .await?;
        let stream_status = stream_response.status().as_u16();
        let stream_content_type = stream_response
            .headers()
            .get(rquest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let mut stream = stream_response.bytes_stream();
        let mut stream_body = String::new();
        let mut locator: Option<gemini_canvas::GeminiCanvasStreamGenerateLocator> = None;
        let mut trigger_body: Option<String> = None;
        let mut followup_body: Option<String> = None;
        let mut export_body: Option<String> = None;

        while let Some(chunk_result) = stream.next().await {
            let chunk =
                chunk_result.map_err(|error| classify_network_error(&error, Some(provider)))?;
            stream_body.push_str(&String::from_utf8_lossy(&chunk));
            if locator.is_some() {
                continue;
            }
            let Ok(candidate_locator) =
                gemini_canvas::extract_stream_generate_locator(&stream_body)
            else {
                continue;
            };
            let (candidate_trigger_body, candidate_followup_body) = self
                .execute_gemini_canvas_direct_http_tts_followups(
                    payload,
                    resolved_model,
                    &session,
                    &bootstrap,
                    batchexecute_header_id.as_deref(),
                    &candidate_locator,
                    timeout,
                )
                .await?;
            locator = Some(candidate_locator);
            trigger_body = Some(candidate_trigger_body);
            followup_body = Some(candidate_followup_body);
        }

        if locator.is_none()
            && (gemini_web::response_indicates_browser_challenge(
                stream_status,
                stream_content_type.as_deref(),
                &stream_body,
            ) || gemini_web::response_indicates_session_invalid(
                stream_status,
                stream_content_type.as_deref(),
                &stream_body,
            ))
        {
            return Err(classify_gemini_canvas_pure_http_error(
                stream_status,
                stream_content_type.as_deref(),
                &stream_body,
            ));
        }

        let locator = if let Some(locator) = locator {
            locator
        } else {
            let locator = gemini_canvas::extract_stream_generate_locator(&stream_body)?;
            let (candidate_trigger_body, candidate_followup_body) = self
                .execute_gemini_canvas_direct_http_tts_followups(
                    payload,
                    resolved_model,
                    &session,
                    &bootstrap,
                    batchexecute_header_id.as_deref(),
                    &locator,
                    timeout,
                )
                .await?;
            trigger_body = Some(candidate_trigger_body);
            followup_body = Some(candidate_followup_body);
            locator
        };
        let trigger_body = trigger_body.unwrap_or_default();
        let followup_body = followup_body.unwrap_or_default();
        if let Some(stream_text) =
            gemini_web::accumulate_gemini_web_response(&stream_body, resolved_model)
                .ok()
                .map(|response| response.text.trim().to_string())
                .filter(|text| !text.is_empty())
        {
            export_body = Some(
                self.execute_gemini_canvas_direct_http_tts_export(
                    payload,
                    resolved_model,
                    &session,
                    &bootstrap,
                    batchexecute_header_id.as_deref(),
                    &locator.app_path,
                    &stream_text,
                    timeout,
                )
                .await?,
            );
        }
        let export_body = export_body.unwrap_or_default();

        if let Some(audio_response) =
            gemini_web_reverse_modular::resolve_direct_http_tts_audio_response(
                self,
                payload,
                req,
                &session,
                timeout,
                &stream_body,
                &trigger_body,
                &followup_body,
                &export_body,
            )
            .await?
        {
            return Ok(audio_response);
        }

        Err(
            gemini_web_reverse_modular::gemini_canvas_tts_direct_http_audio_unavailable_from_bodies(
                &locator.app_path,
                &stream_body,
                &trigger_body,
                &followup_body,
                &export_body,
            ),
        )
    }

    pub(super) async fn execute_gemini_canvas_direct_http_tts_followups(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        batchexecute_header_id: Option<&str>,
        locator: &gemini_canvas::GeminiCanvasStreamGenerateLocator,
        timeout: Duration,
    ) -> Result<(String, String), GatewayError> {
        gemini_web_reverse_modular::execute_direct_http_tts_followups(
            self,
            payload,
            model,
            session,
            bootstrap,
            batchexecute_header_id,
            locator,
            timeout,
        )
        .await
    }

    pub(super) async fn execute_gemini_canvas_direct_http_tts_export(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        batchexecute_header_id: Option<&str>,
        source_path: &str,
        response_text: &str,
        timeout: Duration,
    ) -> Result<String, GatewayError> {
        gemini_web_reverse_modular::execute_direct_http_tts_export(
            self,
            payload,
            model,
            session,
            bootstrap,
            batchexecute_header_id,
            source_path,
            response_text,
            timeout,
        )
        .await
    }
}
