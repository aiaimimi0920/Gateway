use super::*;

impl UpstreamClient {
    pub(super) async fn build_gemini_canvas_stream_generate_request(
        &self,
        bootstrapped: StreamGenerateBootstrap,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        mode_index: i64,
        prompt: &str,
        timeout: Duration,
        allow_replay_template: bool,
        image_edit_uploads: Option<&[gemini_canvas::GeminiCanvasImageEditUpload]>,
        image_edit_followup_context: &mut Option<&mut GeminiCanvasImageEditFollowupContext>,
    ) -> Result<StreamGenerateRequestContext, GatewayError> {
        let StreamGenerateBootstrap {
            preparation:
                StreamGeneratePreparation {
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
                    mut session,
                },
            bootstrap,
            origin,
            authorization,
            page_path,
            text_preflight_source_path,
            referer,
            model_header,
        } = bootstrapped;
        let provider = "gemini_canvas_compatible";
        let object_storage = gateway_object_storage()?;
        if is_text_mode {
            self.execute_gemini_canvas_page_init_preflight_sequence(
                payload,
                model,
                &bootstrap,
                &text_preflight_source_path,
                &session,
                timeout,
                false,
            )
            .await?;
            self.execute_gemini_canvas_text_mode_selection_preflight(
                payload,
                model,
                &bootstrap,
                &text_preflight_source_path,
                &session,
                &origin,
                &authorization,
                timeout,
            )
            .await?;
            self.execute_gemini_canvas_text_bootstrap_preflight(
                payload,
                model,
                &bootstrap,
                &text_preflight_source_path,
                &session,
                &origin,
                &authorization,
                timeout,
            )
            .await?;
            self.execute_gemini_canvas_text_state_preflight(
                payload,
                model,
                &bootstrap,
                &text_preflight_source_path,
                &session,
                &origin,
                &authorization,
                timeout,
            )
            .await?;
            for (state_len, tail_index, tail_value, marker) in
                gemini_canvas_direct_http_text_state_variant_preflight_specs()
            {
                if let Err(error) = self
                    .execute_gemini_canvas_text_state_variant_preflight(
                        payload,
                        model,
                        &bootstrap,
                        &text_preflight_source_path,
                        &session,
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
                        "gemini canvas text optional page-state update failed; continuing with StreamGenerate"
                    );
                }
            }
        } else {
            let media_preflight_source_path = text_preflight_source_path.as_str();
            if is_image_mode {
                let image_preflight_result = if is_image_edit_request {
                    self.execute_gemini_canvas_media_capture_parity_preflight_sequence(
                        payload,
                        model,
                        &bootstrap,
                        media_preflight_source_path,
                        &mut session,
                        mode_index,
                        batchexecute_header_id.as_deref(),
                        is_image_edit_request,
                        timeout,
                    )
                    .await
                    .map(|_| ())
                } else {
                    self.execute_gemini_canvas_image_capture_parity_preflight_sequence(
                        payload,
                        model,
                        &bootstrap,
                        media_preflight_source_path,
                        &session,
                        batchexecute_header_id.as_deref(),
                        timeout,
                    )
                    .await
                };
                if let Err(error) = image_preflight_result {
                    debug!(
                        provider = "gemini_canvas_compatible",
                        error = %summarize_gateway_error(&error),
                        image_edit = is_image_edit_request,
                        source_path = media_preflight_source_path,
                        "gemini canvas image parity preflight failed; falling back to legacy media preflight chain"
                    );
                    self.execute_gemini_canvas_media_legacy_preflight_sequence(
                        payload,
                        model,
                        &bootstrap,
                        media_preflight_source_path,
                        &session,
                        mode_index,
                        batchexecute_header_id.as_deref(),
                        timeout,
                    )
                    .await?;
                }
            } else {
                if let Err(error) = self
                    .execute_gemini_canvas_media_capture_parity_preflight_sequence(
                        payload,
                        model,
                        &bootstrap,
                        media_preflight_source_path,
                        &mut session,
                        mode_index,
                        batchexecute_header_id.as_deref(),
                        false,
                        timeout,
                    )
                    .await
                {
                    debug!(
                        provider = "gemini_canvas_compatible",
                        error = %summarize_gateway_error(&error),
                        mode_index,
                        source_path = media_preflight_source_path,
                        "gemini canvas media parity preflight failed; falling back to legacy media preflight chain"
                    );
                    self.execute_gemini_canvas_media_legacy_preflight_sequence(
                        payload,
                        model,
                        &bootstrap,
                        media_preflight_source_path,
                        &session,
                        mode_index,
                        batchexecute_header_id.as_deref(),
                        timeout,
                    )
                    .await?;
                }
            }
        }

