use super::*;

impl UpstreamClient {
    pub(super) async fn execute_gemini_canvas_direct_http_image_primary_body(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        timeout: Duration,
        context: &mut GeminiCanvasDirectHttpImageContext,
    ) -> Result<GeminiCanvasDirectHttpImagePrimaryResult, GatewayError> {
        let provider = "gemini_canvas_compatible";
        if context.image_json_policy.initial_action()
            == GeminiCanvasDirectHttpImageJsonAction::TryJson
        {
            match self
                .execute_gemini_canvas_direct_http_image_json(
                    payload, req, model, runtime, prompt, timeout,
                )
                .await
            {
                Ok(body) => {
                    return Ok(GeminiCanvasDirectHttpImagePrimaryResult::FinalResponse(
                        body,
                    ));
                }
                Err(image_json_error) => {
                    let image_json_summary = summarize_gateway_error(&image_json_error);
                    debug!(
                        provider,
                        error = %image_json_summary,
                        "gemini canvas direct HTTP image inline JSON attempt failed; falling back to StreamGenerate asset replay for non-url response"
                    );
                    context
                        .image_json_policy
                        .note_prefill_failure(image_json_summary);
                }
            }
        }
        if req.endpoint_kind == EndpointKind::ImagesEdits
            && !context.initial_stream_allows_replay_template
        {
            append_gemini_canvas_image_edit_trace("stream.force-heavy-only", || {
                "Skipping replay-template-first path for focused image-edit probe."
            });
        }
        match self
            .execute_gemini_canvas_direct_http_stream_generate_body(
                payload,
                model,
                runtime,
                context.mode_index,
                prompt,
                timeout,
                context.initial_stream_allows_replay_template,
                context.image_edit_uploads.as_deref(),
                context.image_edit_followup_context.as_mut(),
            )
            .await
        {
            Ok(body_text) => Ok(GeminiCanvasDirectHttpImagePrimaryResult::StreamBody(
                body_text,
            )),
            Err(stream_error) => match context.image_json_policy.on_stream_failure() {
                GeminiCanvasDirectHttpImageJsonAction::ReturnOriginal => Err(stream_error),
                GeminiCanvasDirectHttpImageJsonAction::ReturnOriginalWithSummary {
                    context_key,
                    summary,
                } => Err(append_gateway_error_summary(
                    stream_error,
                    context_key,
                    Some(&summary),
                )),
                GeminiCanvasDirectHttpImageJsonAction::TryJsonWithErrorContext { context_key } => {
                    let stream_summary = summarize_gateway_error(&stream_error);
                    debug!(
                        provider,
                        error = %stream_summary,
                        "gemini canvas direct HTTP image StreamGenerate replay failed; trying legacy JSON fallback because image_json_fallback_enabled=true"
                    );
                    match self
                        .execute_gemini_canvas_direct_http_image_json(
                            payload, req, model, runtime, prompt, timeout,
                        )
                        .await
                    {
                        Ok(body) => Ok(GeminiCanvasDirectHttpImagePrimaryResult::FinalResponse(
                            body,
                        )),
                        Err(image_json_error) => Err(append_gateway_error_summary(
                            image_json_error,
                            context_key,
                            Some(&stream_summary),
                        )),
                    }
                }
                GeminiCanvasDirectHttpImageJsonAction::Skip
                | GeminiCanvasDirectHttpImageJsonAction::TryJson => Err(stream_error),
            },
        }
    }

    pub(super) async fn execute_gemini_canvas_direct_http_media_followup_body(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        operation: gemini_canvas::GeminiCanvasMediaOperation,
        prompt: &str,
        stream_body: &str,
        locator_override: Option<gemini_canvas::GeminiCanvasStreamGenerateLocator>,
        request_started_at: SystemTime,
        timeout: Duration,
        force_root_app_followup: bool,
        locale_override: Option<&str>,
    ) -> Result<String, GatewayError> {
        let GeminiCanvasMediaFollowupContext {
            bootstrap,
            bootstrap_page_url,
            mut session,
            mut followup_target,
        } = self
            .prepare_gemini_canvas_direct_http_media_followup_context(
                payload,
                runtime,
                operation,
                stream_body,
                locator_override,
                timeout,
                force_root_app_followup,
                locale_override,
            )
            .await?;
        let preflight_contract = match self
            .execute_gemini_canvas_media_followup_preflight_contract(
                payload,
                model,
                &bootstrap,
                &followup_target,
                &mut session,
                operation,
                force_root_app_followup,
                timeout,
            )
            .await?
        {
            GeminiCanvasMediaFollowupPreflightOutcome::Completed(body) => return Ok(body),
            GeminiCanvasMediaFollowupPreflightOutcome::Ready(contract) => contract,
        };
        let GeminiCanvasMediaFollowupPreflightContract {
            preflight_plan,
            strategy: preflight_strategy,
            selected_bootstrap_body,
        } = preflight_contract;

        self.execute_gemini_canvas_media_followup_after_preflight(
            payload,
            model,
            runtime,
            operation,
            prompt,
            stream_body,
            request_started_at,
            timeout,
            force_root_app_followup,
            locale_override,
            &bootstrap,
            &session,
            &mut followup_target,
            &preflight_plan,
            preflight_strategy,
            selected_bootstrap_body.as_deref(),
            &bootstrap_page_url,
        )
        .await
    }

