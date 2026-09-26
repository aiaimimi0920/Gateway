use super::*;

#[path = "canvas_completion_part_3/image_generation.rs"]
mod image_generation;
#[path = "canvas_completion_part_3/text.rs"]
mod text;

impl UpstreamClient {
    pub(crate) async fn execute_gemini_canvas_media(
        &self,
        provider_account_id: &str,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        _extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let runtime = gemini_canvas::runtime_from_payload(payload)?;
        let browser_runtime_state_object_key_for = |operation: &str| {
            gemini_canvas::browser_runtime_state_object_key_for_browser_operation(
                payload, operation,
            )
            .unwrap_or_else(|| runtime.runtime_state_object_key.clone())
        };
        let browser_cdp_url = gemini_canvas::browser_cdp_url(payload);
        let browser_cookie_header = gemini_canvas::browser_cookie_header(payload);
        let locale = gemini_canvas::locale_from_payload(payload);
        let base_url = payload.base_url.trim_end_matches('/');

        match req.endpoint_kind {
            EndpointKind::ImagesGenerations => {
                self.execute_gemini_canvas_image_generation(
                    provider_account_id,
                    payload,
                    req,
                    model,
                    &runtime,
                    &browser_runtime_state_object_key_for,
                    &browser_cdp_url,
                    &browser_cookie_header,
                    &locale,
                    base_url,
                )
                .await
            }
            EndpointKind::ImagesEdits => {
                let _ = gemini_canvas::resolve_image_model(model)?;
                if gemini_canvas::requested_output_count(req) > 1 {
                    return Err(gemini_canvas::unsupported_image_edit_count_error(provider));
                }

                let prompt =
                    gemini_web_reverse_modular::prompt_for_legacy_mixed_lane_media_request(
                        req,
                        gemini_canvas::GeminiCanvasMediaOperation::Image,
                    )?;
                self.execute_gemini_canvas_media_direct_http(
                    payload,
                    req,
                    model,
                    gemini_canvas::GeminiCanvasMediaOperation::Image,
                    prompt,
                    self.timeout
                        .min(Duration::from_secs(420))
                        .max(Duration::from_secs(330)),
                )
                .await
            }
            EndpointKind::MusicGenerations => {
                let _ = gemini_canvas::resolve_music_model(model)?;
                let prompt =
                    gemini_web_reverse_modular::prompt_for_legacy_mixed_lane_media_request(
                        req,
                        gemini_canvas::GeminiCanvasMediaOperation::Music,
                    )?;
                if gemini_canvas::pure_http_enabled(payload) {
                    match self
                        .execute_gemini_canvas_media_direct_http(
                            payload,
                            req,
                            model,
                            gemini_canvas::GeminiCanvasMediaOperation::Music,
                            prompt.clone(),
                            self.timeout.max(Duration::from_secs(480)),
                        )
                        .await
                    {
                        Ok(body)
                            if payload.adapter == "gemini_web_reverse_modular_compatible"
                                && gemini_canvas_music_response_requires_browser_followup(
                                    &body,
                                ) =>
                        {
                            debug!(
                                provider,
                                adapter = %payload.adapter,
                                "gemini web reverse modular legacy music direct HTTP lane reached accepted/pending semantics; continuing with browser-owned music invocation"
                            );
                        }
                        Ok(body) => return Ok(body),
                        Err(error)
                            if payload.adapter != "gemini_web_reverse_modular_compatible" =>
                        {
                            return Err(error);
                        }
                        Err(error) => {
                            debug!(
                                provider,
                                adapter = %payload.adapter,
                                error = %summarize_gateway_error(&error),
                                "gemini web reverse modular legacy music direct HTTP lane failed; falling back to browser-owned music invocation"
                            );
                        }
                    }
                }
                let browser_runtime_state_object_key =
                    browser_runtime_state_object_key_for("music");
                let request_timeout = self.timeout.max(Duration::from_secs(480));
                let invocation_input =
                    gemini_canvas_web_reverse_modular::build_browser_operation_invocation_input_from_values(
                        base_url,
                        &runtime.share_id,
                        &browser_runtime_state_object_key,
                        browser_cdp_url.as_deref(),
                        browser_cookie_header.as_deref(),
                        "music",
                        &prompt,
                        &locale,
                        request_timeout,
                    );
                let result = if let Some(result) = self
                    .execute_remote_browser_executor(
                        "gemini_canvas",
                        provider_account_id,
                        req.endpoint_kind,
                        invocation_input,
                    )
                    .await?
                {
                    gemini_canvas_web_reverse_modular::parse_remote_media_browser_invocation_value(
                        provider, result, "music",
                    )?
                } else {
                    let browser_pool_base_url =
                        self.ensure_gemini_canvas_browser_pool(provider).await?;
                    self.execute_gemini_canvas_browser_request_with_recovery(
                        provider,
                        &browser_pool_base_url,
                        base_url,
                        &runtime.share_id,
                        &browser_runtime_state_object_key,
                        browser_cdp_url.as_deref(),
                        browser_cookie_header.as_deref(),
                        "music",
                        &prompt,
                        &locale,
                        request_timeout,
                    )
                    .await?
                };

                let asset = result
                    .media
                    .iter()
                    .find(|asset| asset.kind == "video" || asset.kind == "audio")
                    .map(convert_gemini_canvas_asset)
                    .ok_or_else(|| gemini_canvas_music_missing_asset_error(provider))?;

                Ok(gemini_canvas::build_music_generation_response(
                    model,
                    &prompt,
                    &asset,
                    result.body_text.as_deref(),
                ))
            }
            EndpointKind::VideosGenerations => {
                let _ = gemini_canvas::resolve_video_model(model)?;
                if gemini_canvas::requested_output_count(req) > 1 {
                    return Err(gemini_canvas_video_unsupported_count_error(provider));
                }

                let prompt =
                    gemini_web_reverse_modular::prompt_for_legacy_mixed_lane_media_request(
                        req,
                        gemini_canvas::GeminiCanvasMediaOperation::Video,
                    )?;
                let continuation = gemini_canvas_video_continuation_from_request(req)?;
                let is_continuation = continuation.is_some();
                let browser_resume_requested = req
                    .raw_body
                    .get("resume_existing_media")
                    .and_then(Value::as_bool)
                    .or_else(|| {
                        req.raw_body
                            .get("resumeExistingMedia")
                            .and_then(Value::as_bool)
                    })
                    .unwrap_or(false);
                if gemini_canvas::pure_http_enabled(payload)
                    && is_continuation
                    && !browser_resume_requested
                {
                    return self
                        .execute_gemini_canvas_media_direct_http(
                            payload,
                            req,
                            model,
                            gemini_canvas::GeminiCanvasMediaOperation::Video,
                            prompt,
                            self.timeout.max(Duration::from_secs(720)),
                        )
                        .await;
                }
                let browser_runtime_state_object_key =
                    browser_runtime_state_object_key_for("video");
                let request_timeout = self.timeout.max(Duration::from_secs(720));
                let mut invocation_input =
                    gemini_canvas_web_reverse_modular::build_browser_operation_invocation_input_from_values(
                        base_url,
                        &runtime.share_id,
                        &browser_runtime_state_object_key,
                        browser_cdp_url.as_deref(),
                        browser_cookie_header.as_deref(),
                        "video",
                        &prompt,
                        &locale,
                        request_timeout,
                    );
                if let Some(continuation) = continuation.as_ref() {
                    apply_gemini_canvas_browser_video_continuation(
                        &mut invocation_input,
                        continuation,
                    );
                }
                let result = if let Some(result) = self
                    .execute_remote_browser_executor(
                        "gemini_canvas",
                        provider_account_id,
                        req.endpoint_kind,
                        invocation_input.clone(),
                    )
                    .await?
                {
                    gemini_canvas_web_reverse_modular::parse_remote_media_browser_invocation_value(
                        provider, result, "video",
                    )?
                } else {
                    let browser_pool_base_url =
                        self.ensure_gemini_canvas_browser_pool(provider).await?;
                    gemini_canvas_web_reverse_modular::execute_browser_request_input_with_recovery(
                        &self.http,
                        request_timeout,
                        provider,
                        &browser_pool_base_url,
                        &invocation_input,
                        "video",
                    )
                    .await?
                };

                let mut asset = result
                    .media
                    .iter()
                    .find(|asset| asset.kind == "video")
                    .map(convert_gemini_canvas_asset)
                    .ok_or_else(|| gemini_canvas_video_missing_asset_error(provider))?;

                if asset.body_base64.is_none()
                    && (asset.url.starts_with("http://") || asset.url.starts_with("https://"))
                {
                    let browser_pool_base_url =
                        self.ensure_gemini_canvas_browser_pool(provider).await?;
                    let (bytes, response_content_type) =
                        gemini_canvas_web_reverse_modular::execute_connected_fetch_get_bytes(
                            &self.http,
                            request_timeout,
                            provider,
                            &browser_pool_base_url,
                            base_url,
                            &runtime.share_id,
                            &browser_runtime_state_object_key,
                            browser_cdp_url.as_deref(),
                            browser_cookie_header.as_deref(),
                            &asset.url,
                        )
                        .await?;
                    if bytes.is_empty() {
                        return Err(gemini_canvas_video_missing_asset_error(provider));
                    }
                    if let Some(content_type) = response_content_type
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| value.starts_with("video/"))
                    {
                        asset.mime_type = content_type.to_string();
                    }
                    asset.body_base64 =
                        Some(base64::engine::general_purpose::STANDARD.encode(bytes));
                }

                Ok(gemini_canvas::build_video_generation_response(
                    model,
                    &prompt,
                    &asset,
                    result.body_text.as_deref(),
                ))
            }
            _ => Err(gemini_canvas::unsupported_media_adapter_endpoint_error(
                provider,
            )),
        }
    }
}
