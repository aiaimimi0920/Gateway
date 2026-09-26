use super::*;

impl UpstreamClient {
    pub(super) async fn execute_gemini_canvas_program_app_endpoint_media_direct_http_with_context(
        &self,
        req: &CanonicalRelayRequest,
        model: &str,
        operation: gemini_canvas::GeminiCanvasMediaOperation,
        program_context: GeminiCanvasProgramAppEndpointApiContext,
    ) -> Result<Value, GatewayError> {
        let app_path = program_context
            .relay_config
            .app_endpoint
            .app_path
            .as_deref()
            .unwrap_or("<none>");
        let canvas_base_url = program_context
            .relay_config
            .app_endpoint
            .canvas_program_url
            .as_deref()
            .or(program_context
                .relay_config
                .app_endpoint
                .page_url
                .as_deref())
            .and_then(origin_from_url)
            .unwrap_or_else(|| "https://gemini.google.com".to_string());
        let invoke_contract =
            gemini_canvas_program_web_reverse_modular::preferred_app_endpoint_invoke_contract(
                operation,
                match operation {
                    gemini_canvas::GeminiCanvasMediaOperation::Music => {
                        "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1alpha.GenerativeService.BidiGenerateMusic"
                    }
                    gemini_canvas::GeminiCanvasMediaOperation::Video => {
                        &program_context.invoke_base_url
                    }
                    gemini_canvas::GeminiCanvasMediaOperation::Image => "",
                },
                (operation == gemini_canvas::GeminiCanvasMediaOperation::Video)
                    .then(|| gemini_canvas::resolve_official_video_model(model))
                    .transpose()?,
                &program_context.relay_config,
            );
        let program_model_override =
            gemini_canvas_program_web_reverse_modular::preferred_app_endpoint_model_name(
                &invoke_contract,
            );
        let runtime_api_result = match operation {
            gemini_canvas::GeminiCanvasMediaOperation::Music => {
                if invoke_contract
                    .asset_url
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .is_some()
                {
                    self.execute_gemini_canvas_program_preview_no_key_music(
                        req,
                        model,
                        &program_context,
                        &invoke_contract,
                    )
                    .await
                } else if matches!(
                    invoke_contract.request_envelope_kind.as_deref(),
                    Some("page_stream_generate_form")
                ) || matches!(
                    invoke_contract.transport_kind.as_deref(),
                    Some("program_music_streamgenerate_candidate")
                ) {
                    self.execute_gemini_canvas_program_preview_no_key_music(
                        req,
                        model,
                        &program_context,
                        &invoke_contract,
                    )
                    .await
                } else if matches!(
                    invoke_contract.request_envelope_kind.as_deref(),
                    Some("canvas_proxy_request")
                ) || matches!(
                    invoke_contract.transport_kind.as_deref(),
                    Some("canvas_program_ws_candidate")
                ) {
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
                    let upstream_model = program_model_override
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_string)
                        .unwrap_or_else(|| {
                            gemini_canvas::resolve_music_model(model)
                                .unwrap_or(gemini_canvas::GEMINI_CANVAS_MUSIC_PREVIEW_MODEL)
                                .to_string()
                        });
                    let mut request_body = json!({
                        "setup": {
                            "model": format!("models/{upstream_model}")
                        },
                        "client_content": gemini_canvas::build_music_client_content(&prompt),
                        "playback_control": "PLAY"
                    });
                    let music_generation_config = gemini_canvas::build_music_generation_config(req);
                    if music_generation_config
                        .as_object()
                        .map(|config| !config.is_empty())
                        .unwrap_or(false)
                    {
                        request_body["music_generation_config"] = music_generation_config;
                    }
                    let browser_pool_base_url = self
                        .ensure_gemini_canvas_browser_pool("gemini_canvas_compatible")
                        .await?;
                    let invocation = self
                        .execute_gemini_canvas_connected_fetch_invocation_with_program_context(
                            &program_context.runtime_api.payload,
                            "gemini_canvas_compatible",
                            &browser_pool_base_url,
                            &canvas_base_url,
                            &program_context.relay_config,
                            None,
                            None,
                            "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1alpha.GenerativeService.BidiGenerateMusic",
                            Method::POST,
                            Some(&request_body),
                            "canvas_page_music_no_key",
                            self.timeout.max(Duration::from_secs(60)),
                        )
                        .await?;
                    let audio = decode_gemini_canvas_program_music_no_key_audio(
                        invocation.body_base64.as_deref(),
                        invocation.content_type.as_deref(),
                    )?;
                    let (body, content_type) =
                        gemini_canvas::build_audio_binary_response(req, &audio)?;
                    let asset_body_base64 = base64::engine::general_purpose::STANDARD.encode(&body);
                    let asset = gemini_canvas::GeminiCanvasMediaAsset {
                        kind: "audio".to_string(),
                        url: format!("data:{content_type};base64,{asset_body_base64}"),
                        mime_type: content_type,
                        download_token: None,
                        body_base64: Some(asset_body_base64),
                        alt: Some(prompt.clone()),
                        width: None,
                        height: None,
                        duration_seconds: None,
                    };
                    Ok(gemini_canvas::build_music_generation_response(
                        model, &prompt, &asset, None,
                    ))
                } else {
                    let socket_url = invoke_contract
                        .music_ws_url
                        .as_deref()
                        .ok_or_else(gemini_canvas_program_music_invoke_target_missing_error)?;
                    execute_gemini_canvas_official_music(
                        self.timeout,
                        &program_context.runtime_api.payload,
                        req,
                        model,
                        Some(&program_context.runtime_api),
                        Some(&program_context.official_extra_headers),
                        Some(socket_url),
                        invoke_contract.prompt.as_deref(),
                        invoke_contract.duration_seconds,
                        program_model_override.as_deref(),
                    )
                    .await
                }
            }
            gemini_canvas::GeminiCanvasMediaOperation::Video => {
                if matches!(
                    invoke_contract.request_envelope_kind.as_deref(),
                    Some("canvas_proxy_request") | Some("page_stream_generate_form")
                ) || matches!(
                    invoke_contract.transport_kind.as_deref(),
                    Some("canvas_program_ws_candidate")
                        | Some("program_video_streamgenerate_candidate")
                ) {
                    self.execute_gemini_canvas_program_preview_no_key_video(
                        req,
                        model,
                        &canvas_base_url,
                        &program_context,
                        program_model_override.as_deref(),
                        &invoke_contract,
                    )
                    .await
                } else {
                    let request_url = invoke_contract
                        .video_request_url
                        .as_deref()
                        .ok_or_else(gemini_canvas_program_video_invoke_target_missing_error)?;
                    execute_gemini_canvas_official_video(
                        &self.http,
                        self.timeout,
                        &program_context.runtime_api.payload,
                        req,
                        model,
                        Some(&program_context.official_extra_headers),
                        Some(request_url),
                        invoke_contract.prompt.as_deref(),
                        invoke_contract.aspect_ratio.as_deref(),
                        invoke_contract.duration_seconds,
                        program_model_override.as_deref(),
                    )
                    .await
                }
            }
            gemini_canvas::GeminiCanvasMediaOperation::Image => unreachable!(),
        };
        runtime_api_result.map_err(|error| {
            debug!(
                provider = "gemini_canvas_compatible",
                operation = ?operation,
                app_path,
                error = %summarize_gateway_error(&error),
                "gemini canvas program app-endpoint media lane returned an error"
            );
            error
        })
    }
}
