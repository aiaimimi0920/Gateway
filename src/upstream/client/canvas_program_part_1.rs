use super::*;

impl UpstreamClient {
    pub(super) async fn execute_gemini_canvas_program_preview_no_key_video(
        &self,
        req: &CanonicalRelayRequest,
        model: &str,
        canvas_base_url: &str,
        program_context: &GeminiCanvasProgramAppEndpointApiContext,
        program_model_override: Option<&str>,
        invoke_contract: &gemini_canvas_program_web_reverse_modular::GeminiCanvasProgramAppInvokeContract,
    ) -> Result<Value, GatewayError> {
        let provider = "gemini_canvas_compatible";
        if gemini_canvas::requested_output_count(req) > 1 {
            return Err(gemini_canvas_video_unsupported_count_error(provider));
        }

        let prompt = invoke_contract
            .prompt
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or(
                gemini_web_reverse_modular::prompt_for_legacy_mixed_lane_media_request(
                    req,
                    gemini_canvas::GeminiCanvasMediaOperation::Video,
                )?,
            );
        let resolved_aspect_ratio =
            if req.raw_body.get("size").is_some() || req.raw_body.get("aspect_ratio").is_some() {
                gemini_canvas::aspect_ratio_from_request(req)
            } else {
                invoke_contract
                    .aspect_ratio
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| gemini_canvas::aspect_ratio_from_request(req))
            };
        let runtime = gemini_canvas::runtime_from_payload(&program_context.runtime_api.payload)?;
        let uses_page_owned_stream_generate = matches!(
            invoke_contract.request_envelope_kind.as_deref(),
            Some("page_stream_generate_form")
        ) || matches!(
            invoke_contract.transport_kind.as_deref(),
            Some("program_video_streamgenerate_candidate")
        ) || invoke_contract
            .request_url
            .as_deref()
            .map(|value| value.contains("/StreamGenerate"))
            .unwrap_or(false);
        if uses_page_owned_stream_generate {
            let mut template =
                gemini_canvas_program_web_reverse_modular::build_program_stream_generate_request_from_invoke_contract(
                    invoke_contract,
                    Some(&prompt),
                )?
                .ok_or_else(|| {
                    gemini_canvas_program_video_no_key_request_contract_missing_error(provider)
                })?;
            template
                .headers
                .insert("accept".to_string(), "*/*".to_string());
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

            let replay_result = self
                .execute_gemini_canvas_http_replay_worker(
                    provider,
                    &template,
                    &program_context.runtime_api.session,
                    Some("video"),
                    self.timeout.max(Duration::from_secs(45)),
                )
                .await?;
            let body_text = replay_result.body_text;
            let locator_hint = gemini_canvas::extract_stream_generate_locator(&body_text).ok();
            let conversation_id_hint = locator_hint
                .as_ref()
                .map(|locator| locator.conversation_id.as_str())
                .or(program_context
                    .relay_config
                    .app_endpoint
                    .conversation_id
                    .as_deref());
            let response_id_hint = locator_hint
                .as_ref()
                .map(|locator| locator.response_id.as_str())
                .or(program_context
                    .relay_config
                    .app_endpoint
                    .response_id
                    .as_deref());
            let app_path_hint = locator_hint
                .as_ref()
                .map(|locator| locator.app_path.as_str())
                .or(program_context
                    .relay_config
                    .app_endpoint
                    .app_path
                    .as_deref());
            let video_pending_hint = body_text.contains("video_placeholder")
                || gemini_canvas::response_indicates_video_generation_pending(&body_text);
            let busy_hint = gemini_canvas_body_indicates_context_busy(&body_text);
            let job_id_hint = gemini_canvas::extract_video_generation_job_id(&body_text);

            let request_started_at = SystemTime::now();
            let followup_result = self
                .extract_gemini_canvas_media_assets_with_followup(
                    &program_context.runtime_api.payload,
                    model,
                    &runtime,
                    gemini_canvas::GeminiCanvasMediaOperation::Video,
                    &prompt,
                    &body_text,
                    request_started_at,
                    self.timeout.max(Duration::from_secs(120)),
                    false,
                    None,
                )
                .await;
            let (assets, resolved_body_text) = match followup_result {
                Ok(result) => result,
                Err(error) if video_pending_hint || busy_hint => {
                    debug!(
                        provider,
                        app_path = app_path_hint.unwrap_or("<none>"),
                        conversation_id = conversation_id_hint.unwrap_or("<none>"),
                        response_id = response_id_hint.unwrap_or("<none>"),
                        error = %summarize_gateway_error(&error),
                        "gemini canvas page-owned video StreamGenerate reached pending/busy state before exposing a real asset; returning accepted response"
                    );
                    return Ok(gemini_canvas::build_video_generation_accepted_response(
                        model,
                        &prompt,
                        conversation_id_hint,
                        response_id_hint,
                        app_path_hint,
                        job_id_hint.as_deref(),
                        Some(&body_text),
                    ));
                }
                Err(error) => return Err(error),
            };
            let asset = assets.first().ok_or_else(|| {
                build_gemini_canvas_direct_http_video_missing_asset_error(provider)
            })?;
            if gemini_canvas::video_body_indicates_music_modality_mismatch(&resolved_body_text) {
                return Err(gemini_canvas_video_music_modality_mismatch_error(provider));
            }
            if (video_pending_hint || busy_hint)
                && !gemini_canvas_video_body_has_usable_asset(&resolved_body_text)
            {
                return Ok(gemini_canvas::build_video_generation_accepted_response(
                    model,
                    &prompt,
                    conversation_id_hint,
                    response_id_hint,
                    app_path_hint,
                    job_id_hint.as_deref(),
                    Some(&resolved_body_text),
                ));
            }
            return Ok(gemini_canvas::build_video_generation_response(
                model,
                &prompt,
                asset,
                Some(&resolved_body_text),
            ));
        }