    pub(super) async fn execute_gemini_canvas_media_followup_after_preflight(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        operation: gemini_canvas::GeminiCanvasMediaOperation,
        prompt: &str,
        stream_body: &str,
        request_started_at: SystemTime,
        timeout: Duration,
        force_root_app_followup: bool,
        locale_override: Option<&str>,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        followup_target: &mut GeminiCanvasFollowupTarget,
        preflight_plan: &GeminiCanvasMediaFollowupPreflightPlan,
        preflight_strategy: GeminiCanvasMediaFollowupPreflightStrategy,
        selected_bootstrap_body: Option<&str>,
        bootstrap_page_url: &str,
    ) -> Result<String, GatewayError> {
        if operation == gemini_canvas::GeminiCanvasMediaOperation::Video {
            return self
                .execute_gemini_canvas_video_post_preflight_followup(
                    payload,
                    model,
                    runtime,
                    prompt,
                    stream_body,
                    request_started_at,
                    timeout,
                    force_root_app_followup,
                    locale_override,
                    bootstrap,
                    session,
                    followup_target,
                    preflight_plan,
                    preflight_strategy,
                    selected_bootstrap_body,
                    bootstrap_page_url,
                )
                .await;
        }
        if operation == gemini_canvas::GeminiCanvasMediaOperation::Music {
            return self
                .execute_gemini_canvas_music_post_preflight_followup(
                    payload,
                    model,
                    runtime,
                    prompt,
                    stream_body,
                    request_started_at,
                    timeout,
                    force_root_app_followup,
                    locale_override,
                    bootstrap,
                    session,
                    followup_target,
                    preflight_plan,
                    preflight_strategy,
                    selected_bootstrap_body,
                    bootstrap_page_url,
                )
                .await;
        }

        let provider = "gemini_canvas_compatible";
        let response_id_log = followup_target.response_id();
        let conversation_id_log = followup_target.conversation_id();
        debug!(
            provider,
            app_path = %followup_target.source_path,
            response_id = response_id_log,
            conversation_id = conversation_id_log,
            bootstrap_page = %bootstrap_page_url,
            preflight_strategy = preflight_strategy.as_str(),
            selected_bootstrap_preview = %selected_bootstrap_body
                .map(|body| compact_response_preview(body, 180))
                .unwrap_or_else(|| "<none>".to_string()),
            "sending gemini canvas pure HTTP media aPya6c follow-up request"
        );
        let followup_attempt_state = match self
            .execute_gemini_canvas_media_followup_attempts(
                payload,
                model,
                operation,
                followup_target,
                preflight_plan,
                session,
                bootstrap_page_url,
                timeout,
            )
            .await
        {
            GeminiCanvasMediaFollowupAttemptOutcome::Completed(body) => return Ok(body),
            GeminiCanvasMediaFollowupAttemptOutcome::Incomplete(state) => state,
        };

        finalize_gemini_canvas_media_followup_attempt_state(
            provider,
            followup_target,
            bootstrap_page_url,
            followup_attempt_state,
        )
    }

