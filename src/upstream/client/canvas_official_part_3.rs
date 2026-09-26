use super::*;

impl UpstreamClient {
    pub(super) async fn poll_gemini_canvas_image_edit_signaler_assets(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        timeout: Duration,
        mut edit_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
    ) -> Result<(Vec<gemini_canvas::GeminiCanvasMediaAsset>, String), GatewayError> {
        let base_url = payload.base_url.trim_end_matches('/');
        let app_url = format!("{base_url}{}", gemini_web::GEMINI_WEB_DEFAULT_APP_PATH);
        let storage_state = gateway_object_storage()?
            .read_json(&runtime.runtime_state_object_key)
            .await?;
        let locale_hint = edit_context
            .as_ref()
            .and_then(|context| context.locale_hint.clone())
            .or_else(|| gemini_canvas::harvest_image_edit_template_locale(&storage_state));
        let auth_user = gemini_canvas::direct_http_auth_user(payload);
        let (mut session, mut channel) =
            prepare_gemini_canvas_image_edit_signaler_poll_state_with_http(
                &self.http,
                &self.plain_http,
                payload,
                runtime,
                &storage_state,
                base_url,
                &app_url,
                &auth_user,
                locale_hint.as_deref(),
                edit_context.as_ref().map(|context| &**context),
                timeout,
            )
            .await?;
        let started_at = Instant::now();
        let total_budget = timeout
            .min(Duration::from_secs(540))
            .max(Duration::from_secs(270));
        let mut last_body_preview = None;
        let mut failures = Vec::new();
        let mut seen_app_paths = HashSet::new();
        let mut first_app_path_seen_at: Option<Instant> = None;

        loop {
            let Some(remaining) = total_budget.checked_sub(started_at.elapsed()) else {
                break;
            };
            if remaining <= Duration::from_secs(3) {
                break;
            }
            let poll_url = build_gemini_canvas_image_edit_signaler_poll_url(
                &channel,
                &gemini_canvas_signaler_zx_token(),
            );
            match self
                .send_gemini_canvas_signaler_poll_request_refreshing_session(
                    payload,
                    &mut session,
                    &poll_url,
                    remaining.min(Duration::from_secs(285)),
                    channel.next_aid,
                    locale_hint.as_deref(),
                )
                .await
            {
                Ok(body) => {
                    last_body_preview =
                        Some(record_gemini_canvas_image_edit_signaler_poll_body_preview(
                            edit_context.as_deref_mut(),
                            &body,
                        ));
                    if let Ok(assets) = extract_gemini_canvas_image_edit_signaler_assets_from_body(
                        &body,
                        &session,
                        &channel,
                        locale_hint.as_deref(),
                        edit_context.as_deref_mut(),
                    ) {
                        return Ok((assets, body));
                    }
                    for app_path in gemini_canvas::extract_signaler_app_paths(&body) {
                        let page_url = record_gemini_canvas_image_edit_signaler_app_path(
                            base_url,
                            app_path.as_str(),
                            &mut seen_app_paths,
                            &mut first_app_path_seen_at,
                            edit_context.as_deref_mut(),
                        );
                        match self
                            .fetch_gemini_canvas_direct_http_page_html_with_locale(
                                payload,
                                &session,
                                &page_url,
                                remaining.min(Duration::from_secs(20)),
                                locale_hint.as_deref(),
                            )
                            .await
                        {
                            Ok(page_body) => {
                                if let Some((assets, page_body)) =
                                    try_extract_gemini_canvas_image_edit_signaler_assets_response_from_body(
                                        page_body,
                                        &session,
                                        &channel,
                                        locale_hint.as_deref(),
                                        edit_context.as_deref_mut(),
                                    )
                                {
                                    return Ok((assets, page_body));
                                }
                                match self
                                    .trigger_gemini_canvas_image_page_refresh(
                                        payload,
                                        model,
                                        &storage_state,
                                        &mut session,
                                        &page_url,
                                        remaining.min(Duration::from_secs(20)),
                                    )
                                    .await
                                {
                                    Ok(refresh_preview) => {
                                        failures.push(
                                            gemini_canvas_image_edit_signaler_page_failure_entry(
                                                &page_url,
                                                "signaler_page_refresh",
                                                compact_response_preview(&refresh_preview, 220),
                                            ),
                                        );
                                        match self
                                            .fetch_gemini_canvas_direct_http_page_html_with_locale(
                                                payload,
                                                &session,
                                                &page_url,
                                                remaining.min(Duration::from_secs(20)),
                                                locale_hint.as_deref(),
                                            )
                                            .await
                                        {
                                            Ok(refreshed_page_body) => {
                                                if let Some((assets, refreshed_page_body)) =
                                                    try_extract_gemini_canvas_image_edit_signaler_assets_response_from_body(
                                                        refreshed_page_body,
                                                        &session,
                                                        &channel,
                                                        locale_hint.as_deref(),
                                                        edit_context.as_deref_mut(),
                                                    )
                                                {
                                                    return Ok((assets, refreshed_page_body));
                                                }
                                            }
                                            Err(error) => {
                                                failures.push(
                                                    gemini_canvas_image_edit_signaler_page_failure_entry(
                                                    &page_url,
                                                    "refetch",
                                                    summarize_gateway_error(&error)
                                                    ),
                                                );
                                            }
                                        }
                                    }
                                    Err(error) => {
                                        failures.push(
                                            gemini_canvas_image_edit_signaler_page_failure_entry(
                                                &page_url,
                                                "signaler_page_refresh",
                                                summarize_gateway_error(&error),
                                            ),
                                        );
                                    }
                                }
                            }
                            Err(error) => {
                                failures.push(
                                    gemini_canvas_image_edit_signaler_page_failure_entry(
                                        &page_url,
                                        "fetch",
                                        summarize_gateway_error(&error),
                                    ),
                                );
                            }
                        }
                    }
                    if let Some(handoff_error) =
                        try_finish_gemini_canvas_image_edit_signaler_handoff_ready(
                            seen_app_paths.len(),
                            first_app_path_seen_at.map(|first_seen_at| first_seen_at.elapsed()),
                            last_body_preview.as_deref(),
                            edit_context.as_deref_mut(),
                            &session,
                            &channel,
                            locale_hint.as_deref(),
                            &body,
                        )
                    {
                        return Err(handoff_error);
                    }
                    match refresh_gemini_canvas_image_edit_signaler_creds_from_body_with_http(
                        &self.http,
                        payload,
                        &mut session,
                        &channel,
                        &body,
                        remaining.min(Duration::from_secs(30)),
                        locale_hint.as_deref(),
                    )
                    .await
                    {
                        Ok(_) => {}
                        Err(error) => {
                            failures.push(gemini_canvas_image_edit_signaler_refresh_error_entry(
                                channel.next_aid,
                                summarize_gateway_error(&error),
                            ));
                        }
                    }
                    update_gemini_canvas_image_edit_signaler_next_aid_from_body(
                        &mut channel,
                        &body,
                    );
                }
                Err(error) => {
                    failures.push(gemini_canvas_image_edit_signaler_poll_error_entry(
                        channel.next_aid,
                        summarize_gateway_error(&error),
                    ));
                    sleep(Duration::from_millis(800)).await;
                }
            }
        }
        Err(finish_gemini_canvas_image_edit_signaler_missing_asset(
            edit_context.as_deref_mut(),
            &session,
            &channel,
            locale_hint.as_deref(),
            &failures,
            last_body_preview,
        ))
    }
}
