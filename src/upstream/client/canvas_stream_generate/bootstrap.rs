use super::*;

impl UpstreamClient {
    pub(super) async fn bootstrap_gemini_canvas_stream_generate(
        &self,
        prepared: StreamGeneratePreparation,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        mode_index: i64,
        timeout: Duration,
        allow_replay_template: bool,
        image_edit_followup_context: &mut Option<&mut GeminiCanvasImageEditFollowupContext>,
    ) -> Result<StreamGenerateBootstrapOutcome, GatewayError> {
        if let StreamGenerateReplayOutcome::Completed(body) = self
            .replay_gemini_canvas_stream_generate_before_bootstrap(
                &prepared,
                runtime,
                timeout,
                allow_replay_template,
                image_edit_followup_context,
            )
            .await?
        {
            return Ok(StreamGenerateBootstrapOutcome::Completed(body));
        }

        let StreamGeneratePreparation {
            effective_base_url,
            app_bootstrap_url,
            share_bootstrap_url,
            is_text_mode,
            is_image_mode,
            is_image_edit_request,
            image_stream_timeout,
            bootstrap_url,
            stream_url,
            storage_state,
            batchexecute_header_id,
            replay_template,
            session,
        } = prepared;
        let provider = "gemini_canvas_compatible";
        let session_target_url = replay_template
            .as_ref()
            .map(|template| template.url.as_str())
            .unwrap_or_else(|| {
                if is_text_mode || is_image_mode {
                    app_bootstrap_url.as_str()
                } else {
                    share_bootstrap_url.as_str()
                }
            });
        let origin = gemini_canvas_http_origin(payload);
        let authorization = gemini_canvas::build_sapisid_authorization(
            &session.sapisid,
            &origin,
            current_unix_timestamp_i64(),
        )?;
        let (bootstrap_body, _bootstrap_request_contract, bootstrap_page_url) = if is_text_mode {
            let mut headers = HeaderMap::new();
            apply_gemini_canvas_navigation_headers(&mut headers);
            apply_gemini_canvas_cookie_header(&mut headers, &session);
            insert_header_map_value(
                &mut headers,
                "accept-language",
                &gemini_canvas::locale_from_payload(payload),
            );
            let bootstrap_request_contract =
                build_gemini_canvas_direct_http_text_bootstrap_request_contract(
                    &bootstrap_url,
                    &headers,
                );

            let bootstrap_response = self
                .http
                .request(Method::GET, &bootstrap_url)
                .headers(headers)
                .timeout(timeout.max(Duration::from_secs(30)))
                .send()
                .await
                .map_err(|error| classify_network_error(&error, Some(provider)))?;
            let bootstrap_final_url = bootstrap_response.url().to_string();
            let bootstrap_status = bootstrap_response.status().as_u16();
            let bootstrap_location = bootstrap_response
                .headers()
                .get(rquest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            let bootstrap_content_type = bootstrap_response
                .headers()
                .get(rquest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            let bootstrap_body = bootstrap_response
                .text()
                .await
                .map_err(|error| classify_network_error(&error, Some(provider)))?;
            let bootstrap_response_meta = build_gemini_canvas_direct_http_bootstrap_response_meta(
                &bootstrap_final_url,
                bootstrap_location.as_deref(),
                bootstrap_content_type.as_deref(),
                &bootstrap_body,
            );
            if !(200..300).contains(&bootstrap_status)
                || gemini_web::response_indicates_browser_challenge(
                    bootstrap_status,
                    bootstrap_content_type.as_deref(),
                    &bootstrap_body,
                )
                || gemini_web::response_indicates_session_invalid(
                    bootstrap_status,
                    bootstrap_content_type.as_deref(),
                    &bootstrap_body,
                )
            {
                return Err(
                    build_gemini_canvas_direct_http_text_bootstrap_failure_error(
                        bootstrap_status,
                        bootstrap_content_type.as_deref(),
                        &bootstrap_body,
                        &bootstrap_request_contract,
                        &bootstrap_response_meta,
                    ),
                );
            }

            (
                bootstrap_body,
                bootstrap_request_contract,
                bootstrap_final_url,
            )
        } else {
            let bootstrap_candidates = gemini_canvas_direct_http_bootstrap_candidates(
                payload,
                &effective_base_url,
                &runtime.share_id,
                is_image_mode,
            );
            let mut failures = Vec::new();
            let mut last_error = None;
            let mut selected_bootstrap: Option<(String, String)> = None;

            for candidate_url in &bootstrap_candidates {
                match self
                    .fetch_gemini_canvas_direct_http_page_html(
                        payload,
                        &session,
                        candidate_url,
                        timeout,
                    )
                    .await
                {
                    Ok(body) => {
                        selected_bootstrap = Some((body, candidate_url.to_string()));
                        break;
                    }
                    Err(error) => {
                        failures.push(format!(
                            "{}: {}",
                            candidate_url,
                            summarize_gateway_error(&error)
                        ));
                        last_error = Some(error);
                    }
                }
            }

            if let Some((body, selected_url)) = selected_bootstrap {
                let bootstrap_request_contract =
                    build_gemini_canvas_direct_http_page_harvest_bootstrap_request_contract(
                        &selected_url,
                        &bootstrap_candidates,
                        session_target_url,
                        session.cookie_header.len(),
                    );
                (body, bootstrap_request_contract, selected_url)
            } else {
                return Err(build_gemini_canvas_direct_http_page_harvest_failure_error(
                    last_error,
                    &bootstrap_candidates,
                    session_target_url,
                    &failures,
                ));
            }
        };

        let fallback_bootstrap =
            gemini_web::bootstrap_from_payload_cache(payload.extra_body.as_ref());
        let mut bootstrap = gemini_web::parse_bootstrap_from_app_html(
            &bootstrap_body,
            payload
                .extra_body
                .as_ref()
                .and_then(|extra| extra.get("language").and_then(Value::as_str)),
        )
        .or_else(|primary_error| fallback_bootstrap.clone().ok_or(primary_error))?;
        bootstrap =
            gemini_web::merge_bootstrap_from_fallback(bootstrap, fallback_bootstrap.as_ref());
        if is_image_edit_request {
            if let Some(locale) = gemini_canvas::harvest_image_edit_template_locale(&storage_state)
            {
                bootstrap.language = locale;
            }
        }
        let page_path =
            resolve_gemini_canvas_direct_http_bootstrap_page_path(&bootstrap_page_url, &bootstrap);
        let text_preflight_source_path =
            resolve_gemini_canvas_direct_http_preflight_source_path(payload);

        let referer = resolve_gemini_canvas_direct_http_referer(
            &effective_base_url,
            &page_path,
            &text_preflight_source_path,
            is_text_mode,
            is_image_mode,
        );
        let harvested_stream_generate_model_header =
            gemini_canvas::build_stream_generate_model_header_from_storage_state(
                &storage_state,
                mode_index,
                is_image_edit_request,
            );
        let model_header = resolve_gemini_canvas_direct_http_stream_generate_model_header(
            harvested_stream_generate_model_header.as_deref(),
            is_text_mode,
            is_image_mode,
        );
        Ok(StreamGenerateBootstrap {
            preparation: StreamGeneratePreparation {
                effective_base_url,
                app_bootstrap_url,
                share_bootstrap_url,
                is_text_mode,
                is_image_mode,
                is_image_edit_request,
                image_stream_timeout,
                bootstrap_url,
                stream_url,
                storage_state,
                batchexecute_header_id,
                replay_template,
                session,
            },
            bootstrap,
            origin,
            authorization,
            page_path,
            text_preflight_source_path,
            referer,
            model_header: model_header.to_string(),
        })
        .map(StreamGenerateBootstrapOutcome::Continue)
    }
}
