use super::*;

impl UpstreamClient {
    pub(super) async fn materialize_gemini_canvas_direct_http_images(
        &self,
        payload: &ProviderAccountPayload,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        assets: &[&gemini_canvas::GeminiCanvasMediaAsset],
        request_timeout: Duration,
        provider: &str,
    ) -> Result<Vec<gemini_canvas::GeminiCanvasImage>, GatewayError> {
        let mut downloaded = Vec::with_capacity(assets.len());
        for asset in assets {
            if let Some(image) =
                gemini_canvas_web_reverse_modular::decode_direct_http_inline_image_asset(
                    provider, asset,
                )?
            {
                downloaded.push(image);
                continue;
            }

            let image = self
                .fetch_gemini_canvas_direct_http_image_asset(
                    payload,
                    runtime,
                    asset,
                    request_timeout,
                )
                .await?;
            downloaded.push(image);
        }
        Ok(downloaded)
    }

    pub(super) async fn fetch_gemini_canvas_direct_http_image_asset(
        &self,
        payload: &ProviderAccountPayload,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        asset: &gemini_canvas::GeminiCanvasMediaAsset,
        timeout: Duration,
    ) -> Result<gemini_canvas::GeminiCanvasImage, GatewayError> {
        let fetched_asset = match self
            .materialize_gemini_canvas_direct_http_media_asset(
                payload,
                runtime,
                &asset.url,
                Some("image"),
                Some(&asset.mime_type),
                timeout,
            )
            .await
        {
            Ok(asset) => asset,
            Err(download_error) => {
                if payload.adapter == "gemini_canvas_program_web_reverse_compatible" {
                    return Err(download_error);
                }
                let browser_runtime_state_object_key =
                    gemini_canvas::browser_runtime_state_object_key_for_browser_operation(
                        payload, "image",
                    )
                    .unwrap_or_else(|| runtime.runtime_state_object_key.clone());
                let browser_cdp_url = gemini_canvas::browser_cdp_url(payload);
                let browser_cookie_header = gemini_canvas::browser_cookie_header(payload);
                let browser_pool_base_url = self
                    .ensure_gemini_canvas_browser_pool(payload.adapter.as_str())
                    .await?;
                let (bytes, response_content_type) =
                    gemini_canvas_web_reverse_modular::execute_connected_fetch_get_bytes(
                        &self.http,
                        timeout,
                        payload.adapter.as_str(),
                        &browser_pool_base_url,
                        payload.base_url.trim_end_matches('/'),
                        &runtime.share_id,
                        &browser_runtime_state_object_key,
                        browser_cdp_url.as_deref(),
                        browser_cookie_header.as_deref(),
                        &asset.url,
                    )
                    .await
                    .map_err(|browser_fetch_error| {
                        let mut browser_fetch_error = browser_fetch_error;
                        browser_fetch_error.message = sanitize_provider_error_message(&format!(
                            "{}; direct_http_download={}",
                            browser_fetch_error.message,
                            summarize_gateway_error(&download_error)
                        ));
                        browser_fetch_error
                    })?;
                return Ok(
                    gemini_canvas_web_reverse_modular::build_downloaded_image_from_bytes(
                        asset,
                        response_content_type.as_deref(),
                        &bytes,
                    ),
                );
            }
        };
        let body_base64 = fetched_asset
            .body_base64
            .as_deref()
            .ok_or_else(gemini_canvas_image_fetch_missing_inline_bytes_error)?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(body_base64)
            .map_err(|error| {
                gemini_canvas_image_fetch_invalid_inline_bytes_error(error.to_string().as_str())
            })?;
        let mime_type = if fetched_asset.mime_type.starts_with("image/") {
            fetched_asset.mime_type
        } else {
            sniff_image_mime_type_from_bytes(&bytes)
                .map(str::to_string)
                .unwrap_or_else(|| asset.mime_type.clone())
        };
        Ok(gemini_canvas::GeminiCanvasImage { mime_type, bytes })
    }

