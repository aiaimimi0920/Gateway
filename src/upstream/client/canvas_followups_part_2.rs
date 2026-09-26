use super::*;

impl UpstreamClient {
    pub(super) async fn recover_gemini_canvas_video_completion_or_page_body(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        followup_target: &GeminiCanvasFollowupTarget,
        locator: &gemini_canvas::GeminiCanvasStreamGenerateLocator,
        model_header: &str,
        stream_body: &str,
        timeout: Duration,
        force_root_app_followup: bool,
        locale_override: Option<&str>,
    ) -> Result<String, GatewayError> {
        let provider = "gemini_canvas_compatible";
        match self
            .poll_gemini_canvas_direct_http_video_completion_body(
                payload,
                model,
                bootstrap,
                session,
                &followup_target.source_path,
                &locator.conversation_id,
                &locator.response_id,
                model_header,
                stream_body,
                timeout,
            )
            .await
        {
            Ok(body) => Ok(body),
            Err(error) => {
                debug!(
                    provider,
                    app_path = %followup_target.source_path,
                    response_id = followup_target.response_id(),
                    conversation_id = followup_target.conversation_id(),
                    error = %summarize_gateway_error(&error),
                    "gemini canvas media video completion follow-up failed; trying concrete page fallback"
                );
                let base_url = payload.base_url.trim_end_matches('/');
                let concrete_page_url = format!("{base_url}{}", locator.app_path);
                match self
                    .poll_gemini_canvas_media_assets_from_conversation_page(
                        payload,
                        model,
                        runtime,
                        gemini_canvas::GeminiCanvasMediaOperation::Video,
                        stream_body,
                        timeout
                            .min(Duration::from_secs(150))
                            .max(Duration::from_secs(45)),
                        force_root_app_followup,
                        locale_override,
                        Some(session),
                        Some(concrete_page_url.as_str()),
                    )
                    .await
                {
                    Ok((_assets, page_body)) => Ok(page_body),
                    Err(page_error) => {
                        debug!(
                            provider,
                            app_path = %locator.app_path,
                            response_id = followup_target.response_id(),
                            conversation_id = followup_target.conversation_id(),
                            error = %summarize_gateway_error(&page_error),
                            "gemini canvas media concrete page poll after video completion failure also missed asset"
                        );
                        let completion_summary = summarize_gateway_error(&error);
                        Err(append_gateway_error_summary(
                            page_error,
                            "video_completion_followup_failure",
                            Some(&completion_summary),
                        ))
                    }
                }
            }
        }
    }

    pub(super) async fn execute_gemini_canvas_video_completion_attempt(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        attempt: usize,
        source_path: &str,
        conversation_id: &str,
        response_id: &str,
        requests: &GeminiCanvasVideoCompletionRequests,
        model_header: &str,
        remaining: Duration,
    ) -> Result<GeminiCanvasVideoCompletionAttemptOutcome, GatewayError> {
        let completion_body = self
            .send_gemini_canvas_text_batchexecute_request(
                payload,
                model,
                &requests.completion_request,
                session,
                model_header,
                remaining.min(Duration::from_secs(60)),
            )
            .await?;

        let completion = match classify_gemini_canvas_video_stage_body(completion_body) {
            Ok(body) => return Ok(GeminiCanvasVideoCompletionAttemptOutcome::Completed(body)),
            Err(progress) => progress,
        };
        let metadata_body = self
            .try_fetch_gemini_canvas_video_metadata_body(
                payload,
                model,
                session,
                attempt,
                source_path,
                conversation_id,
                response_id,
                &requests.metadata_request,
                model_header,
                remaining,
            )
            .await;
        if let Some(metadata_body) = metadata_body {
            let metadata = match classify_gemini_canvas_video_stage_body(metadata_body) {
                Ok(body) => return Ok(GeminiCanvasVideoCompletionAttemptOutcome::Completed(body)),
                Err(progress) => progress,
            };
            Ok(GeminiCanvasVideoCompletionAttemptOutcome::Continue(
                GeminiCanvasVideoCompletionAttemptState {
                    completion,
                    metadata: Some(metadata),
                },
            ))
        } else {
            Ok(GeminiCanvasVideoCompletionAttemptOutcome::Continue(
                GeminiCanvasVideoCompletionAttemptState {
                    completion,
                    metadata: None,
                },
            ))
        }
    }

    pub(super) async fn try_fetch_gemini_canvas_video_metadata_body(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        attempt: usize,
        source_path: &str,
        conversation_id: &str,
        response_id: &str,
        metadata_request: &gemini_web::GeminiWebRequest,
        model_header: &str,
        remaining: Duration,
    ) -> Option<String> {
        let provider = "gemini_canvas_compatible";
        match self
            .send_gemini_canvas_text_batchexecute_request(
                payload,
                model,
                metadata_request,
                session,
                model_header,
                remaining.min(Duration::from_secs(20)),
            )
            .await
        {
            Ok(body) => Some(body),
            Err(error) => {
                debug!(
                    provider,
                    attempt,
                    source_path,
                    conversation_id,
                    response_id,
                    error = %summarize_gateway_error(&error),
                    "gemini canvas video MUAZcd metadata follow-up failed"
                );
                None
            }
        }
    }

