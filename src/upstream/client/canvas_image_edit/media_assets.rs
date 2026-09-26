use super::*;

impl UpstreamClient {
    pub(in crate::upstream::client) async fn poll_gemini_canvas_media_assets_from_conversation_page(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        operation: gemini_canvas::GeminiCanvasMediaOperation,
        stream_body: &str,
        timeout: Duration,
        force_root_app_followup: bool,
        locale_override: Option<&str>,
        session_override: Option<&gemini_canvas::GeminiCanvasPureHttpSession>,
        bootstrap_page_url_override: Option<&str>,
    ) -> Result<(Vec<gemini_canvas::GeminiCanvasMediaAsset>, String), GatewayError> {
        let provider = "gemini_canvas_compatible";
        let page_seed = prepare_gemini_canvas_page_seed(
            payload,
            runtime,
            operation,
            stream_body,
            force_root_app_followup,
            bootstrap_page_url_override,
            true,
            provider,
            "gemini canvas page poll",
        )?;
        let locator = page_seed.locator;
        let base_url = payload.base_url.trim_end_matches('/');
        let app_bootstrap_url = page_seed.app_bootstrap_url;
        let share_bootstrap_url = page_seed.share_bootstrap_url;
        let has_bootstrap_override = page_seed.has_page_url_override;
        let locator_mode = classify_gemini_canvas_page_target_mode(
            force_root_app_followup,
            has_bootstrap_override,
            locator.is_some(),
            false,
        );
        let conversation_url = page_seed.conversation_page_url;
        if operation == gemini_canvas::GeminiCanvasMediaOperation::Image && force_root_app_followup
        {
            append_gemini_canvas_image_edit_trace("page-poll.bootstrap", || {
                conversation_url
                    .as_deref()
                    .unwrap_or(app_bootstrap_url.as_str())
            });
        }
        let storage_state = gateway_object_storage()?
            .read_json(&runtime.runtime_state_object_key)
            .await?;
        let auth_user = gemini_canvas::direct_http_auth_user(payload);
        let mut session = if let Some(existing_session) = session_override {
            existing_session.clone()
        } else {
            gemini_canvas_pure_http_session_from_payload_or_storage(
                payload,
                &storage_state,
                conversation_url
                    .as_deref()
                    .unwrap_or(app_bootstrap_url.as_str()),
                base_url,
                &auth_user,
            )?
        };
        let poll_urls = build_gemini_canvas_page_poll_urls(
            force_root_app_followup,
            has_bootstrap_override,
            conversation_url.as_deref(),
            &app_bootstrap_url,
            &share_bootstrap_url,
        );
        let mut failures = Vec::new();
        let mut last_page_preview = None;
        let started_at = Instant::now();
        let page_poll_timing = plan_gemini_canvas_conversation_page_poll_timing(
            operation,
            has_bootstrap_override,
            force_root_app_followup,
            timeout,
        );
        let poll_budget = page_poll_timing.poll_budget;
        let sleep_between_attempts = page_poll_timing.sleep_between_attempts;
        let max_fetch_timeout = page_poll_timing.max_fetch_timeout;
        let mut attempt_count = 0usize;

        loop {
            let Some(remaining) = resolve_gemini_canvas_conversation_page_poll_remaining(
                poll_budget,
                started_at.elapsed(),
            ) else {
                break;
            };
            attempt_count += 1;
            if let Some(refresh_page_url) =
                resolve_gemini_canvas_conversation_page_poll_refresh_target(
                    operation,
                    has_bootstrap_override,
                    force_root_app_followup,
                    conversation_url.as_deref(),
                    app_bootstrap_url.as_str(),
                )
            {
                match self
                    .trigger_gemini_canvas_image_page_refresh(
                        payload,
                        model,
                        &storage_state,
                        &mut session,
                        refresh_page_url,
                        remaining.min(Duration::from_secs(20)),
                    )
                    .await
                {
                    Ok(body) => {
                        failures.push(gemini_canvas_conversation_page_poll_refresh_body_entry(
                            attempt_count,
                            &body,
                        ));
                    }
                    Err(error) => {
                        failures.push(gemini_canvas_conversation_page_poll_refresh_error_entry(
                            attempt_count,
                            &error,
                        ));
                    }
                }
            }
            for page_url in &poll_urls {
                let Some(remaining_for_fetch) =
                    resolve_gemini_canvas_conversation_page_poll_remaining(
                        poll_budget,
                        started_at.elapsed(),
                    )
                else {
                    break;
                };
                match self
                    .fetch_gemini_canvas_direct_http_page_html_refreshing_session_with_locale(
                        payload,
                        &mut session,
                        page_url,
                        remaining_for_fetch.min(max_fetch_timeout),
                        locale_override,
                    )
                    .await
                {
                    Ok(page_body) => {
                        match try_extract_gemini_canvas_conversation_page_poll_assets_from_body(
                            attempt_count,
                            page_url,
                            operation,
                            page_body,
                        ) {
                            Ok(result) => return Ok(result),
                            Err(failure) => {
                                last_page_preview = Some(failure.page_preview);
                                failures.push(failure.failure_entry);
                            }
                        }
                    }
                    Err(error) => {
                        failures.push(gemini_canvas_conversation_page_poll_fetch_error_entry(
                            attempt_count,
                            page_url,
                            &error,
                        ));
                    }
                }
            }

            if should_sleep_after_gemini_canvas_conversation_page_poll_attempt(
                poll_budget,
                started_at.elapsed(),
                sleep_between_attempts,
            ) {
                sleep(sleep_between_attempts).await;
                continue;
            }
            break;
        }

        let app_path = locator
            .as_ref()
            .map(|locator| locator.app_path.as_str())
            .unwrap_or(gemini_web::GEMINI_WEB_DEFAULT_APP_PATH);
        let failure_summary = failures.join(" | ");
        let last_page_preview_text = last_page_preview.unwrap_or_else(|| "<none>".to_string());
        Err(gemini_canvas_conversation_page_missing_asset_error(
            operation,
            locator_mode,
            app_path,
            attempt_count,
            poll_budget.as_secs(),
            &failure_summary,
            &last_page_preview_text,
        ))
    }
}
