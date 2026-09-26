use super::*;

impl UpstreamClient {
    pub(super) async fn execute_gemini_canvas_media_direct_http(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        operation: gemini_canvas::GeminiCanvasMediaOperation,
        prompt: String,
        timeout: Duration,
    ) -> Result<Value, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let runtime = gemini_canvas::runtime_from_payload(payload)?;
        if operation == gemini_canvas::GeminiCanvasMediaOperation::Video {
            if let Some(continuation) = gemini_canvas_video_continuation_from_request(req)? {
                return self
                    .execute_gemini_canvas_video_continuation(
                        payload,
                        model,
                        &runtime,
                        &prompt,
                        continuation,
                        timeout,
                    )
                    .await;
            }
        }
        if matches!(
            operation,
            gemini_canvas::GeminiCanvasMediaOperation::Music
                | gemini_canvas::GeminiCanvasMediaOperation::Video
        ) {
            if payload.adapter == "gemini_canvas_program_web_reverse_compatible" {
                let program_context = self
                    .prepare_gemini_canvas_program_app_endpoint_api_context(
                        payload, &runtime, timeout,
                    )
                    .await?;
                return self
                    .execute_gemini_canvas_program_app_endpoint_media_direct_http_with_context(
                        req,
                        model,
                        operation,
                        program_context,
                    )
                    .await;
            } else {
                match self
                    .execute_gemini_canvas_runtime_api_media_direct_http(
                        payload, req, model, operation, timeout,
                    )
                    .await
                {
                    Ok(body) => return Ok(body),
                    Err(error) => {
                        debug!(
                            provider,
                            operation = ?operation,
                            error = %summarize_gateway_error(&error),
                            "gemini canvas runtime api media lane failed; falling back to StreamGenerate direct HTTP"
                        );
                    }
                }
            }
        }
        if operation == gemini_canvas::GeminiCanvasMediaOperation::Image {
            let image_direct_http_timeout = timeout.min(Duration::from_secs(75));
            match self
                .execute_gemini_canvas_direct_http_image(
                    payload,
                    req,
                    model,
                    &runtime,
                    prompt.clone(),
                    image_direct_http_timeout,
                )
                .await
            {
                Ok(body) => return Ok(body),
                Err(error) if should_fallback_gemini_canvas_image_to_browser(&error) => {
                    debug!(
                        provider,
                        error = %summarize_gateway_error(&error),
                        "gemini canvas direct HTTP image lane failed; retrying through browser-backed invocation"
                    );
                    let browser_pool_base_url =
                        self.ensure_gemini_canvas_browser_pool(provider).await?;
                    let locale = gemini_canvas::locale_from_payload(payload);
                    let result = self
                        .execute_gemini_canvas_owned_browser_invocation(
                            payload,
                            provider,
                            &browser_pool_base_url,
                            payload.base_url.trim_end_matches('/'),
                            &runtime,
                            None,
                            "image",
                            &prompt,
                            &locale,
                            timeout,
                        )
                        .await?;
                    return gemini_canvas_web_reverse_modular::build_image_generation_response_from_invocation(
                        &self.http,
                        provider,
                        req,
                        &prompt,
                        &result,
                        timeout,
                    )
                    .await;
                }
                Err(error) => return Err(error),
            }
        }
        let mode_index = gemini_canvas::stream_generate_mode_index(operation);
        let request_started_at = SystemTime::now();
        let body_text = self
            .execute_gemini_canvas_direct_http_stream_generate_body(
                payload, model, &runtime, mode_index, &prompt, timeout, true, None, None,
            )
            .await?;