    pub(super) async fn execute_gemini_canvas_generate_content_json(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
    ) -> Result<Value, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let runtime = gemini_canvas::runtime_from_payload(payload)?;
        let browser_runtime_state_object_key =
            resolved_gemini_canvas_browser_runtime_state_object_key(payload, &runtime);
        let browser_cdp_url = gemini_canvas::browser_cdp_url(payload);
        let browser_cookie_header = gemini_canvas::browser_cookie_header(payload);
        let model = gemini_canvas::resolve_text_model(model)?;
        let request_timeout = self.timeout.max(Duration::from_secs(300));
        let prompt = gemini_canvas::prompt_for_text_request(
            req,
            "Gemini Canvas requests require a prompt.",
            "missing_gemini_canvas_text_prompt",
        )?;
        let mut direct_http_failure_summary: Option<String> = None;
        if gemini_canvas::pure_http_enabled(payload) {
            let primary_stream_error = match self
                .execute_gemini_canvas_direct_http_stream_generate_text(
                    payload,
                    req,
                    model,
                    &runtime,
                    &prompt,
                    request_timeout,
                )
                .await
            {
                Ok(canonical) => {
                    if !gemini_canvas_text_response_is_generic_welcome(&prompt, &canonical.text) {
                        return Ok(build_gemini_canvas_text_success_body(
                            req,
                            model,
                            &canonical.text,
                            canonical.usage.as_ref(),
                            &canonical.tool_calls,
                        ));
                    }
                    let error = gemini_canvas_generic_welcome_response_error(provider);
                    debug!(
                        provider,
                        "Gemini Canvas StreamGenerate returned a generic welcome; falling back to generateContent JSON direct HTTP"
                    );
                    error
                }
                Err(error) => {
                    let stream_summary = summarize_gateway_error(&error);
                    debug!(
                        provider,
                        error = %stream_summary,
                        "Gemini Canvas StreamGenerate direct HTTP failed; falling back to generateContent JSON direct HTTP"
                    );
                    error
                }
            };

            let request_body = gemini_canvas::build_text_request_body(req, model);
            let request_url = gemini_canvas::build_text_fetch_url(&runtime, model);
            let mut fallback_failures = Vec::new();
            let runtime_api = if payload.api_key.trim().is_empty() {
                match self
                    .prepare_gemini_canvas_runtime_api_payload(payload, &runtime, request_timeout)
                    .await
                {
                    Ok(context) => Some(context),
                    Err(error) => {
                        let summary = summarize_gateway_error(&error);
                        debug!(
                            provider,
                            error = %summary,
                            "Gemini Canvas generateContent text fallback could not prepare runtime API key candidates; retrying legacy direct HTTP request"
                        );
                        fallback_failures.push(format!("runtime_api_prepare={summary}"));
                        None
                    }
                }
            } else {
                None
            };
            let attempts = build_gemini_canvas_text_direct_http_fallback_attempts(
                payload,
                runtime_api.as_ref(),
            );
            let mut last_fallback_error: Option<GatewayError> = None;

            for attempt in attempts {
                let transports: Vec<GeminiCanvasDirectHttpApiKeyTransport> =
                    if attempt.api_key_override.is_some() {
                        gemini_canvas_direct_http_api_key_transports(&request_url).to_vec()
                    } else {
                        vec![GeminiCanvasDirectHttpApiKeyTransport::HeaderOnly]
                    };
                for transport in transports {
                    match self
                        .execute_gemini_canvas_direct_http_json_with_options(
                            payload,
                            &runtime,
                            &request_url,
                            &request_body,
                            request_timeout,
                            attempt.api_key_override,
                            transport,
                            None,
                            attempt.referer_override,
                            false,
                            attempt.preserve_cross_origin_referer,
                            attempt.include_signed_headers,
                        )
                        .await
                    {
                        Ok(body) => {
                            let canonical =
                                gemini_api_modular::parse_generate_content_response(&body, model)?;
                            if gemini_canvas_text_response_is_generic_welcome(
                                &prompt,
                                &canonical.text,
                            ) {
                                let error = gemini_canvas_generic_welcome_response_error(provider);
                                fallback_failures.push(format!(
                                    "{}[transport={}]={}",
                                    attempt.label,
                                    transport.label(),
                                    summarize_gateway_error(&error)
                                ));
                                last_fallback_error = Some(error);
                                continue;
                            }
                            return Ok(build_gemini_canvas_text_success_body(
                                req,
                                model,
                                &canonical.text,
                                canonical.usage.as_ref(),
                                &canonical.tool_calls,
                            ));
                        }
                        Err(error) => {
                            fallback_failures.push(format!(
                                "{}[transport={}]={}",
                                attempt.label,
                                transport.label(),
                                summarize_gateway_error(&error)
                            ));
                            last_fallback_error = Some(error);
                        }
                    }
                }
            }

            let mut fallback_error = last_fallback_error
                .expect("Gemini Canvas text direct HTTP fallback attempts should record an error");
            if !fallback_failures.is_empty() {
                fallback_error.message = sanitize_provider_error_message(&format!(
                    "{}; direct_http_fallback_attempts={}",
                    fallback_error.message,
                    fallback_failures.join(" | ")
                ));
            }
            fallback_error.message = sanitize_provider_error_message(&format!(
                "{}; StreamGenerate primary failure: {}",
                fallback_error.message,
                summarize_gateway_error(&primary_stream_error)
            ));
            let summary = summarize_gateway_error(&fallback_error);
            debug!(
                provider,
                error = %summary,
                "Gemini Canvas direct HTTP text fallback failed; retrying through browser-backed invocation"
            );
            direct_http_failure_summary = Some(summary);
        }

        let locale = gemini_canvas::locale_from_payload(payload);
        let browser_pool_base_url = self.ensure_gemini_canvas_browser_pool(provider).await?;
        let invocation = self
            .execute_gemini_canvas_browser_request_with_recovery(
                provider,
                &browser_pool_base_url,
                payload.base_url.trim_end_matches('/'),
                &runtime.share_id,
                &browser_runtime_state_object_key,
                browser_cdp_url.as_deref(),
                browser_cookie_header.as_deref(),
                "text",
                &prompt,
                &locale,
                request_timeout,
            )
            .await
            .map_err(|error| {
                if let Some(summary) = direct_http_failure_summary.as_deref() {
                    let mut error = error;
                    error.message = sanitize_provider_error_message(&format!(
                        "{}; direct_http_failure={summary}",
                        error.message
                    ));
                    return error;
                }
                error
            })?;
        let raw_text = gemini_canvas_web_reverse_modular::extract_text_or_body_text(&invocation);
        if gemini_canvas_text_response_is_generic_welcome(&prompt, &raw_text) {
            return Err(gemini_canvas_generic_welcome_response_error(provider));
        }
        Ok(build_gemini_canvas_text_success_body(
            req,
            model,
            &raw_text,
            None,
            &[],
        ))
    }
}
