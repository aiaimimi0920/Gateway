use super::*;

impl UpstreamClient {
    pub(super) async fn send_gemini_canvas_stream_generate_request(
        &self,
        request_context: StreamGenerateRequestContext,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        mode_index: i64,
        timeout: Duration,
        allow_replay_template: bool,
        image_edit_followup_context: &mut Option<&mut GeminiCanvasImageEditFollowupContext>,
    ) -> Result<String, GatewayError> {
        let StreamGenerateRequestContext {
            preparation:
                StreamGeneratePreparation {
                    effective_base_url: _,
                    app_bootstrap_url: _,
                    share_bootstrap_url: _,
                    is_text_mode,
                    is_image_mode,
                    is_image_edit_request,
                    image_stream_timeout,
                    bootstrap_url,
                    stream_url,
                    storage_state: _,
                    batchexecute_header_id: _,
                    replay_template,
                    session,
                },
            bootstrap,
            origin,
            authorization,
            page_path,
            text_preflight_source_path: _,
            referer,
            model_header,
            request_uuid,
            request,
        } = request_context;
        let provider = "gemini_canvas_compatible";
        if is_image_mode {
            if let Some(template) = replay_template.as_ref() {
                let refreshed_template =
                    gemini_canvas::refresh_stream_generate_template_with_bootstrap(
                        template, &bootstrap, true,
                    )?;
                let mut headers = HeaderMap::new();
                apply_gemini_canvas_replay_template_headers(
                    &mut headers,
                    &refreshed_template.headers,
                    &session,
                );
                if is_image_edit_request {
                    append_gemini_canvas_image_edit_request_debug_snapshot(
                        "gemini-canvas-image-edit-request-debug-template-post-refresh.json",
                        "template-post-refresh",
                        &refreshed_template.url,
                        &refreshed_template.query,
                        &refreshed_template.form,
                        || gemini_canvas_debug_headers_snapshot_from_header_map(&headers),
                        || {
                            build_gemini_canvas_image_edit_template_post_refresh_debug_extra(
                                &request_uuid,
                                &runtime.runtime_state_object_key,
                                &gemini_canvas::image_edit_stream_generate_template_object_key(
                                    &runtime.runtime_state_object_key,
                                ),
                                bootstrap.build_label.as_deref(),
                                bootstrap.session_id.as_deref(),
                                Some(bootstrap.language.as_str()),
                            )
                        },
                    );
                }
                debug!(
                    provider,
                    url = %refreshed_template.url,
                    "sending gemini canvas pure HTTP image StreamGenerate request via refreshed replay template"
                );
                let response = self
                    .http
                    .request(Method::POST, &refreshed_template.url)
                    .headers(headers)
                    .query(&refreshed_template.query)
                    .timeout(image_stream_timeout)
                    .body(refreshed_template.raw_post_data.clone())
                    .send()
                    .await
                    .map_err(|error| classify_network_error(&error, Some(provider)))?;
                let status = response.status().as_u16();
                let final_url = response.uri().to_string();
                let location = response
                    .headers()
                    .get(rquest::header::LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string);
                let content_type = response
                    .headers()
                    .get(rquest::header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string);
                let body_text = response
                    .text()
                    .await
                    .map_err(|error| classify_network_error(&error, Some(provider)))?;
                if is_image_edit_request {
                    append_gemini_canvas_image_edit_stream_response_debug_snapshot(
                        "gemini-canvas-image-edit-stream-response-template-after-refresh",
                        "template-after-refresh-response",
                        &final_url,
                        status,
                        location.as_deref(),
                        content_type.as_deref(),
                        &body_text,
                        || {
                            build_gemini_canvas_image_edit_stream_response_template_after_refresh_debug_extra(
                            &runtime.runtime_state_object_key,
                            bootstrap.build_label.as_deref(),
                            bootstrap.session_id.as_deref(),
                            Some(bootstrap.language.as_str()),
                        )
                        },
                    );
                }
                if (200..300).contains(&status)
                    && !gemini_web::response_indicates_browser_challenge(
                        status,
                        content_type.as_deref(),
                        &body_text,
                    )
                    && !gemini_web::response_indicates_session_invalid(
                        status,
                        content_type.as_deref(),
                        &body_text,
                    )
                    && !gemini_canvas::response_indicates_image_generation_unavailable(
                        status,
                        content_type.as_deref(),
                        &body_text,
                    )
                {
                    if is_image_edit_request {
                        if let Some(context) = image_edit_followup_context.as_deref_mut() {
                            if context.signaler_response_id.is_none() {
                                if let Some(response_id) =
                                    gemini_canvas::extract_stream_generate_response_id(&body_text)
                                        .ok()
                                {
                                    context.signaler_response_id = Some(response_id);
                                }
                            }
                        }
                        debug!(
                            provider,
                            status,
                            "gemini canvas pure HTTP image-edit replay template returned a non-challenge StreamGenerate body after bootstrap refresh; deferring asset resolution to follow-up extraction"
                        );
                        return Ok(body_text);
                    }
                    return Ok(body_text);
                }
                debug!(
                    provider,
                    status,
                    content_type = content_type.as_deref().unwrap_or("<none>"),
                    "gemini canvas pure HTTP image replay template failed after bootstrap refresh; falling back to heavy builder"
                );
            }
        }

        let mut headers = gemini_web_reverse_modular::build_headers(payload, None, model);
        apply_gemini_canvas_direct_http_stream_generate_headers(
            &mut headers,
            payload,
            &request_uuid,
            &model_header,
            if is_text_mode { Some(&session) } else { None },
            is_text_mode || is_image_mode,
        );
        if is_text_mode {
            headers.remove(HeaderName::from_static("origin"));
        }
        if is_image_mode {
            apply_gemini_canvas_cookie_header(&mut headers, &session);
            insert_header_map_value(
                &mut headers,
                "x-same-domain",
                gemini_web::GEMINI_WEB_DEFAULT_SAME_DOMAIN_HEADER,
            );
            headers
                .entry(rquest::header::CONTENT_TYPE)
                .or_insert(HeaderValue::from_static(
                    "application/x-www-form-urlencoded;charset=UTF-8",
                ));
            headers.remove("authorization");
            headers.remove("x-origin");
            headers.remove("x-goog-authuser");
        } else if !is_text_mode {
            apply_gemini_canvas_signed_headers(
                &mut headers,
                &session,
                &origin,
                &referer,
                &authorization,
                true,
            );
        }
        let browser_runtime_state_object_key =
            gemini_canvas::browser_runtime_state_object_key_for_browser_operation(
                payload,
                if is_text_mode { "text" } else { "image" },
            )
            .unwrap_or_else(|| runtime.runtime_state_object_key.clone());
        let browser_cdp_url = gemini_canvas::browser_cdp_url(payload);
        let browser_cookie_header = gemini_canvas::browser_cookie_header(payload);
        let stream_request_contract = build_gemini_canvas_image_edit_stream_request_contract(
            &stream_url,
            &bootstrap_url,
            &page_path,
            mode_index,
            is_text_mode,
            is_image_mode,
            &headers,
        );
        if is_image_edit_request
            && !allow_replay_template
            && should_attempt_gemini_canvas_browser_backed_image_edit_retry(
                payload,
                EndpointKind::ImagesEdits,
            )
            && !browser_runtime_state_object_key.trim().is_empty()
        {
            match self.ensure_gemini_canvas_browser_pool(provider).await {
                Ok(browser_pool_base_url) => {
                    debug!(
                        provider,
                        browser_runtime_state_object_key = %browser_runtime_state_object_key,
                        has_browser_cdp = browser_cdp_url.is_some(),
                        "retrying gemini canvas image-edit heavy StreamGenerate through browser-backed fetch before pure HTTP fallback"
                    );
                    match self
                        .execute_gemini_canvas_browser_fetch_form_request(
                            provider,
                            &browser_pool_base_url,
                            origin.as_str(),
                            runtime.share_id.as_str(),
                            browser_runtime_state_object_key.as_str(),
                            browser_cdp_url.as_deref(),
                            browser_cookie_header.as_deref(),
                            &stream_url,
                            &request.query,
                            &headers,
                            &request.form,
                            image_stream_timeout,
                        )
                        .await
                    {
                        Ok(invocation) => {
                            let browser_status = invocation.status;
                            let browser_content_type = invocation.content_type.clone();
                            let browser_final_url = invocation
                                .final_url
                                .clone()
                                .unwrap_or_else(|| stream_url.clone());
                            if let Some(body_text) = invocation.body_text {
                                if is_image_edit_request {
                                    append_gemini_canvas_image_edit_stream_response_debug_snapshot(
                                        "gemini-canvas-image-edit-stream-response-heavy-browser-fetch",
                                        "heavy-builder-browser-fetch-response",
                                        &browser_final_url,
                                        browser_status,
                                        None,
                                        browser_content_type.as_deref(),
                                        &body_text,
                                        || {
                                            build_gemini_canvas_image_edit_stream_response_heavy_debug_extra(
                                            &browser_runtime_state_object_key,
                                            &request_uuid,
                                            &stream_request_contract,
                                        )
                                        },
                                    );
                                }
                                let browser_failed = !(200..300).contains(&browser_status)
                                    || gemini_web::response_indicates_browser_challenge(
                                        browser_status,
                                        browser_content_type.as_deref(),
                                        &body_text,
                                    )
                                    || gemini_web::response_indicates_session_invalid(
                                        browser_status,
                                        browser_content_type.as_deref(),
                                        &body_text,
                                    );
                                if !browser_failed {
                                    return Ok(body_text);
                                }
                                debug!(
                                    provider,
                                    status = browser_status,
                                    content_type =
                                        browser_content_type.as_deref().unwrap_or("<none>"),
                                    "browser-backed image-edit heavy StreamGenerate did not yield a usable non-challenge body; falling back to pure HTTP heavy send"
                                );
                            } else {
                                debug!(
                                    provider,
                                    status = browser_status,
                                    "browser-backed image-edit heavy StreamGenerate completed without a text body; falling back to pure HTTP heavy send"
                                );
                            }
                        }
                        Err(error) => {
                            debug!(
                                provider,
                                error = %summarize_gateway_error(&error),
                                "browser-backed image-edit heavy StreamGenerate fetch failed; falling back to pure HTTP heavy send"
                            );
                        }
                    }
                }
                Err(error) => {
                    debug!(
                        provider,
                        error = %summarize_gateway_error(&error),
                        "failed to prepare Gemini Canvas browser pool for image-edit heavy fetch; continuing with pure HTTP heavy send"
                    );
                }
            }
        }
        if is_image_edit_request {
            append_gemini_canvas_image_edit_request_debug_snapshot(
                "gemini-canvas-image-edit-request-debug-heavy.json",
                "heavy-builder",
                &stream_url,
                &request.query,
                &request.form,
                || gemini_canvas_debug_headers_snapshot_from_header_map(&headers),
                || {
                    build_gemini_canvas_image_edit_heavy_builder_debug_extra(
                        &request_uuid,
                        &runtime.runtime_state_object_key,
                        &gemini_canvas::image_edit_stream_generate_template_object_key(
                            &runtime.runtime_state_object_key,
                        ),
                        &stream_request_contract,
                        bootstrap.build_label.as_deref(),
                        bootstrap.session_id.as_deref(),
                        Some(bootstrap.language.as_str()),
                    )
                },
            );
        }

        debug!(
            provider,
            url = %stream_url,
            page_path = %page_path,
            mode_index,
            "sending gemini canvas pure HTTP StreamGenerate request"
        );
        let mut request_form = request.form.clone();
        let mut xsrf_retry_token: Option<String> = None;

        loop {
            let response = self
                .http
                .request(Method::POST, &stream_url)
                .headers(headers.clone())
                .query(&request.query)
                .timeout(image_stream_timeout)
                .form(&request_form)
                .send()
                .await
                .map_err(|error| classify_network_error(&error, Some(provider)))?;
            let final_url = response.uri().to_string();
            let status = response.status().as_u16();
            let location = response
                .headers()
                .get(rquest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            let content_type = response
                .headers()
                .get(rquest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            let (stream_operation, allow_early_locator) =
                gemini_canvas_stream_collection_policy(mode_index, is_image_edit_request);
            let body_text = self
                .collect_gemini_canvas_stream_generate_body(
                    response,
                    provider,
                    stream_operation,
                    allow_early_locator,
                )
                .await?;
            if is_image_edit_request {
                append_gemini_canvas_image_edit_stream_response_debug_snapshot(
                    "gemini-canvas-image-edit-stream-response-heavy",
                    "heavy-builder-response",
                    &final_url,
                    status,
                    location.as_deref(),
                    content_type.as_deref(),
                    &body_text,
                    || {
                        build_gemini_canvas_image_edit_stream_response_heavy_debug_extra(
                            &runtime.runtime_state_object_key,
                            &request_uuid,
                            &stream_request_contract,
                        )
                    },
                );
            }
            let stream_response_meta = build_gemini_canvas_image_edit_stream_response_meta(
                &final_url,
                location.as_deref(),
                content_type.as_deref(),
                &body_text,
            );
            let failed = !(200..300).contains(&status)
                || gemini_web::response_indicates_browser_challenge(
                    status,
                    content_type.as_deref(),
                    &body_text,
                )
                || gemini_web::response_indicates_session_invalid(
                    status,
                    content_type.as_deref(),
                    &body_text,
                );
            if failed {
                if status == 400 {
                    if let Some(token) = maybe_retry_gemini_canvas_form_xsrf_token(
                        &mut request_form,
                        &body_text,
                        &mut xsrf_retry_token,
                    ) {
                        debug!(
                            provider,
                            xsrf_token_preview = %truncate_response_preview(&token, 24),
                            "retrying gemini canvas StreamGenerate with xsrf token extracted from upstream error"
                        );
                        continue;
                    }
                }
                return Err(append_gateway_error_summary(
                    append_gateway_error_summary(
                        classify_gemini_canvas_pure_http_error(
                            status,
                            content_type.as_deref(),
                            &body_text,
                        ),
                        "stream_request_contract",
                        Some(&stream_request_contract),
                    ),
                    "stream_response_meta",
                    Some(&stream_response_meta),
                ));
            }

            return Ok(body_text);
        }
    }
}
