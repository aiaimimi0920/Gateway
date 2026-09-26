// Replays eligible captured requests before authorization and bootstrap refresh.
use super::*;

impl UpstreamClient {
    pub(super) async fn replay_gemini_canvas_stream_generate_before_bootstrap(
        &self,
        prepared: &StreamGeneratePreparation,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        timeout: Duration,
        allow_replay_template: bool,
        image_edit_followup_context: &mut Option<&mut GeminiCanvasImageEditFollowupContext>,
    ) -> Result<StreamGenerateReplayOutcome, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let is_text_mode = prepared.is_text_mode;
        let is_image_mode = prepared.is_image_mode;
        let is_image_edit_request = prepared.is_image_edit_request;
        let image_stream_timeout = prepared.image_stream_timeout;
        let replay_template = &prepared.replay_template;
        let session = &prepared.session;
        if is_image_mode {
            if let Some(template) = replay_template.as_ref() {
                match self
                    .execute_gemini_canvas_http_replay_worker(
                        provider,
                        template,
                        &session,
                        Some("image"),
                        image_stream_timeout,
                    )
                    .await
                {
                    Ok(worker_result)
                        if (200..300).contains(&worker_result.status)
                            && !gemini_web::response_indicates_browser_challenge(
                                worker_result.status,
                                worker_result.content_type.as_deref(),
                                &worker_result.body_text,
                            )
                            && !gemini_web::response_indicates_session_invalid(
                                worker_result.status,
                                worker_result.content_type.as_deref(),
                                &worker_result.body_text,
                            )
                            && !gemini_canvas::response_indicates_image_generation_unavailable(
                                worker_result.status,
                                worker_result.content_type.as_deref(),
                                &worker_result.body_text,
                            ) =>
                    {
                        if is_image_edit_request {
                            if let Some(context) = image_edit_followup_context.as_deref_mut() {
                                if context.signaler_response_id.is_none() {
                                    if let Some(response_id) =
                                        gemini_canvas::extract_stream_generate_response_id(
                                            &worker_result.body_text,
                                        )
                                        .ok()
                                    {
                                        context.signaler_response_id = Some(response_id);
                                    }
                                }
                            }
                            debug!(
                                provider,
                                status = worker_result.status,
                                "gemini canvas pure HTTP image-edit replay worker produced a non-challenge StreamGenerate body before bootstrap refresh; deferring asset resolution to follow-up extraction"
                            );
                            return Ok(StreamGenerateReplayOutcome::Completed(
                                worker_result.body_text,
                            ));
                        }
                        match gemini_canvas::extract_stream_generate_media_assets(
                            &worker_result.body_text,
                            gemini_canvas::GeminiCanvasMediaOperation::Image,
                        ) {
                            Ok(assets) if !assets.is_empty() => {
                                debug!(
                                    provider,
                                    status = worker_result.status,
                                    asset_count = assets.len(),
                                    "gemini canvas pure HTTP image replay worker produced a parseable StreamGenerate body before bootstrap refresh"
                                );
                                return Ok(StreamGenerateReplayOutcome::Completed(
                                    worker_result.body_text,
                                ));
                            }
                            Ok(_) => {
                                debug!(
                                    provider,
                                    status = worker_result.status,
                                    "gemini canvas pure HTTP image replay worker returned no parseable image assets; falling back to direct Rust replay before bootstrap refresh"
                                );
                            }
                            Err(parse_error) => {
                                debug!(
                                    provider,
                                    status = worker_result.status,
                                    error = %summarize_gateway_error(&parse_error),
                                    "gemini canvas pure HTTP image replay worker returned a non-parseable StreamGenerate body; falling back to direct Rust replay before bootstrap refresh"
                                );
                            }
                        }
                    }
                    Ok(worker_result) => {
                        debug!(
                            provider,
                            status = worker_result.status,
                            content_type =
                                worker_result.content_type.as_deref().unwrap_or("<none>"),
                            "gemini canvas pure HTTP image replay worker returned a non-usable response; falling back to direct Rust replay before bootstrap refresh"
                        );
                    }
                    Err(error) => {
                        debug!(
                            provider,
                            error = %summarize_gateway_error(&error),
                            "gemini canvas pure HTTP image replay worker failed; falling back to direct Rust replay before bootstrap refresh"
                        );
                    }
                }
                let mut headers = HeaderMap::new();
                apply_gemini_canvas_replay_template_headers(
                    &mut headers,
                    &template.headers,
                    &session,
                );
                debug!(
                    provider,
                    url = %template.url,
                    "sending gemini canvas pure HTTP image StreamGenerate request via harvested replay template before bootstrap refresh"
                );
                let response = self
                    .http
                    .request(Method::POST, &template.url)
                    .headers(headers)
                    .query(&template.query)
                    .timeout(image_stream_timeout)
                    .body(template.raw_post_data.clone())
                    .send()
                    .await
                    .map_err(|error| classify_network_error(&error, Some(provider)))?;
                let status = response.status().as_u16();
                let content_type = response
                    .headers()
                    .get(rquest::header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string);
                let body_text = self
                    .collect_gemini_canvas_stream_generate_body(
                        response,
                        provider,
                        gemini_canvas::GeminiCanvasMediaOperation::Image,
                        is_image_edit_request,
                    )
                    .await?;
                if is_image_edit_request {
                    append_gemini_canvas_image_edit_stream_response_debug_snapshot(
                        "gemini-canvas-image-edit-stream-response-template-before-refresh",
                        "template-before-refresh-response",
                        &template.url,
                        status,
                        None,
                        content_type.as_deref(),
                        &body_text,
                        || {
                            build_gemini_canvas_image_edit_stream_response_template_before_refresh_debug_extra(
                            &runtime.runtime_state_object_key,
                            allow_replay_template,
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
                            "gemini canvas pure HTTP image-edit harvested replay template returned a non-challenge StreamGenerate body before bootstrap refresh; deferring asset resolution to follow-up extraction"
                        );
                        return Ok(StreamGenerateReplayOutcome::Completed(body_text));
                    }
                    return Ok(StreamGenerateReplayOutcome::Completed(body_text));
                }
                debug!(
                    provider,
                    status,
                    content_type = content_type.as_deref().unwrap_or("<none>"),
                    "gemini canvas pure HTTP image harvested replay template failed before bootstrap refresh; continuing with bootstrap + preflights"
                );
            }
        }
        if is_text_mode {
            if let Some(template) = replay_template.as_ref() {
                let mut headers = HeaderMap::new();
                apply_gemini_canvas_replay_template_headers(
                    &mut headers,
                    &template.headers,
                    &session,
                );
                debug!(
                    provider,
                    url = %template.url,
                    "sending gemini canvas pure HTTP StreamGenerate request via harvested replay template"
                );
                let response = self
                    .http
                    .request(Method::POST, &template.url)
                    .headers(headers)
                    .query(&template.query)
                    .timeout(timeout.max(Duration::from_secs(120)))
                    .form(&template.form)
                    .send()
                    .await
                    .map_err(|error| classify_network_error(&error, Some(provider)))?;
                let status = response.status().as_u16();
                let content_type = response
                    .headers()
                    .get(rquest::header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string);
                let body_text = self
                    .collect_gemini_canvas_stream_generate_body(
                        response,
                        provider,
                        gemini_canvas::GeminiCanvasMediaOperation::Image,
                        is_image_edit_request,
                    )
                    .await?;
                if !(200..300).contains(&status)
                    || gemini_web::response_indicates_browser_challenge(
                        status,
                        content_type.as_deref(),
                        &body_text,
                    )
                    || gemini_web::response_indicates_session_invalid(
                        status,
                        content_type.as_deref(),
                        &body_text,
                    )
                {
                    return Err(classify_gemini_canvas_pure_http_error(
                        status,
                        content_type.as_deref(),
                        &body_text,
                    ));
                }

                return Ok(StreamGenerateReplayOutcome::Completed(body_text));
            }
        }
        Ok(StreamGenerateReplayOutcome::ContinueBootstrap)
    }
}