    pub(super) async fn execute_gemini_canvas_video_post_preflight_followup(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        stream_body: &str,
        request_started_at: SystemTime,
        timeout: Duration,
        force_root_app_followup: bool,
        locale_override: Option<&str>,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        followup_target: &mut GeminiCanvasFollowupTarget,
        preflight_plan: &GeminiCanvasMediaFollowupPreflightPlan,
        preflight_strategy: GeminiCanvasMediaFollowupPreflightStrategy,
        selected_bootstrap_body: Option<&str>,
        bootstrap_page_url: &str,
    ) -> Result<String, GatewayError> {
        let provider = "gemini_canvas_compatible";
        self.try_ensure_gemini_canvas_video_followup_locator(
            payload,
            model,
            bootstrap,
            session,
            &preflight_plan.followup_model_header,
            prompt,
            request_started_at,
            stream_body,
            timeout,
            followup_target,
        )
        .await;
        let response_id_log = followup_target.response_id();
        let conversation_id_log = followup_target.conversation_id();
        debug!(
            provider,
            app_path = %followup_target.source_path,
            response_id = response_id_log,
            conversation_id = conversation_id_log,
            bootstrap_page = %bootstrap_page_url,
            preflight_strategy = preflight_strategy.as_str(),
            selected_bootstrap_preview = %selected_bootstrap_body
                .map(|body| compact_response_preview(body, 180))
                .unwrap_or_else(|| "<none>".to_string()),
            "sending gemini canvas pure HTTP video aPya6c follow-up request"
        );
        let followup_attempt_state = match self
            .execute_gemini_canvas_media_followup_attempts(
                payload,
                model,
                gemini_canvas::GeminiCanvasMediaOperation::Video,
                followup_target,
                preflight_plan,
                session,
                bootstrap_page_url,
                timeout,
            )
            .await
        {
            GeminiCanvasMediaFollowupAttemptOutcome::Completed(body) => return Ok(body),
            GeminiCanvasMediaFollowupAttemptOutcome::Incomplete(state) => state,
        };

        self.finalize_gemini_canvas_video_followup_result(
            payload,
            model,
            runtime,
            bootstrap,
            session,
            followup_target,
            preflight_plan,
            stream_body,
            timeout,
            force_root_app_followup,
            locale_override,
            bootstrap_page_url,
            followup_attempt_state,
        )
        .await
    }

    pub(super) async fn execute_gemini_canvas_music_post_preflight_followup(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        stream_body: &str,
        request_started_at: SystemTime,
        timeout: Duration,
        force_root_app_followup: bool,
        locale_override: Option<&str>,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        followup_target: &mut GeminiCanvasFollowupTarget,
        preflight_plan: &GeminiCanvasMediaFollowupPreflightPlan,
        preflight_strategy: GeminiCanvasMediaFollowupPreflightStrategy,
        selected_bootstrap_body: Option<&str>,
        bootstrap_page_url: &str,
    ) -> Result<String, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let response_id_log = followup_target.response_id();
        let conversation_id_log = followup_target.conversation_id();
        debug!(
            provider,
            app_path = %followup_target.source_path,
            response_id = response_id_log,
            conversation_id = conversation_id_log,
            bootstrap_page = %bootstrap_page_url,
            preflight_strategy = preflight_strategy.as_str(),
            selected_bootstrap_preview = %selected_bootstrap_body
                .map(|body| compact_response_preview(body, 180))
                .unwrap_or_else(|| "<none>".to_string()),
            "sending gemini canvas pure HTTP music aPya6c follow-up request"
        );
        let followup_attempt_state = match self
            .execute_gemini_canvas_media_followup_attempts(
                payload,
                model,
                gemini_canvas::GeminiCanvasMediaOperation::Music,
                followup_target,
                preflight_plan,
                session,
                bootstrap_page_url,
                timeout,
            )
            .await
        {
            GeminiCanvasMediaFollowupAttemptOutcome::Completed(body) => return Ok(body),
            GeminiCanvasMediaFollowupAttemptOutcome::Incomplete(state) => state,
        };

        self.finalize_gemini_canvas_music_followup_result(
            payload,
            model,
            runtime,
            bootstrap,
            session,
            prompt,
            stream_body,
            request_started_at,
            timeout,
            force_root_app_followup,
            locale_override,
            followup_target,
            preflight_plan,
            bootstrap_page_url,
            followup_attempt_state,
        )
        .await
    }

    pub(super) async fn try_ensure_gemini_canvas_video_followup_locator(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        model_header: &str,
        prompt: &str,
        request_started_at: SystemTime,
        stream_body: &str,
        timeout: Duration,
        followup_target: &mut GeminiCanvasFollowupTarget,
    ) {
        let provider = "gemini_canvas_compatible";
        if followup_target.locator.is_some() {
            return;
        }
        let response_id_hint = gemini_canvas::extract_stream_generate_response_id(stream_body).ok();
        match self
            .try_recover_gemini_canvas_video_locator_from_conversation_list(
                payload,
                model,
                bootstrap,
                session,
                &followup_target.source_path,
                model_header,
                prompt,
                request_started_at,
                response_id_hint.as_deref(),
                timeout,
            )
            .await
        {
            Ok(Some(recovered_locator)) => {
                followup_target.adopt_video_recovered_locator(recovered_locator);
            }
            Ok(None) => {}
            Err(error) => {
                debug!(
                    provider,
                    error = %summarize_gateway_error(&error),
                    "gemini canvas video conversation-list locator recovery failed"
                );
            }
        }
    }
}
