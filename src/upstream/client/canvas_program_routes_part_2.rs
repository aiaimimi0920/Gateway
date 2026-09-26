use super::*;

#[path = "canvas_program_routes_part_2/image_materialization.rs"]
mod image_materialization;

impl UpstreamClient {
    pub(super) async fn execute_gemini_canvas_direct_http_stream_generate_response_with_text_context(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        prompt: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        timeout: Duration,
    ) -> Result<rquest::Response, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let configured_base_url = payload.base_url.trim_end_matches('/');
        let effective_base_url = if configured_base_url.is_empty() {
            gemini_canvas_http_origin(payload)
        } else {
            configured_base_url.to_string()
        };
        let stream_url = format!(
            "{effective_base_url}{}",
            gemini_web::GEMINI_WEB_DEFAULT_STREAM_GENERATE_PATH
        );
        let text_preflight_source_path = gemini_web::GEMINI_WEB_DEFAULT_APP_PATH.to_string();
        let request_uuid = gemini_canvas::new_stream_generate_request_uuid();
        let request = gemini_canvas::build_stream_generate_heavy_request(
            prompt,
            bootstrap,
            &request_uuid,
            gemini_canvas::GEMINI_CANVAS_TEXT_STREAM_GENERATE_TEXT_MODE_INDEX,
        )?;

        for (rpcid, rpc_payload, model_header) in
            build_gemini_canvas_direct_http_text_generic_preflight_specs(&bootstrap.language)
        {
            self.execute_gemini_canvas_text_generic_preflight(
                payload,
                model,
                bootstrap,
                &text_preflight_source_path,
                session,
                rpcid,
                rpc_payload,
                model_header,
                timeout,
            )
            .await?;
        }
        let (state_len, tail_index, tail_value, marker) =
            gemini_canvas_direct_http_text_fast_version_preflight_spec();
        self.execute_gemini_canvas_text_state_variant_preflight(
            payload,
            model,
            bootstrap,
            &text_preflight_source_path,
            session,
            state_len,
            tail_index,
            tail_value,
            marker,
            timeout,
        )
        .await?;
        self.execute_gemini_canvas_text_mode_selection_preflight(
            payload,
            model,
            bootstrap,
            &text_preflight_source_path,
            session,
            "",
            "",
            timeout,
        )
        .await?;
        self.execute_gemini_canvas_text_bootstrap_preflight(
            payload,
            model,
            bootstrap,
            &text_preflight_source_path,
            session,
            "",
            "",
            timeout,
        )
        .await?;
        self.execute_gemini_canvas_text_state_preflight(
            payload,
            model,
            bootstrap,
            &text_preflight_source_path,
            session,
            "",
            "",
            timeout,
        )
        .await?;
        for (state_len, tail_index, tail_value, marker) in [
            (41usize, 40usize, Value::from(0), "side_nav_open_by_default"),
            (87usize, 86usize, Value::from(1), "popup_zs_visits_cooldown"),
            (87usize, 86usize, Value::from(2), "popup_zs_visits_cooldown"),
            (
                94usize,
                93usize,
                Value::String("NULL".to_string()),
                "current_popup_id",
            ),
            (
                94usize,
                93usize,
                Value::String("HUMAN_REVIEWER_DISCLOSURE".to_string()),
                "current_popup_id",
            ),
        ] {
            if let Err(error) = self
                .execute_gemini_canvas_text_state_variant_preflight(
                    payload,
                    model,
                    bootstrap,
                    &text_preflight_source_path,
                    session,
                    state_len,
                    tail_index,
                    tail_value,
                    marker,
                    timeout,
                )
                .await
            {
                debug!(
                    provider = "gemini_canvas_compatible",
                    error = %summarize_gateway_error(&error),
                    marker,
                    state_len,
                    tail_index,
                    "gemini canvas text optional page-state update failed during TTS direct HTTP StreamGenerate; continuing"
                );
            }
        }

        let mut headers = gemini_web_reverse_modular::build_headers(payload, None, model);
        apply_gemini_canvas_direct_http_stream_generate_headers(
            &mut headers,
            payload,
            &request_uuid,
            gemini_canvas::GEMINI_CANVAS_TEXT_STREAM_GENERATE_MODEL_HEADER,
            Some(session),
            true,
        );

