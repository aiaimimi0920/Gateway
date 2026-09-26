use super::*;

impl UpstreamClient {
    pub(super) async fn execute_gemini_canvas_program_preview_no_key_music(
        &self,
        req: &CanonicalRelayRequest,
        model: &str,
        program_context: &GeminiCanvasProgramAppEndpointApiContext,
        invoke_contract: &gemini_canvas_program_web_reverse_modular::GeminiCanvasProgramAppInvokeContract,
    ) -> Result<Value, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let timeout = self.timeout.max(Duration::from_secs(45));
        let request_started_at = SystemTime::now();
        let prompt = invoke_contract
            .prompt
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or(
                gemini_web_reverse_modular::prompt_for_legacy_mixed_lane_media_request(
                    req,
                    gemini_canvas::GeminiCanvasMediaOperation::Music,
                )?,
            );
        let runtime = gemini_canvas::runtime_from_payload(&program_context.runtime_api.payload)?;
        if let Some(asset_url) = invoke_contract
            .asset_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            match self
                .materialize_gemini_canvas_direct_http_media_asset(
                    &program_context.runtime_api.payload,
                    &runtime,
                    asset_url,
                    invoke_contract.asset_kind.as_deref(),
                    invoke_contract.asset_mime_type.as_deref(),
                    timeout,
                )
                .await
            {
                Ok(asset) => {
                    return Ok(gemini_canvas::build_music_generation_response(
                        model,
                        &prompt,
                        &asset,
                        Some("music_player_ready"),
                    ));
                }
                Err(error) => {
                    debug!(
                        provider,
                        asset_url,
                        error = %summarize_gateway_error(&error),
                        "gemini canvas no-key music invoke contract exposed a final asset target, but direct materialization failed; continuing browserless StreamGenerate replay path"
                    );
                }
            }
        }
        let mut template =
            gemini_canvas_program_web_reverse_modular::build_program_stream_generate_request_from_invoke_contract(
                invoke_contract,
                Some(&prompt),
            )?
            .ok_or_else(|| {
                gemini_canvas_program_music_no_key_request_contract_missing_error(provider)
            })?;
        template
            .headers
            .insert("accept".to_string(), "*/*".to_string());
        template.headers.insert(
            "content-type".to_string(),
            "application/x-www-form-urlencoded;charset=UTF-8".to_string(),
        );
        template.headers.insert(
            "accept-language".to_string(),
            format!(
                "{},zh;q=0.9,en;q=0.8",
                gemini_canvas::locale_from_payload(&program_context.runtime_api.payload)
            ),
        );
        template.headers.insert(
            "authorization".to_string(),
            gemini_canvas::build_sapisid_authorization(
                &program_context.runtime_api.session.sapisid,
                &program_context.runtime_api.page_origin,
                current_unix_timestamp_i64(),
            )?,
        );
        template.headers.insert(
            "origin".to_string(),
            program_context.runtime_api.page_origin.clone(),
        );
        template.headers.insert(
            "referer".to_string(),
            program_context.runtime_api.page_referer.clone(),
        );
        template.headers.insert(
            "user-agent".to_string(),
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 Edg/143.0.0.0".to_string(),
        );
        template.headers.insert(
            "x-goog-authuser".to_string(),
            program_context.runtime_api.session.auth_user.clone(),
        );
        template.headers.insert(
            "x-origin".to_string(),
            program_context.runtime_api.page_origin.clone(),
        );
        template
            .headers
            .insert("x-same-domain".to_string(), "1".to_string());
        template
            .headers
            .insert("sec-fetch-site".to_string(), "same-origin".to_string());
        template
            .headers
            .insert("sec-fetch-mode".to_string(), "cors".to_string());
        template
            .headers
            .insert("sec-fetch-dest".to_string(), "empty".to_string());
        template.headers.insert(
            "sec-ch-ua".to_string(),
            "\"Microsoft Edge\";v=\"143\", \"Chromium\";v=\"143\", \"Not_A Brand\";v=\"24\""
                .to_string(),
        );
        template
            .headers
            .insert("sec-ch-ua-mobile".to_string(), "?0".to_string());
        template
            .headers
            .insert("sec-ch-ua-platform".to_string(), "\"Windows\"".to_string());
        let session_cookie_hash = {
            let digest = <sha1::Sha1 as sha1::Digest>::digest(
                program_context.runtime_api.session.cookie_header.as_bytes(),
            );
            hex::encode(digest)
        };
        debug!(
            provider,
            cookie_header_len = program_context.runtime_api.session.cookie_header.len(),
            cookie_count = program_context
                .runtime_api
                .session
                .cookie_header
                .split(';')
                .filter(|value| !value.trim().is_empty())
                .count(),
            has_1psid = program_context
                .runtime_api
                .session
                .cookie_header
                .contains("__Secure-1PSID="),
            has_1psidts = program_context
                .runtime_api
                .session
                .cookie_header
                .contains("__Secure-1PSIDTS="),
            has_sapisid = program_context
                .runtime_api
                .session
                .cookie_header
                .contains("SAPISID="),
            auth_user = %program_context.runtime_api.session.auth_user,
            cookie_sha1 = %session_cookie_hash,
            page_referer = %program_context.runtime_api.page_referer,
            request_url = %template.url,
            request_query = ?template.query,
            "gemini canvas no-key music replay session prepared"
        );

        let initial_result = self
            .execute_gemini_canvas_http_replay_worker(
                provider,
                &template,
                &program_context.runtime_api.session,
                Some("music"),
                timeout,
            )
            .await?;
        let initial_body = initial_result.body_text;
        debug!(
            provider,
            body_len = initial_body.len(),
            has_mp3 = initial_body.contains(".mp3"),
            has_audio_mpeg = initial_body.contains("audio/mpeg"),
            has_mp4 = initial_body.contains(".mp4"),
            has_1060 = initial_body.contains("[1060]"),
            has_track_details = initial_body.contains("Track Details"),
            "gemini canvas no-key music initial StreamGenerate replay completed"
        );
        if let Ok((assets, resolved_body)) = self
            .extract_gemini_canvas_media_assets_with_followup(
                &program_context.runtime_api.payload,
                model,
                &runtime,
                gemini_canvas::GeminiCanvasMediaOperation::Music,
                &prompt,
                &initial_body,
                SystemTime::now(),
                timeout,
                false,
                None,
            )
            .await
        {
            if let Some(asset) = select_preferred_gemini_canvas_music_asset(&assets) {
                let asset = self
                    .best_effort_materialize_gemini_canvas_direct_http_media_asset(
                        &program_context.runtime_api.payload,
                        &runtime,
                        asset,
                        timeout,
                        provider,
                    )
                    .await;
                return Ok(gemini_canvas::build_music_generation_response(
                    model,
                    &prompt,
                    &asset,
                    Some(&resolved_body),
                ));
            }
            if let Some(result) = self
                .try_resolve_gemini_canvas_program_preview_no_key_music_followup(
                    &program_context.runtime_api.payload,
                    model,
                    &runtime,
                    &prompt,
                    &resolved_body,
                    request_started_at,
                    timeout,
                    program_context
                        .relay_config
                        .app_endpoint
                        .conversation_id
                        .as_deref(),
                    program_context
                        .relay_config
                        .app_endpoint
                        .response_id
                        .as_deref(),
                    program_context
                        .relay_config
                        .app_endpoint
                        .app_path
                        .as_deref(),
                )
                .await?
            {
                return Ok(result);
            }
            debug!(
                provider,
                asset_count = assets.len(),
                "gemini canvas no-key music initial replay extracted assets but none were selected as final"
            );
        } else {
            let extract_error = gemini_canvas::extract_stream_generate_media_assets(
                &initial_body,
                gemini_canvas::GeminiCanvasMediaOperation::Music,
            )
            .err()
            .map(|error| summarize_gateway_error(&error))
            .unwrap_or_else(|| "<unknown>".to_string());
            debug!(
                provider,
                error = %extract_error,
                "gemini canvas no-key music initial replay did not yield direct music assets"
            );
        }
        if let Some(result) = self
            .try_resolve_gemini_canvas_program_preview_no_key_music_followup(
                &program_context.runtime_api.payload,
                model,
                &runtime,
                &prompt,
                &initial_body,
                request_started_at,
                timeout,
                program_context
                    .relay_config
                    .app_endpoint
                    .conversation_id
                    .as_deref(),
                program_context
                    .relay_config
                    .app_endpoint
                    .response_id
                    .as_deref(),
                program_context
                    .relay_config
                    .app_endpoint
                    .app_path
                    .as_deref(),
            )
            .await?
        {
            return Ok(result);
        }

        let followup_context = self
            .prepare_gemini_canvas_direct_http_media_followup_context(
                &program_context.runtime_api.payload,
                &runtime,
                gemini_canvas::GeminiCanvasMediaOperation::Music,
                &initial_body,
                None,
                timeout,
                false,
                None,
            )
            .await;

        if let Ok(GeminiCanvasMediaFollowupContext {
            bootstrap,
            mut session,
            followup_target,
            ..
        }) = followup_context
        {
            let selected_mode_request = gemini_canvas::build_mode_selection_preflight_request(
                &bootstrap,
                &followup_target.source_path,
                gemini_canvas::GEMINI_CANVAS_TEXT_LAST_SELECTED_MODE_ID,
            )?;
            let model_header = gemini_canvas::build_text_batchexecute_model_header(None, None);
            let _ = self
                .send_gemini_canvas_text_batchexecute_request_refreshing_session(
                    &program_context.runtime_api.payload,
                    model,
                    &selected_mode_request,
                    &mut session,
                    &model_header,
                    timeout,
                )
                .await;

            let replay_after_prelude = self
                .execute_gemini_canvas_http_replay_worker(
                    provider,
                    &template,
                    &session,
                    Some("music"),
                    timeout,
                )
                .await?;
            let replay_after_prelude_body = replay_after_prelude.body_text;
            debug!(
                provider,
                body_len = replay_after_prelude_body.len(),
                has_mp3 = replay_after_prelude_body.contains(".mp3"),
                has_audio_mpeg = replay_after_prelude_body.contains("audio/mpeg"),
                has_mp4 = replay_after_prelude_body.contains(".mp4"),
                has_1060 = replay_after_prelude_body.contains("[1060]"),
                has_track_details = replay_after_prelude_body.contains("Track Details"),
                "gemini canvas no-key music replay after minimal prelude completed"
            );
            if let Ok((assets, resolved_body)) = self
                .extract_gemini_canvas_media_assets_with_followup(
                    &program_context.runtime_api.payload,
                    model,
                    &runtime,
                    gemini_canvas::GeminiCanvasMediaOperation::Music,
                    &prompt,
                    &replay_after_prelude_body,
                    SystemTime::now(),
                    timeout,
                    false,
                    None,
                )
                .await
            {
                if let Some(asset) = select_preferred_gemini_canvas_music_asset(&assets) {
                    let asset = self
                        .best_effort_materialize_gemini_canvas_direct_http_media_asset(
                            &program_context.runtime_api.payload,
                            &runtime,
                            asset,
                            timeout,
                            provider,
                        )
                        .await;
                    return Ok(gemini_canvas::build_music_generation_response(
                        model,
                        &prompt,
                        &asset,
                        Some(&resolved_body),
                    ));
                }
                if let Some(result) = self
                    .try_resolve_gemini_canvas_program_preview_no_key_music_followup(
                        &program_context.runtime_api.payload,
                        model,
                        &runtime,
                        &prompt,
                        &resolved_body,
                        request_started_at,
                        timeout,
                        program_context
                            .relay_config
                            .app_endpoint
                            .conversation_id
                            .as_deref(),
                        program_context
                            .relay_config
                            .app_endpoint
                            .response_id
                            .as_deref(),
                        program_context
                            .relay_config
                            .app_endpoint
                            .app_path
                            .as_deref(),
                    )
                    .await?
                {
                    return Ok(result);
                }
                debug!(
                    provider,
                    asset_count = assets.len(),
                    "gemini canvas no-key music replay after prelude extracted assets but none were selected as final"
                );
            } else {
                let extract_error = gemini_canvas::extract_stream_generate_media_assets(
                    &replay_after_prelude_body,
                    gemini_canvas::GeminiCanvasMediaOperation::Music,
                )
                .err()
                .map(|error| summarize_gateway_error(&error))
                .unwrap_or_else(|| "<unknown>".to_string());
                debug!(
                    provider,
                    error = %extract_error,
                    "gemini canvas no-key music replay after prelude did not yield direct music assets"
                );
            }
            if let Some(result) = self
                .try_resolve_gemini_canvas_program_preview_no_key_music_followup(
                    &program_context.runtime_api.payload,
                    model,
                    &runtime,
                    &prompt,
                    &replay_after_prelude_body,
                    request_started_at,
                    timeout,
                    program_context
                        .relay_config
                        .app_endpoint
                        .conversation_id
                        .as_deref(),
                    program_context
                        .relay_config
                        .app_endpoint
                        .response_id
                        .as_deref(),
                    program_context
                        .relay_config
                        .app_endpoint
                        .app_path
                        .as_deref(),
                )
                .await?
            {
                return Ok(result);
            }
            if replay_after_prelude_body.contains("[1060]") {
                return Err(
                    gemini_canvas_program_music_no_key_prelude_stage_incomplete_error(provider),
                );
            }
            return Err(
                gemini_canvas_program_music_no_key_post_1060_contract_missing_error(provider),
            );
        }

        if initial_body.contains("[1060]") {
            return Err(
                gemini_canvas_program_music_no_key_browserless_stage_incomplete_error(provider),
            );
        }
        Err(gemini_canvas_program_music_no_key_contract_missing_error(
            provider,
        ))
    }
}