    pub(super) async fn try_send_gemini_canvas_video_job_poll(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        attempt: usize,
        source_path: &str,
        conversation_id: &str,
        model_header: &str,
        remaining: Duration,
        job_poll_request: Option<&Result<gemini_web::GeminiWebRequest, GatewayError>>,
    ) -> Option<String> {
        let provider = "gemini_canvas_compatible";
        let Some(job_poll_request) = job_poll_request else {
            return None;
        };
        match job_poll_request {
            Ok(request) => match self
                .send_gemini_canvas_text_batchexecute_request(
                    payload,
                    model,
                    request,
                    session,
                    model_header,
                    remaining.min(Duration::from_secs(30)),
                )
                .await
            {
                Ok(body) => Some(body),
                Err(error) => {
                    debug!(
                        provider,
                        attempt,
                        source_path,
                        conversation_id,
                        error = %summarize_gateway_error(&error),
                        "gemini canvas video kwDCne job poll failed"
                    );
                    None
                }
            },
            Err(error) => {
                debug!(
                    provider,
                    attempt,
                    source_path,
                    conversation_id,
                    error = %summarize_gateway_error(error),
                    "gemini canvas video kwDCne request build failed"
                );
                None
            }
        }
    }

    pub(super) async fn try_recover_gemini_canvas_video_locator_from_conversation_list(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        source_path: &str,
        model_header: &str,
        prompt: &str,
        request_started_at: SystemTime,
        response_id_hint: Option<&str>,
        timeout: Duration,
    ) -> Result<Option<gemini_canvas::GeminiCanvasStreamGenerateLocator>, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let response_id = response_id_hint
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        if response_id.is_none() {
            return Ok(None);
        }

        let probe_request =
            gemini_canvas::build_conversation_list_probe_request(bootstrap, source_path)?;
        let full_request =
            gemini_canvas::build_conversation_list_full_request(bootstrap, source_path)?;
        let mut session = session.clone();
        let mut bodies = Vec::new();

        for (label, request) in [("probe", &probe_request), ("full", &full_request)] {
            match self
                .send_gemini_canvas_text_batchexecute_request_capture_aligned_refreshing_session(
                    payload,
                    model,
                    request,
                    &mut session,
                    model_header,
                    timeout
                        .min(Duration::from_secs(20))
                        .max(Duration::from_secs(8)),
                )
                .await
            {
                Ok(body) => bodies.push((label, body)),
                Err(error) => {
                    debug!(
                        provider,
                        rpc = label,
                        app_path = %source_path,
                        error = %summarize_gateway_error(&error),
                        "gemini canvas video conversation-list locator recovery request failed"
                    );
                }
            }
        }

        let mut entries = Vec::new();
        for (_, body) in &bodies {
            entries.extend(gemini_canvas::extract_conversation_list_entries(body));
        }
        if entries.is_empty() {
            return Ok(None);
        }

        let Some(selected_entry) =
            select_gemini_canvas_recent_conversation_entry(&entries, prompt, request_started_at)
        else {
            return Ok(None);
        };
        let Some(app_path) = selected_entry.app_path() else {
            return Ok(None);
        };

        debug!(
            provider,
            app_path = %app_path,
            conversation_id = %selected_entry.conversation_id,
            response_id = %response_id.as_deref().unwrap_or("<none>"),
            entries = %preview_gemini_canvas_conversation_entries(&entries, 5),
            "recovered gemini canvas video locator from conversation list"
        );

        Ok(Some(gemini_canvas::GeminiCanvasStreamGenerateLocator {
            response_id: response_id.unwrap(),
            conversation_id: selected_entry.conversation_id.clone(),
            app_path,
        }))
    }

    pub(super) async fn try_recover_gemini_canvas_recent_conversation_page_url(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        source_path: &str,
        model_header: &str,
        prompt: &str,
        request_started_at: SystemTime,
        timeout: Duration,
    ) -> Result<Option<String>, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let probe_request =
            gemini_canvas::build_conversation_list_probe_request(bootstrap, source_path)?;
        let full_request =
            gemini_canvas::build_conversation_list_full_request(bootstrap, source_path)?;
        let mut session = session.clone();
        let mut bodies = Vec::new();

        for (label, request) in [("probe", &probe_request), ("full", &full_request)] {
            match self
                .send_gemini_canvas_text_batchexecute_request_capture_aligned_refreshing_session(
                    payload,
                    model,
                    request,
                    &mut session,
                    model_header,
                    timeout
                        .min(Duration::from_secs(20))
                        .max(Duration::from_secs(8)),
                )
                .await
            {
                Ok(body) => bodies.push((label, body)),
                Err(error) => {
                    debug!(
                        provider,
                        rpc = label,
                        app_path = %source_path,
                        error = %summarize_gateway_error(&error),
                        "gemini canvas music conversation-list recovery request failed"
                    );
                }
            }
        }

        let mut entries = Vec::new();
        for (_, body) in &bodies {
            entries.extend(gemini_canvas::extract_conversation_list_entries(body));
        }
        if entries.is_empty() {
            return Ok(None);
        }

        let Some(selected_entry) =
            select_gemini_canvas_recent_conversation_entry(&entries, prompt, request_started_at)
        else {
            return Ok(None);
        };
        let Some(app_path) = selected_entry.app_path() else {
            return Ok(None);
        };
        let app_url = format!("{}{}", payload.base_url.trim_end_matches('/'), app_path);

        debug!(
            provider,
            app_url = %app_url,
            conversation_id = %selected_entry.conversation_id,
            response_id = %selected_entry.response_id.as_deref().unwrap_or("<none>"),
            entries = %preview_gemini_canvas_conversation_entries(&entries, 5),
            "recovered gemini canvas recent conversation page for music asset follow-up"
        );

        Ok(Some(app_url))
    }
}