        let image_edit_sidecar_key = if is_image_edit_request {
            Some(
                gemini_canvas::image_edit_stream_generate_template_object_key(
                    &runtime.runtime_state_object_key,
                ),
            )
        } else {
            None
        };
        let image_edit_remote_sidecar = if let Some(sidecar_key) = image_edit_sidecar_key.as_deref()
        {
            object_storage.read_json(sidecar_key).await.ok()
        } else {
            None
        };
        let image_edit_local_sidecar = if let Some(sidecar_key) = image_edit_sidecar_key.as_deref()
        {
            read_gemini_canvas_runtime_mirror_json(sidecar_key)
        } else {
            None
        };
        let image_edit_template_source_origin = if is_image_edit_request
            && image_edit_local_sidecar
                .as_ref()
                .map(|value| {
                    gemini_canvas_sidecar_has_any_key(
                        value,
                        &["imageEditStreamGenerateTemplate", "imageEditStreamTemplate"],
                    )
                })
                .unwrap_or(false)
        {
            Some("local_sidecar")
        } else if is_image_edit_request
            && image_edit_remote_sidecar
                .as_ref()
                .map(|value| {
                    gemini_canvas_sidecar_has_any_key(
                        value,
                        &["imageEditStreamGenerateTemplate", "imageEditStreamTemplate"],
                    )
                })
                .unwrap_or(false)
        {
            Some("remote_sidecar")
        } else if is_image_edit_request
            && gemini_canvas_sidecar_has_any_key(
                &storage_state,
                &["imageEditStreamGenerateTemplate", "imageEditStreamTemplate"],
            )
        {
            Some("storage_state")
        } else {
            None
        };
        let image_edit_template_source = match image_edit_template_source_origin {
            Some("local_sidecar") => image_edit_local_sidecar.as_ref(),
            Some("remote_sidecar") => image_edit_remote_sidecar.as_ref(),
            Some("storage_state") => Some(&storage_state),
            _ => None,
        };