        let mut parameters = serde_json::Map::new();
        parameters.insert("aspectRatio".to_string(), json!(resolved_aspect_ratio));
        if let Some(duration_seconds) = invoke_contract.duration_seconds {
            if req.raw_body.get("duration_s").is_none()
                && req.raw_body.get("durationSeconds").is_none()
                && req.raw_body.get("duration").is_none()
            {
                parameters.insert("durationSeconds".to_string(), json!(duration_seconds));
            }
        }
        let request_body = json!({
            "instances": [{
                "prompt": prompt
            }],
            "parameters": parameters
        });

        let browser_pool_base_url = self.ensure_gemini_canvas_browser_pool(provider).await?;
        let preview_model = gemini_canvas::resolve_video_model(model)?;
        let official_model = program_model_override
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| {
                gemini_canvas::resolve_official_video_model(model)
                    .unwrap_or(gemini_canvas::GEMINI_CANVAS_OFFICIAL_VIDEO_MODEL)
                    .to_string()
            });
        let candidate_request_urls = vec![
            format!(
                "{}/models/{}:predictLongRunning",
                gemini_canvas::GEMINI_CANVAS_DIRECT_HTTP_IMAGE_API_BASE_URL.trim_end_matches('/'),
                preview_model
            ),
            format!(
                "{}/models/{}:predictLongRunning",
                program_context
                    .runtime_api
                    .payload
                    .base_url
                    .trim_end_matches('/'),
                official_model
            ),
        ];

        let mut operation_value: Option<Value> = None;
        let mut operation_base_url: Option<String> = None;
        let mut last_error: Option<GatewayError> = None;

        for request_url in candidate_request_urls {
            match self
                .execute_gemini_canvas_connected_fetch_invocation_with_program_context(
                    &program_context.runtime_api.payload,
                    provider,
                    &browser_pool_base_url,
                    canvas_base_url,
                    &program_context.relay_config,
                    None,
                    None,
                    &request_url,
                    Method::POST,
                    Some(&request_body),
                    "canvas_page_no_key",
                    self.timeout.max(Duration::from_secs(30)),
                )
                .await
            {
                Ok(invocation) => {
                    let body = invocation.body_text.as_deref().ok_or_else(|| {
                        gemini_canvas_program_video_no_key_empty_body_error(provider)
                    })?;
                    let body_json: Value = serde_json::from_str(body).map_err(|error| {
                        gemini_canvas_program_video_no_key_invalid_json_error(
                            provider,
                            error.to_string().as_str(),
                        )
                    })?;
                    if (200..300).contains(&invocation.status) {
                        operation_base_url = request_url
                            .split_once("/models/")
                            .map(|(prefix, _)| prefix.to_string())
                            .or_else(|| {
                                origin_from_url(&request_url)
                                    .map(|origin| format!("{origin}/v1beta"))
                            });
                        operation_value = Some(body_json);
                        break;
                    }
                    last_error = Some(classify_upstream_error(
                        invocation.status,
                        body,
                        Some(provider),
                    ));
                }
                Err(error) => {
                    last_error = Some(error);
                }
            }
        }

        let operation_value = match operation_value {
            Some(value) => value,
            None => {
                return Err(last_error.unwrap_or_else(|| {
                    gemini_canvas_program_video_no_key_request_exhausted_error(provider)
                }));
            }
        };
        let operation_name = operation_value
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| gemini_canvas_video_missing_operation_error(provider))?;
        let operation_url =
            if operation_name.starts_with("http://") || operation_name.starts_with("https://") {
                operation_name.to_string()
            } else {
                format!(
                    "{}/{}",
                    operation_base_url.unwrap_or_else(|| {
                        program_context
                            .runtime_api
                            .payload
                            .base_url
                            .trim_end_matches('/')
                            .to_string()
                    }),
                    operation_name.trim_start_matches('/')
                )
            };

        let deadline = std::time::Instant::now() + self.timeout.max(Duration::from_secs(600));
        let mut poll = operation_value.clone();
        while std::time::Instant::now() < deadline {
            if poll.get("done").and_then(Value::as_bool).unwrap_or(false) {
                break;
            }
            sleep(Duration::from_secs(5)).await;
            let invocation = self
                .execute_gemini_canvas_connected_fetch_invocation_with_program_context(
                    &program_context.runtime_api.payload,
                    provider,
                    &browser_pool_base_url,
                    canvas_base_url,
                    &program_context.relay_config,
                    None,
                    None,
                    &operation_url,
                    Method::GET,
                    None,
                    "canvas_page_no_key",
                    self.timeout.max(Duration::from_secs(30)),
                )
                .await?;
            let body = invocation.body_text.as_deref().ok_or_else(|| {
                gemini_canvas_program_video_no_key_poll_empty_body_error(provider)
            })?;
            if !(200..300).contains(&invocation.status) {
                return Err(classify_upstream_error(
                    invocation.status,
                    body,
                    Some(provider),
                ));
            }
            poll = serde_json::from_str(body).map_err(|error| {
                gemini_canvas_program_video_no_key_poll_invalid_json_error(
                    provider,
                    error.to_string().as_str(),
                )
            })?;
        }
        if !poll.get("done").and_then(Value::as_bool).unwrap_or(false) {
            return Err(gemini_canvas_video_operation_timeout_error(provider));
        }
        if let Some(error) = poll.get("error") {
            return Err(classify_upstream_error(
                502,
                &error.to_string(),
                Some(provider),
            ));
        }
        let video_uri = extract_gemini_canvas_video_operation_download_uri(provider, &poll)?;
        let (bytes, content_type) = self
            .execute_gemini_canvas_connected_fetch_get_bytes_with_program_context(
                &program_context.runtime_api.payload,
                provider,
                &browser_pool_base_url,
                canvas_base_url,
                &program_context.relay_config,
                None,
                None,
                video_uri,
                "canvas_page_no_key",
                self.timeout.max(Duration::from_secs(120)),
            )
            .await?;
        let mime_type = content_type.unwrap_or_else(|| "video/mp4".to_string());
        let body_base64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let asset = gemini_canvas::GeminiCanvasMediaAsset {
            kind: "video".to_string(),
            url: video_uri.to_string(),
            mime_type,
            download_token: None,
            body_base64: Some(body_base64),
            alt: Some(prompt.clone()),
            width: None,
            height: None,
            duration_seconds: None,
        };
        Ok(gemini_canvas::build_video_generation_response(
            model, &prompt, &asset, None,
        ))
    }
}