        debug!(
            provider,
            url = %stream_url,
            "sending gemini canvas pure HTTP TTS StreamGenerate request with reused text context"
        );
        let mut request_form = request.form.clone();
        let mut xsrf_retry_token: Option<String> = None;
        loop {
            let response = self
                .http
                .request(Method::POST, &stream_url)
                .headers(headers.clone())
                .query(&request.query)
                .timeout(timeout.max(Duration::from_secs(120)))
                .form(&request_form)
                .send()
                .await
                .map_err(|error| classify_network_error(&error, Some(provider)))?;
            let status = response.status().as_u16();
            let content_type = response
                .headers()
                .get(rquest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            let content_type_is_html = content_type
                .as_deref()
                .is_some_and(|value| value.to_ascii_lowercase().contains("text/html"));
            if !(200..300).contains(&status) || content_type_is_html {
                let body_text = response
                    .text()
                    .await
                    .map_err(|error| classify_network_error(&error, Some(provider)))?;
                if status == 400 {
                    if let Some(token) = extract_gemini_canvas_batchexecute_xsrf_token(&body_text) {
                        let current_at = header_map_string_from_form(&request_form, "at");
                        if xsrf_retry_token.as_deref() != Some(token.as_str())
                            && current_at.as_deref() != Some(token.as_str())
                        {
                            debug!(
                                provider,
                                xsrf_token_preview = %truncate_response_preview(&token, 24),
                                "retrying gemini canvas TTS StreamGenerate with xsrf token extracted from upstream error"
                            );
                            upsert_form_field(&mut request_form, "at", &token);
                            xsrf_retry_token = Some(token);
                            continue;
                        }
                    }
                }
                return Err(classify_gemini_canvas_pure_http_error(
                    status,
                    content_type.as_deref(),
                    &body_text,
                ));
            }

            return Ok(response);
        }
    }

    pub(super) async fn execute_gemini_canvas_direct_http_image_lane(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        body_text: &str,
        timeout: Duration,
        mode_index: i64,
        request_started_at: SystemTime,
        initial_stream_allows_replay_template: bool,
        image_edit_uploads: Option<&[gemini_canvas::GeminiCanvasImageEditUpload]>,
        mut image_edit_followup_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
        image_json_policy: &GeminiCanvasDirectHttpImageJsonPolicy,
    ) -> Result<Value, GatewayError> {
        let recovery_strategy = build_gemini_canvas_image_recovery_strategy(
            req.endpoint_kind,
            initial_stream_allows_replay_template,
            body_text,
        );
        let image_assets = self
            .resolve_gemini_canvas_direct_http_image_assets(
                payload,
                req.endpoint_kind,
                model,
                runtime,
                prompt,
                body_text,
                timeout,
                mode_index,
                request_started_at,
                recovery_strategy,
                image_edit_uploads,
                image_edit_followup_context.as_deref_mut(),
            )
            .await?;

        self.build_gemini_canvas_direct_http_image_response(
            payload,
            req,
            model,
            runtime,
            prompt,
            timeout,
            &image_assets,
            image_json_policy,
        )
        .await
    }

    pub(super) async fn resolve_gemini_canvas_direct_http_image_assets(
        &self,
        payload: &ProviderAccountPayload,
        endpoint_kind: EndpointKind,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        body_text: &str,
        timeout: Duration,
        mode_index: i64,
        request_started_at: SystemTime,
        recovery_strategy: GeminiCanvasImageRecoveryStrategy,
        image_edit_uploads: Option<&[gemini_canvas::GeminiCanvasImageEditUpload]>,
        mut image_edit_followup_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
    ) -> Result<Vec<gemini_canvas::GeminiCanvasMediaAsset>, GatewayError> {
        let (image_assets, _resolved_body_text) = match self
            .extract_gemini_canvas_image_assets_with_followup(
                payload,
                model,
                runtime,
                prompt,
                body_text,
                request_started_at,
                timeout,
                recovery_strategy.mode,
                image_edit_followup_context.as_deref_mut(),
            )
            .await
        {
            Ok(result) => result,
            Err(primary_error) => {
                self.execute_gemini_canvas_direct_http_image_retry_strategy(
                    payload,
                    endpoint_kind,
                    model,
                    runtime,
                    prompt,
                    primary_error,
                    timeout,
                    mode_index,
                    request_started_at,
                    recovery_strategy,
                    image_edit_uploads,
                    image_edit_followup_context.as_deref_mut(),
                )
                .await?
            }
        };
        Ok(image_assets)
    }

