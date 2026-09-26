use super::*;

impl UpstreamClient {
    pub(super) async fn execute_gemini_canvas_media_followup_preflight_contract(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        followup_target: &GeminiCanvasFollowupTarget,
        session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
        operation: gemini_canvas::GeminiCanvasMediaOperation,
        is_image_edit_request: bool,
        timeout: Duration,
    ) -> Result<GeminiCanvasMediaFollowupPreflightOutcome, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let preflight_plan = build_gemini_canvas_media_followup_preflight_plan(
            bootstrap,
            &followup_target.source_path,
            operation,
        )?;

        let parity_followup = match self
            .execute_gemini_canvas_media_capture_parity_preflight_sequence(
                payload,
                model,
                bootstrap,
                &followup_target.source_path,
                session,
                preflight_plan.mode_index,
                preflight_plan.batchexecute_header_id.as_deref(),
                is_image_edit_request,
                timeout,
            )
            .await
        {
            Ok(result) => Some(result),
            Err(error) => {
                debug!(
                    provider,
                    app_path = %followup_target.source_path,
                    mode_index = preflight_plan.mode_index,
                    error = %summarize_gateway_error(&error),
                    "gemini canvas media parity follow-up preflight failed; falling back to legacy follow-up chain"
                );
                None
            }
        };

        if let Some(parity_followup) = parity_followup {
            let selected_bootstrap_body = parity_followup.selected_bootstrap_body;
            if gemini_canvas::extract_stream_generate_media_assets(
                &selected_bootstrap_body,
                operation,
            )
            .is_ok()
            {
                return Ok(GeminiCanvasMediaFollowupPreflightOutcome::Completed(
                    selected_bootstrap_body,
                ));
            }

            self.execute_gemini_canvas_media_legacy_preflight_sequence(
                payload,
                model,
                bootstrap,
                &followup_target.source_path,
                session,
                preflight_plan.mode_index,
                preflight_plan.batchexecute_header_id.as_deref(),
                timeout,
            )
            .await?;

            return Ok(GeminiCanvasMediaFollowupPreflightOutcome::Ready(
                GeminiCanvasMediaFollowupPreflightContract {
                    preflight_plan,
                    strategy: GeminiCanvasMediaFollowupPreflightStrategy::ParityThenLegacy,
                    selected_bootstrap_body: Some(selected_bootstrap_body),
                },
            ));
        }

        self.execute_gemini_canvas_media_legacy_preflight_sequence(
            payload,
            model,
            bootstrap,
            &followup_target.source_path,
            session,
            preflight_plan.mode_index,
            preflight_plan.batchexecute_header_id.as_deref(),
            timeout,
        )
        .await?;