        let image_edit_uploaded_refs = if is_image_edit_request {
            Some(
                self.upload_gemini_canvas_image_edit_inputs(
                    payload,
                    &session,
                    &bootstrap,
                    image_edit_uploads.unwrap_or(&[]),
                    timeout,
                )
                .await?,
            )
        } else {
            None
        };
        let image_edit_seed = if is_image_edit_request {
            image_edit_template_source
                .and_then(gemini_canvas::harvest_image_edit_stream_generate_seed)
        } else {
            None
        };
        // Image-edit StreamGenerate requests appear to require a fresh request UUID on every send.
        // Reusing the harvested template UUID keeps the request pinned to the captured page-owned state
        // and prevents the replay lane from advancing into a new live edit settlement.
        let request_uuid = gemini_canvas::new_stream_generate_request_uuid();
        let stream_generate_header_id = if is_image_edit_request {
            Some(gemini_canvas::new_batchexecute_header_id())
        } else {
            None
        };
        let stream_generate_request_hex = if is_image_edit_request {
            Some(uuid::Uuid::new_v4().simple().to_string())
        } else {
            None
        };
        let mut replay_template = if is_image_edit_request && allow_replay_template {
            let uploaded_refs = image_edit_uploaded_refs.as_deref().unwrap_or(&[]);
            let initial_rebuild = image_edit_template_source.and_then(|value| {
                match gemini_canvas::build_image_edit_stream_generate_request_from_template(
                    value,
                    prompt,
                    &request_uuid,
                    uploaded_refs,
                ) {
                    Ok(template) => template,
                    Err(error) => {
                        debug!(
                            provider,
                            error = %summarize_gateway_error(&error),
                            template_source = image_edit_template_source_origin.unwrap_or("<none>"),
                            "gemini canvas image-edit preferred replay template source was invalid after upload; falling back to legacy heavy builder"
                        );
                        None
                    }
                }
            });
            match initial_rebuild {
                Some(template) => Some(template),
                None => {
                    append_gemini_canvas_image_edit_debug_json(
                        "gemini-canvas-image-edit-request-debug-template-miss.json",
                        || {
                            build_gemini_canvas_image_edit_template_miss_debug_snapshot(
                                &runtime.runtime_state_object_key,
                                image_edit_sidecar_key.as_deref(),
                                storage_state
                                    .get("imageEditStreamGenerateTemplate")
                                    .is_some(),
                                storage_state.get("imageEditStreamTemplate").is_some(),
                                image_edit_sidecar_key
                                    .as_ref()
                                    .and_then(|key| gemini_canvas_runtime_mirror_json_path(key))
                                    .map(|path| path.exists())
                                    .unwrap_or(false),
                                image_edit_template_source_origin,
                                image_edit_template_source_origin.is_some(),
                            )
                        },
                    );
                    None
                }
            }
        } else {
            replay_template
        };
        if is_image_edit_request && replay_template.is_none() {
            let image_edit_sidecar_has_template_key = image_edit_remote_sidecar
                .as_ref()
                .or(image_edit_local_sidecar.as_ref())
                .map(|value| {
                    value.get("imageEditStreamGenerateTemplate").is_some()
                        || value.get("imageEditStreamTemplate").is_some()
                })
                .unwrap_or(false);
            if !image_edit_sidecar_has_template_key
                && !storage_state
                    .get("imageEditStreamGenerateTemplate")
                    .is_some()
                && !storage_state.get("imageEditStreamTemplate").is_some()
            {
                append_gemini_canvas_image_edit_debug_json(
                    "gemini-canvas-image-edit-request-debug-template-miss.json",
                    || {
                        build_gemini_canvas_image_edit_template_miss_debug_snapshot(
                            &runtime.runtime_state_object_key,
                            image_edit_sidecar_key.as_deref(),
                            storage_state
                                .get("imageEditStreamGenerateTemplate")
                                .is_some(),
                            storage_state.get("imageEditStreamTemplate").is_some(),
                            image_edit_sidecar_key
                                .as_ref()
                                .and_then(|key| gemini_canvas_runtime_mirror_json_path(key))
                                .map(|path| path.exists())
                                .unwrap_or(false),
                            image_edit_template_source_origin,
                            image_edit_sidecar_has_template_key,
                        )
                    },
                );
            }
        }
        if let (true, Some(template), Some(header_id)) = (
            is_image_edit_request,
            replay_template.as_mut(),
            stream_generate_header_id.as_deref(),
        ) {
            let _ = gemini_canvas::refresh_stream_generate_template_model_header_id(
                template, header_id,
            );
        }
        if let (true, Some(template), Some(request_hex)) = (
            is_image_edit_request,
            replay_template.as_mut(),
            stream_generate_request_hex.as_deref(),
        ) {
            let _ =
                gemini_canvas::refresh_stream_generate_template_request_hex(template, request_hex);
        }
        if is_image_edit_request {
            if let Some(template) = replay_template.as_ref() {
                append_gemini_canvas_image_edit_request_debug_snapshot(
                    "gemini-canvas-image-edit-request-debug-template-pre-refresh.json",
                    "template-pre-refresh",
                    &template.url,
                    &template.query,
                    &template.form,
                    || gemini_canvas_debug_headers_snapshot_from_hash_map(&template.headers),
                    || {
                        let keys = ["imageEditStreamGenerateTemplate", "imageEditStreamTemplate"];
                        build_gemini_canvas_image_edit_template_pre_refresh_debug_extra(
                            &request_uuid,
                            &runtime.runtime_state_object_key,
                            &gemini_canvas::image_edit_stream_generate_template_object_key(
                                &runtime.runtime_state_object_key,
                            ),
                            image_edit_template_source_origin,
                            gemini_canvas_sidecar_has_any_key(&storage_state, &keys),
                            image_edit_remote_sidecar
                                .as_ref()
                                .map(|value| gemini_canvas_sidecar_has_any_key(value, &keys))
                                .unwrap_or(false),
                            image_edit_local_sidecar
                                .as_ref()
                                .map(|value| gemini_canvas_sidecar_has_any_key(value, &keys))
                                .unwrap_or(false),
                            &build_gemini_canvas_image_edit_uploaded_refs_debug_snapshot(
                                image_edit_uploaded_refs.as_deref().unwrap_or(&[]),
                            ),
                            build_gemini_canvas_image_edit_seed_debug_snapshot(
                                image_edit_seed.as_ref(),
                            ),
                        )
                    },
                );
            }
        }
        let request = if let Some(uploaded_refs) = image_edit_uploaded_refs.as_deref() {
            gemini_canvas::build_stream_generate_heavy_request_with_uploaded_files_seeded(
                prompt,
                &bootstrap,
                &request_uuid,
                mode_index,
                uploaded_refs,
                image_edit_seed.as_ref(),
            )?
        } else {
            gemini_canvas::build_stream_generate_heavy_request(
                prompt,
                &bootstrap,
                &request_uuid,
                mode_index,
            )?
        };
        Ok(StreamGenerateRequestContext {
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
            model_header,
            request_uuid,
            request,
        })
    }
}