    pub(super) async fn execute_gemini_canvas_direct_http_image_retry_strategy(
        &self,
        payload: &ProviderAccountPayload,
        endpoint_kind: EndpointKind,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        primary_error: GatewayError,
        timeout: Duration,
        mode_index: i64,
        request_started_at: SystemTime,
        recovery_strategy: GeminiCanvasImageRecoveryStrategy,
        image_edit_uploads: Option<&[gemini_canvas::GeminiCanvasImageEditUpload]>,
        mut image_edit_followup_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
    ) -> Result<(Vec<gemini_canvas::GeminiCanvasMediaAsset>, String), GatewayError> {
        let provider = "gemini_canvas_compatible";
        match recovery_strategy.template_retry_action {
            GeminiCanvasImageTemplateRetryAction::ReturnOriginal => Err(primary_error),
            GeminiCanvasImageTemplateRetryAction::ReturnOriginalWithLog(message) => {
                debug!(
                    provider,
                    endpoint_kind = ?endpoint_kind,
                    "{message}"
                );
                if should_attempt_gemini_canvas_browser_backed_image_edit_retry(
                    payload,
                    endpoint_kind,
                ) {
                    return self
                        .retry_resolve_gemini_canvas_direct_http_image_assets_with_legacy_template(
                            payload,
                            model,
                            runtime,
                            prompt,
                            &primary_error,
                            timeout,
                            mode_index,
                            request_started_at,
                            image_edit_uploads,
                            image_edit_followup_context.as_deref_mut(),
                            endpoint_kind,
                            recovery_strategy.mode,
                        )
                        .await;
                }
                Err(primary_error)
            }
            GeminiCanvasImageTemplateRetryAction::RetryLegacyTemplate => {
                self.retry_resolve_gemini_canvas_direct_http_image_assets_with_legacy_template(
                    payload,
                    model,
                    runtime,
                    prompt,
                    &primary_error,
                    timeout,
                    mode_index,
                    request_started_at,
                    image_edit_uploads,
                    image_edit_followup_context.as_deref_mut(),
                    endpoint_kind,
                    recovery_strategy.mode,
                )
                .await
            }
        }
    }

    pub(super) async fn retry_resolve_gemini_canvas_direct_http_image_assets_with_legacy_template(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        primary_error: &GatewayError,
        timeout: Duration,
        mode_index: i64,
        request_started_at: SystemTime,
        image_edit_uploads: Option<&[gemini_canvas::GeminiCanvasImageEditUpload]>,
        mut image_edit_followup_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
        endpoint_kind: EndpointKind,
        recovery_mode: GeminiCanvasImageRecoveryMode,
    ) -> Result<(Vec<gemini_canvas::GeminiCanvasMediaAsset>, String), GatewayError> {
        let provider = "gemini_canvas_compatible";
        let retry_summary = summarize_gateway_error(primary_error);
        debug!(
            provider,
            error = %retry_summary,
            endpoint_kind = ?endpoint_kind,
            "gemini canvas direct HTTP image asset extraction failed after template-capable path; retrying legacy heavy builder"
        );
        let retry_body = self
            .execute_gemini_canvas_direct_http_stream_generate_body(
                payload,
                model,
                runtime,
                mode_index,
                prompt,
                timeout,
                false,
                image_edit_uploads,
                image_edit_followup_context.as_deref_mut(),
            )
            .await
            .map_err(|retry_error| {
                append_gateway_error_summary(
                    retry_error,
                    "image_template_extract_failure",
                    Some(&retry_summary),
                )
            })?;
        self.extract_gemini_canvas_image_assets_with_followup(
            payload,
            model,
            runtime,
            prompt,
            &retry_body,
            request_started_at,
            timeout,
            recovery_mode,
            image_edit_followup_context.as_deref_mut(),
        )
        .await
        .map_err(|retry_extract_error| {
            append_gateway_error_summary(
                retry_extract_error,
                "image_template_extract_failure",
                Some(&retry_summary),
            )
        })
    }
}