        Ok(GeminiCanvasMediaFollowupPreflightOutcome::Ready(
            GeminiCanvasMediaFollowupPreflightContract {
                preflight_plan,
                strategy: GeminiCanvasMediaFollowupPreflightStrategy::LegacyOnly,
                selected_bootstrap_body: None,
            },
        ))
    }

    pub(super) async fn execute_gemini_canvas_media_followup_attempts(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        operation: gemini_canvas::GeminiCanvasMediaOperation,
        followup_target: &GeminiCanvasFollowupTarget,
        preflight_plan: &GeminiCanvasMediaFollowupPreflightPlan,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        bootstrap_page_url: &str,
        timeout: Duration,
    ) -> GeminiCanvasMediaFollowupAttemptOutcome {
        let provider = "gemini_canvas_compatible";
        let response_id_log = followup_target.response_id();
        let conversation_id_log = followup_target.conversation_id();
        let followup_attempts = if operation == gemini_canvas::GeminiCanvasMediaOperation::Music {
            3usize
        } else {
            1usize
        };
        let mut last_body: Option<String> = None;
        let mut last_error: Option<GatewayError> = None;

        for attempt in 0..followup_attempts {
            if operation == gemini_canvas::GeminiCanvasMediaOperation::Music {
                if let Err(error) = self
                    .send_gemini_canvas_text_batchexecute_request(
                        payload,
                        model,
                        &preflight_plan.activity_request,
                        session,
                        &preflight_plan.followup_model_header,
                        timeout,
                    )
                    .await
                {
                    debug!(
                        provider,
                        app_path = %followup_target.source_path,
                        response_id = response_id_log,
                        conversation_id = conversation_id_log,
                        attempt = attempt + 1,
                        error = %summarize_gateway_error(&error),
                        "gemini canvas media ESY5D follow-up preflight failed"
                    );
                }
            }

            match self
                .send_gemini_canvas_text_batchexecute_request(
                    payload,
                    model,
                    &preflight_plan.followup_request,
                    session,
                    &preflight_plan.followup_model_header,
                    timeout,
                )
                .await
            {
                Ok(body) => {
                    if gemini_canvas::extract_stream_generate_media_assets(&body, operation).is_ok()
                    {
                        return GeminiCanvasMediaFollowupAttemptOutcome::Completed(body);
                    }
                    last_error = Some(build_gemini_canvas_media_followup_missing_asset_error(
                        provider,
                        operation,
                        followup_target,
                        bootstrap_page_url,
                        attempt + 1,
                        &body,
                    ));
                    last_body = Some(body);
                }
                Err(error) => {
                    let mut wrapped = gemini_canvas_media_followup_failed_error(
                        provider,
                        followup_target,
                        bootstrap_page_url,
                        summarize_gateway_error(&error).as_str(),
                    );
                    wrapped.http_status = error.http_status;
                    last_error = Some(wrapped);
                }
            }

            if attempt + 1 < followup_attempts {
                sleep(Duration::from_millis(750)).await;
            }
        }

        GeminiCanvasMediaFollowupAttemptOutcome::Incomplete(GeminiCanvasMediaFollowupAttemptState {
            last_body,
            last_error,
        })
    }

    pub(super) async fn finalize_gemini_canvas_video_followup_result(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        followup_target: &GeminiCanvasFollowupTarget,
        preflight_plan: &GeminiCanvasMediaFollowupPreflightPlan,
        stream_body: &str,
        timeout: Duration,
        force_root_app_followup: bool,
        locale_override: Option<&str>,
        bootstrap_page_url: &str,
        state: GeminiCanvasMediaFollowupAttemptState,
    ) -> Result<String, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let GeminiCanvasMediaFollowupAttemptState {
            last_body,
            mut last_error,
        } = state;

        let Some(locator) = followup_target.locator.as_ref() else {
            if last_error.is_none() {
                last_error = Some(gemini_canvas_video_followup_missing_locator_error(
                    provider,
                    followup_target.source_path.as_str(),
                    bootstrap_page_url,
                ));
            }
            return finalize_gemini_canvas_media_followup_attempt_state(
                provider,
                followup_target,
                bootstrap_page_url,
                GeminiCanvasMediaFollowupAttemptState {
                    last_body,
                    last_error,
                },
            );
        };

        match self
            .recover_gemini_canvas_video_completion_or_page_body(
                payload,
                model,
                runtime,
                bootstrap,
                session,
                followup_target,
                locator,
                &preflight_plan.followup_model_header,
                stream_body,
                timeout,
                force_root_app_followup,
                locale_override,
            )
            .await
        {
            Ok(body) => Ok(body),
            Err(error) => {
                last_error = Some(error);
                finalize_gemini_canvas_media_followup_attempt_state(
                    provider,
                    followup_target,
                    bootstrap_page_url,
                    GeminiCanvasMediaFollowupAttemptState {
                        last_body,
                        last_error,
                    },
                )
            }
        }
    }

    pub(super) async fn finalize_gemini_canvas_music_followup_result(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        prompt: &str,
        stream_body: &str,
        request_started_at: SystemTime,
        timeout: Duration,
        force_root_app_followup: bool,
        locale_override: Option<&str>,
        followup_target: &GeminiCanvasFollowupTarget,
        preflight_plan: &GeminiCanvasMediaFollowupPreflightPlan,
        bootstrap_page_url: &str,
        state: GeminiCanvasMediaFollowupAttemptState,
    ) -> Result<String, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let GeminiCanvasMediaFollowupAttemptState {
            last_body,
            last_error: _,
        } = state;
        let mut page_seed_body = last_body.clone().unwrap_or_else(|| stream_body.to_string());

        if let Ok(response_id) = gemini_canvas::extract_stream_generate_response_id(&page_seed_body)
            .or_else(|_| gemini_canvas::extract_stream_generate_response_id(stream_body))
        {
            let mut trigger_session = session.clone();
            let trigger_source_paths =
                if followup_target.source_path == gemini_web::GEMINI_WEB_DEFAULT_APP_PATH {
                    vec![followup_target.source_path.clone()]
                } else {
                    vec![
                        followup_target.source_path.clone(),
                        gemini_web::GEMINI_WEB_DEFAULT_APP_PATH.to_string(),
                    ]
                };

            for trigger_source_path in trigger_source_paths {
                let trigger_request = gemini_canvas::build_music_trigger_request(
                    &response_id,
                    bootstrap,
                    &trigger_source_path,
                )?;
                match self
                    .send_gemini_canvas_text_batchexecute_request_capture_aligned_refreshing_session(
                        payload,
                        model,
                        &trigger_request,
                        &mut trigger_session,
                        &preflight_plan.followup_model_header,
                        timeout.min(Duration::from_secs(20)).max(Duration::from_secs(8)),
                    )
                    .await
                {
                    Ok(trigger_body) => {
                        if gemini_canvas::extract_stream_generate_media_assets(
                            &trigger_body,
                            gemini_canvas::GeminiCanvasMediaOperation::Music,
                        )
                        .is_ok()
                            || gemini_canvas::extract_page_blob_media_assets(
                                &trigger_body,
                                gemini_canvas::GeminiCanvasMediaOperation::Music,
                            )
                            .is_ok()
                        {
                            return Ok(trigger_body);
                        }
                        page_seed_body = trigger_body;
                        break;
                    }
                    Err(_error) => {}
                }
            }
        }

        let mut last_error = match self
            .poll_gemini_canvas_media_assets_from_conversation_page(
                payload,
                model,
                runtime,
                gemini_canvas::GeminiCanvasMediaOperation::Music,
                &page_seed_body,
                timeout,
                force_root_app_followup,
                locale_override,
                Some(session),
                None,
            )
            .await
        {
            Ok((_assets, page_body)) => return Ok(page_body),
            Err(error) => {
                let upstream_summary = summarize_gateway_error(&error);
                Some(gemini_canvas_program_music_page_poll_failed_error(
                    provider,
                    bootstrap_page_url,
                    upstream_summary.as_str(),
                ))
            }
        };

        let recovered_page_url = self
            .try_recover_gemini_canvas_recent_conversation_page_url(
                payload,
                model,
                bootstrap,
                session,
                &followup_target.source_path,
                &preflight_plan.followup_model_header,
                prompt,
                request_started_at,
                timeout,
            )
            .await?;

        if let Some(app_page_url) = recovered_page_url.as_deref() {
            match self
                .poll_gemini_canvas_media_assets_from_conversation_page(
                    payload,
                    model,
                    runtime,
                    gemini_canvas::GeminiCanvasMediaOperation::Music,
                    &page_seed_body,
                    timeout,
                    force_root_app_followup,
                    locale_override,
                    Some(session),
                    Some(app_page_url),
                )
                .await
            {
                Ok((_assets, page_body)) => return Ok(page_body),
                Err(error) => {
                    let upstream_summary = summarize_gateway_error(&error);
                    last_error = Some(gemini_canvas_program_music_recent_page_poll_failed_error(
                        provider,
                        bootstrap_page_url,
                        app_page_url,
                        upstream_summary.as_str(),
                    ));
                }
            }
        }

        finalize_gemini_canvas_media_followup_attempt_state(
            provider,
            followup_target,
            bootstrap_page_url,
            GeminiCanvasMediaFollowupAttemptState {
                last_body,
                last_error,
            },
        )
    }
}
