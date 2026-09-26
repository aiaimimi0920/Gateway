use super::*;

#[path = "canvas_assets_part_1/async_followups.rs"]
mod async_followups;

impl UpstreamClient {
    pub(super) async fn execute_gemini_canvas_direct_http_image(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: String,
        timeout: Duration,
    ) -> Result<Value, GatewayError> {
        let mut context = prepare_image_context(payload, req, &prompt).await?;
        let primary_result = self
            .execute_gemini_canvas_direct_http_image_primary_body(
                payload,
                req,
                model,
                runtime,
                &prompt,
                timeout,
                &mut context,
            )
            .await?;
        let body_text = match primary_result {
            GeminiCanvasDirectHttpImagePrimaryResult::StreamBody(body_text) => body_text,
            GeminiCanvasDirectHttpImagePrimaryResult::FinalResponse(body) => return Ok(body),
        };

        self.execute_gemini_canvas_direct_http_image_lane(
            payload,
            req,
            model,
            runtime,
            &prompt,
            &body_text,
            timeout,
            context.mode_index,
            context.request_started_at,
            context.initial_stream_allows_replay_template,
            context.image_edit_uploads.as_deref(),
            context.image_edit_followup_context.as_mut(),
            &context.image_json_policy,
        )
        .await
    }

    pub(super) async fn extract_gemini_canvas_media_assets_with_followup(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        operation: gemini_canvas::GeminiCanvasMediaOperation,
        prompt: &str,
        primary_body: &str,
        request_started_at: SystemTime,
        timeout: Duration,
        force_root_app_followup: bool,
        _image_edit_followup_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
    ) -> Result<(Vec<gemini_canvas::GeminiCanvasMediaAsset>, String), GatewayError> {
        match gemini_canvas::extract_stream_generate_media_assets(primary_body, operation) {
            Ok(assets) => Ok((assets, primary_body.to_string())),
            Err(primary_error)
                if primary_error.code.as_deref()
                    == Some("gemini_canvas_image_generation_unavailable") =>
            {
                Err(primary_error)
            }
            Err(primary_error) => {
                self.extract_gemini_canvas_media_assets_after_primary_failure(
                    payload,
                    model,
                    runtime,
                    operation,
                    prompt,
                    primary_body,
                    primary_error,
                    request_started_at,
                    timeout,
                    force_root_app_followup,
                )
                .await
            }
        }
    }

    pub(super) async fn extract_gemini_canvas_image_assets_with_followup(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        primary_body: &str,
        request_started_at: SystemTime,
        timeout: Duration,
        recovery_mode: GeminiCanvasImageRecoveryMode,
        image_edit_followup_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
    ) -> Result<(Vec<gemini_canvas::GeminiCanvasMediaAsset>, String), GatewayError> {
        match gemini_canvas::extract_stream_generate_media_assets(
            primary_body,
            gemini_canvas::GeminiCanvasMediaOperation::Image,
        ) {
            Ok(assets) => Ok((assets, primary_body.to_string())),
            Err(primary_error)
                if primary_error.code.as_deref()
                    == Some("gemini_canvas_image_generation_unavailable") =>
            {
                Err(primary_error)
            }
            Err(primary_error) if recovery_mode == GeminiCanvasImageRecoveryMode::EditAsync => {
                self.extract_gemini_canvas_image_edit_assets_with_followup(
                    payload,
                    model,
                    runtime,
                    primary_body,
                    primary_error,
                    timeout,
                    image_edit_followup_context,
                )
                .await
            }
            Err(primary_error) => {
                self.extract_gemini_canvas_image_generation_assets_with_followup(
                    payload,
                    model,
                    runtime,
                    prompt,
                    primary_body,
                    primary_error,
                    request_started_at,
                    timeout,
                )
                .await
            }
        }
    }

    pub(super) async fn extract_gemini_canvas_image_generation_assets_with_followup(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        primary_body: &str,
        primary_error: GatewayError,
        request_started_at: SystemTime,
        timeout: Duration,
    ) -> Result<(Vec<gemini_canvas::GeminiCanvasMediaAsset>, String), GatewayError> {
        self.extract_gemini_canvas_media_assets_after_primary_failure(
            payload,
            model,
            runtime,
            gemini_canvas::GeminiCanvasMediaOperation::Image,
            prompt,
            primary_body,
            primary_error,
            request_started_at,
            timeout,
            false,
        )
        .await
    }

    pub(super) async fn extract_gemini_canvas_image_edit_assets_with_followup(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        primary_body: &str,
        mut primary_error: GatewayError,
        timeout: Duration,
        image_edit_followup_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
    ) -> Result<(Vec<gemini_canvas::GeminiCanvasMediaAsset>, String), GatewayError> {
        if let Some(result) = self
            .try_resolve_gemini_canvas_image_edit_async_followups(
                payload,
                model,
                runtime,
                primary_body,
                timeout,
                image_edit_followup_context,
                &mut primary_error,
            )
            .await?
        {
            return Ok(result);
        }
        Err(primary_error)
    }

