use super::*;

impl UpstreamClient {
    pub(super) async fn prepare_gemini_canvas_stream_generate(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        mode_index: i64,
        prompt: &str,
        timeout: Duration,
        allow_replay_template: bool,
        image_edit_uploads: Option<&[gemini_canvas::GeminiCanvasImageEditUpload]>,
        image_edit_followup_context: &mut Option<&mut GeminiCanvasImageEditFollowupContext>,
    ) -> Result<StreamGeneratePreparation, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let configured_base_url = payload.base_url.trim_end_matches('/');
        let effective_base_url = if configured_base_url.is_empty() {
            gemini_canvas_http_origin(payload)
        } else {
            configured_base_url.to_string()
        };
        let app_bootstrap_url = if payload.adapter == "gemini_canvas_program_web_reverse_compatible"
        {
            gemini_canvas_program_web_reverse_modular::gemini_canvas_program_payload_page_url(
                payload,
                &effective_base_url,
            )
            .unwrap_or_else(|| {
                format!(
                    "{}{}",
                    effective_base_url,
                    gemini_web::GEMINI_WEB_DEFAULT_APP_PATH
                )
            })
        } else {
            format!(
                "{}{}",
                effective_base_url,
                gemini_web::GEMINI_WEB_DEFAULT_APP_PATH
            )
        };
        let share_bootstrap_url =
            gemini_canvas::direct_http_referrer(&effective_base_url, &runtime.share_id);
        let is_text_mode =
            mode_index == gemini_canvas::GEMINI_CANVAS_TEXT_STREAM_GENERATE_TEXT_MODE_INDEX;
        let is_image_mode =
            mode_index == gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX;
        let is_image_edit_request = is_image_mode
            && image_edit_uploads
                .map(|uploads| !uploads.is_empty())
                .unwrap_or(false);
        let image_stream_timeout = if is_image_edit_request {
            // Real successful image-edit captures can spend multiple minutes in the
            // page-owned async generation flow before the final asset frames land.
            // The latest broad captures keep the same StreamGenerate request open
            // for roughly 9 minutes before downstream signaler/page settlement
            // has fully converged. Earlier 45s / 120s / 240-300s caps caused us
            // to preserve only the initial metadata, progress, or short-ack
            // frames, which looked like a completed response even though the
            // upstream stream was still active.
            timeout
                .min(Duration::from_secs(900))
                .max(Duration::from_secs(660))
        } else {
            timeout.max(Duration::from_secs(120))
        };
        let bootstrap_url = app_bootstrap_url.clone();
        let stream_url = format!(
            "{effective_base_url}{}",
            gemini_web::GEMINI_WEB_DEFAULT_STREAM_GENERATE_PATH
        );
        let object_storage = gateway_object_storage()?;
        let mut storage_state = object_storage
            .read_json(&runtime.runtime_state_object_key)
            .await?;
        if is_image_edit_request
            && storage_state
                .get("imageEditStreamGenerateTemplate")
                .is_none()
        {
            let sidecar_key = gemini_canvas::image_edit_stream_generate_template_object_key(
                &runtime.runtime_state_object_key,
            );
            let remote_sidecar = object_storage.read_json(&sidecar_key).await.ok();
            let local_sidecar = read_gemini_canvas_runtime_mirror_json(&sidecar_key);
            let sidecar = if remote_sidecar
                .as_ref()
                .map(|value| {
                    gemini_canvas_sidecar_has_any_key(
                        value,
                        &["imageEditStreamGenerateTemplate", "imageEditStreamTemplate"],
                    )
                })
                .unwrap_or(false)
            {
                remote_sidecar
            } else if local_sidecar
                .as_ref()
                .map(|value| {
                    gemini_canvas_sidecar_has_any_key(
                        value,
                        &["imageEditStreamGenerateTemplate", "imageEditStreamTemplate"],
                    )
                })
                .unwrap_or(false)
            {
                local_sidecar
            } else {
                remote_sidecar.or(local_sidecar)
            };
            if let Some(sidecar) = sidecar {
                let maybe_template = sidecar
                    .get("imageEditStreamGenerateTemplate")
                    .cloned()
                    .or_else(|| {
                        sidecar
                            .get("url")
                            .and_then(Value::as_str)
                            .map(|_| sidecar.clone())
                    });
                if let Some(template) = maybe_template {
                    if let Some(root) = storage_state.as_object_mut() {
                        root.insert("imageEditStreamGenerateTemplate".to_string(), template);
                    }
                }
            }
        }
        if is_image_mode && storage_state.get("imageStreamGenerateTemplate").is_none() {
            let sidecar_key = gemini_canvas::image_stream_generate_template_object_key(
                &runtime.runtime_state_object_key,
            );
            let remote_sidecar = object_storage.read_json(&sidecar_key).await.ok();
            let local_sidecar = read_gemini_canvas_runtime_mirror_json(&sidecar_key);
            let sidecar = if remote_sidecar
                .as_ref()
                .map(|value| {
                    gemini_canvas_sidecar_has_any_key(
                        value,
                        &["imageStreamGenerateTemplate", "mediaStreamGenerateTemplate"],
                    )
                })
                .unwrap_or(false)
            {
                remote_sidecar
            } else if local_sidecar
                .as_ref()
                .map(|value| {
                    gemini_canvas_sidecar_has_any_key(
                        value,
                        &["imageStreamGenerateTemplate", "mediaStreamGenerateTemplate"],
                    )
                })
                .unwrap_or(false)
            {
                local_sidecar
            } else {
                remote_sidecar.or(local_sidecar)
            };
            if let Some(sidecar) = sidecar {
                let maybe_template =
                    sidecar
                        .get("imageStreamGenerateTemplate")
                        .cloned()
                        .or_else(|| {
                            sidecar
                                .get("url")
                                .and_then(Value::as_str)
                                .map(|_| sidecar.clone())
                        });
                if let Some(template) = maybe_template {
                    if let Some(root) = storage_state.as_object_mut() {
                        root.insert("imageStreamGenerateTemplate".to_string(), template);
                    }
                }
            }
        }
        let batchexecute_header_id = if is_image_edit_request {
            Some(gemini_canvas::new_batchexecute_header_id())
        } else {
            gemini_canvas::harvest_text_batchexecute_header_id(&storage_state)
        };
        let replay_template = if allow_replay_template {
            if is_text_mode {
                match gemini_canvas::build_text_stream_generate_request_from_template(
                    &storage_state,
                    prompt,
                ) {
                    Ok(template) => template,
                    Err(error) => {
                        debug!(
                            provider,
                            error = %summarize_gateway_error(&error),
                            "gemini canvas text replay template was invalid; falling back to bootstrap + legacy request builder"
                        );
                        None
                    }
                }
            } else if is_image_mode && !is_image_edit_request {
                match gemini_canvas::build_image_stream_generate_request_from_template(
                    &storage_state,
                    prompt,
                    "",
                ) {
                    Ok(template) => template,
                    Err(error) => {
                        debug!(
                            provider,
                            error = %summarize_gateway_error(&error),
                            "gemini canvas image replay template was invalid; falling back to bootstrap + legacy request builder"
                        );
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };
        let image_edit_locale_hint = if is_image_edit_request {
            gemini_canvas::harvest_image_edit_template_locale(&storage_state)
        } else {
            None
        };
        if let Some(context) = image_edit_followup_context.as_deref_mut() {
            if context.locale_hint.is_none() {
                context.locale_hint = image_edit_locale_hint.clone();
            }
        }
        let auth_user = gemini_canvas::direct_http_auth_user(payload);
        let explicit_cookie_header = payload
            .extra_body
            .as_ref()
            .and_then(|extra| {
                extra
                    .get("canvasProgramInvokeContract")
                    .and_then(Value::as_object)
                    .and_then(|contract| contract.get("cookieHeader"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .or_else(|| {
                payload
                    .extra_body
                    .as_ref()
                    .and_then(|extra| extra.get("cookieHeader"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .or_else(|| {
                payload
                    .extra_body
                    .as_ref()
                    .and_then(|extra| {
                        extra
                            .get("canvasProgramInvokeContract")
                            .and_then(Value::as_object)
                            .and_then(|contract| contract.get("cookie_header"))
                            .and_then(Value::as_str)
                    })
                    .map(str::to_string)
            })
            .or_else(|| {
                payload
                    .extra_body
                    .as_ref()
                    .and_then(|extra| extra.get("cookie_header"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .or_else(|| read_json_string(&storage_state, "cookieHeader"));
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
        let mut session = if let Some(cookie_header) = explicit_cookie_header
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            gemini_canvas::pure_http_session_from_cookie_header(cookie_header, &auth_user)?
        } else {
            gemini_canvas::storage_state_to_pure_http_session(
                &storage_state,
                session_target_url,
                &effective_base_url,
                &auth_user,
            )?
        };
        if is_image_edit_request {
            append_gemini_canvas_image_edit_trace("prewarm.start", || session_target_url);
            match self
                .prewarm_gemini_canvas_image_edit_signaler(
                    payload,
                    runtime,
                    &mut session,
                    image_edit_locale_hint.as_deref(),
                    timeout,
                )
                .await
            {
                Ok(channel) => {
                    append_gemini_canvas_image_edit_trace("prewarm.ok", || {
                        format!("next_aid={}", channel.next_aid)
                    });
                    if let Some(context) = image_edit_followup_context.as_deref_mut() {
                        context.signaler_session = Some(session.clone());
                        context.signaler_channel = Some(channel);
                    }
                }
                Err(error) => {
                    append_gemini_canvas_image_edit_trace("prewarm.err", || {
                        summarize_gateway_error(&error)
                    });
                    debug!(
                        provider,
                        error = %summarize_gateway_error(&error),
                        "gemini canvas image-edit signaler prewarm failed; continuing with direct HTTP StreamGenerate"
                    );
                }
            }
        }
        Ok(StreamGeneratePreparation {
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
        })
    }
}