        match operation {
            gemini_canvas::GeminiCanvasMediaOperation::Music => {
                let followup_result = self
                    .extract_gemini_canvas_media_assets_with_followup(
                        payload,
                        model,
                        &runtime,
                        operation,
                        &prompt,
                        &body_text,
                        request_started_at,
                        timeout,
                        false,
                        None,
                    )
                    .await;
                let (assets, resolved_body_text) = match followup_result {
                    Ok(result) => result,
                    Err(error)
                        if gemini_canvas_music_body_indicates_accepted_progress(&body_text) =>
                    {
                        debug!(
                            provider,
                            error = %summarize_gateway_error(&error),
                            "gemini canvas direct HTTP music follow-up did not expose a final asset, but the primary StreamGenerate body already reached accepted progress"
                        );
                        return Ok(build_gemini_canvas_music_accepted_response_from_body(
                            model,
                            &prompt,
                            gemini_canvas::duration_seconds_from_request(req),
                            &body_text,
                            None,
                            None,
                            None,
                        ));
                    }
                    Err(error) => return Err(error),
                };
                if assets.is_empty()
                    && gemini_canvas_music_body_indicates_accepted_progress(&resolved_body_text)
                {
                    return Ok(build_gemini_canvas_music_accepted_response_from_body(
                        model,
                        &prompt,
                        gemini_canvas::duration_seconds_from_request(req),
                        &resolved_body_text,
                        None,
                        None,
                        None,
                    ));
                }
                let asset = select_preferred_gemini_canvas_music_asset(&assets)
                    .expect("assets is non-empty");
                let asset = if asset.body_base64.is_some() {
                    asset.clone()
                } else {
                    self.materialize_gemini_canvas_direct_http_media_asset(
                        payload,
                        &runtime,
                        &asset.url,
                        Some(asset.kind.as_str()),
                        Some(asset.mime_type.as_str()),
                        timeout,
                    )
                    .await?
                };
                Ok(gemini_canvas::build_music_generation_response(
                    model,
                    &prompt,
                    &asset,
                    Some(&resolved_body_text),
                ))
            }
            gemini_canvas::GeminiCanvasMediaOperation::Video => {
                let (assets, resolved_body_text) = match self
                    .extract_gemini_canvas_media_assets_with_followup(
                        payload,
                        model,
                        &runtime,
                        operation,
                        &prompt,
                        &body_text,
                        request_started_at,
                        timeout,
                        false,
                        None,
                    )
                    .await
                {
                    Ok(value) => value,
                    Err(error)
                        if gemini_canvas::response_indicates_video_generation_pending(
                            &body_text,
                        )
                            || gemini_canvas::response_indicates_video_generation_quota_reached(
                                &body_text,
                            ) =>
                    {
                        debug!(
                            provider,
                            error = %summarize_gateway_error(&error),
                            "gemini canvas direct HTTP video follow-up did not expose a final asset, but the primary StreamGenerate body already reached accepted progress"
                        );
                        return Ok(build_gemini_canvas_video_accepted_response_from_body(
                            model, &prompt, &body_text, None, None, None,
                        ));
                    }
                    Err(error) => return Err(error),
                };
                if assets.is_empty()
                    && (gemini_canvas::response_indicates_video_generation_pending(
                        &resolved_body_text,
                    ) || gemini_canvas::response_indicates_video_generation_quota_reached(
                        &resolved_body_text,
                    ))
                {
                    return Ok(build_gemini_canvas_video_accepted_response_from_body(
                        model,
                        &prompt,
                        &resolved_body_text,
                        None,
                        None,
                        None,
                    ));
                }
                let asset = assets.first().ok_or_else(|| {
                    build_gemini_canvas_direct_http_video_missing_asset_error(provider)
                })?;
                if gemini_canvas::video_body_indicates_music_modality_mismatch(&resolved_body_text)
                {
                    return Err(gemini_canvas_video_music_modality_mismatch_error(provider));
                }
                let asset = if asset.body_base64.is_some() {
                    asset.clone()
                } else {
                    self.materialize_gemini_canvas_direct_http_media_asset(
                        payload,
                        &runtime,
                        &asset.url,
                        Some(asset.kind.as_str()),
                        Some(asset.mime_type.as_str()),
                        timeout,
                    )
                    .await?
                };
                Ok(gemini_canvas::build_video_generation_response(
                    model,
                    &prompt,
                    &asset,
                    Some(&resolved_body_text),
                ))
            }
            gemini_canvas::GeminiCanvasMediaOperation::Image => unreachable!(),
        }
    }

    pub(super) async fn execute_gemini_canvas_runtime_api_media_direct_http(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        operation: gemini_canvas::GeminiCanvasMediaOperation,
        timeout: Duration,
    ) -> Result<Value, GatewayError> {
        let runtime = gemini_canvas::runtime_from_payload(payload)?;
        let runtime_api = self
            .prepare_gemini_canvas_runtime_api_payload(payload, &runtime, timeout)
            .await?;
        self.execute_gemini_canvas_official_media("", &runtime_api.payload, req, model, None)
            .await
            .map_err(|error| {
                debug!(
                    provider = "gemini_canvas_compatible",
                    operation = ?operation,
                    error = %summarize_gateway_error(&error),
                    "gemini canvas runtime api media lane returned an error"
                );
                error
            })
    }
}