    pub(super) async fn extract_gemini_canvas_media_assets_after_primary_failure(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        operation: gemini_canvas::GeminiCanvasMediaOperation,
        prompt: &str,
        primary_body: &str,
        mut primary_error: GatewayError,
        request_started_at: SystemTime,
        timeout: Duration,
        force_root_app_followup: bool,
    ) -> Result<(Vec<gemini_canvas::GeminiCanvasMediaAsset>, String), GatewayError> {
        let followup_body = match self
            .execute_gemini_canvas_direct_http_media_followup_body(
                payload,
                model,
                runtime,
                operation,
                prompt,
                primary_body,
                None,
                request_started_at,
                timeout,
                force_root_app_followup,
                None,
            )
            .await
        {
            Ok(body) => body,
            Err(followup_error)
                if followup_error.code.as_deref()
                    == Some("gemini_canvas_image_generation_unavailable") =>
            {
                return Err(followup_error);
            }
            Err(followup_error) => {
                if matches!(
                    operation,
                    gemini_canvas::GeminiCanvasMediaOperation::Image
                        | gemini_canvas::GeminiCanvasMediaOperation::Video
                ) && matches!(
                    followup_error.code.as_deref(),
                    Some("gemini_canvas_stream_generate_missing_response_id")
                        | Some("gemini_canvas_stream_generate_missing_conversation_id")
                ) {
                    match self
                        .poll_gemini_canvas_media_assets_from_conversation_page(
                            payload,
                            model,
                            runtime,
                            operation,
                            primary_body,
                            timeout,
                            force_root_app_followup,
                            None,
                            None,
                            None,
                        )
                        .await
                    {
                        Ok((assets, page_body)) => return Ok((assets, page_body)),
                        Err(page_poll_error) => {
                            append_gateway_error_fields(
                                &mut primary_error,
                                &[
                                    (
                                        "media_followup_failure",
                                        summarize_gateway_error(&followup_error),
                                    ),
                                    (
                                        "media_page_poll_failure",
                                        summarize_gateway_error(&page_poll_error),
                                    ),
                                ],
                            );
                            return Err(primary_error);
                        }
                    }
                }
                append_gateway_error_fields(
                    &mut primary_error,
                    &[(
                        "media_followup_failure",
                        summarize_gateway_error(&followup_error),
                    )],
                );
                return Err(primary_error);
            }
        };

        match gemini_canvas::extract_stream_generate_media_assets(&followup_body, operation) {
            Ok(assets) => Ok((assets, followup_body)),
            Err(followup_extract_error)
                if followup_extract_error.code.as_deref()
                    == Some("gemini_canvas_image_generation_unavailable") =>
            {
                Err(followup_extract_error)
            }
            Err(followup_extract_error) => {
                let page_blob_extract_error = match gemini_canvas::extract_page_blob_media_assets(
                    &followup_body,
                    operation,
                ) {
                    Ok(assets) => return Ok((assets, followup_body)),
                    Err(error) => error,
                };
                let music_followup_pending = operation
                    == gemini_canvas::GeminiCanvasMediaOperation::Music
                    && gemini_canvas_music_body_indicates_accepted_progress(&followup_body);
                let video_followup_pending = operation
                    == gemini_canvas::GeminiCanvasMediaOperation::Video
                    && (gemini_canvas::response_indicates_video_generation_pending(&followup_body)
                        || gemini_canvas::response_indicates_video_generation_quota_reached(
                            &followup_body,
                        ));
                match self
                    .poll_gemini_canvas_media_assets_from_conversation_page(
                        payload,
                        model,
                        runtime,
                        operation,
                        &followup_body,
                        timeout,
                        force_root_app_followup,
                        None,
                        None,
                        None,
                    )
                    .await
                {
                    Ok((assets, page_body)) => Ok((assets, page_body)),
                    Err(page_poll_error) => {
                        if music_followup_pending || video_followup_pending {
                            return Ok((Vec::new(), followup_body));
                        }
                        append_gateway_error_fields(
                            &mut primary_error,
                            &[
                                (
                                    "media_followup_extract_failure",
                                    summarize_gateway_error(&followup_extract_error),
                                ),
                                (
                                    "media_followup_blob_failure",
                                    summarize_gateway_error(&page_blob_extract_error),
                                ),
                                (
                                    "media_page_poll_failure",
                                    summarize_gateway_error(&page_poll_error),
                                ),
                                (
                                    "media_followup_preview",
                                    compact_gemini_diagnostic_preview(&followup_body, 320),
                                ),
                            ],
                        );
                        Err(primary_error)
                    }
                }
            }
        }
    }
}
