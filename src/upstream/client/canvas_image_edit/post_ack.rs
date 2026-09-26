use super::*;

impl UpstreamClient {
    pub(in crate::upstream::client) async fn execute_gemini_canvas_image_edit_post_ack_followups(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        stream_body: &str,
        timeout: Duration,
        edit_context: &mut GeminiCanvasImageEditFollowupContext,
    ) -> Result<GeminiCanvasImageEditPostAckFollowupResult, GatewayError> {
        let app_url = edit_context
            .signaler_app_url
            .clone()
            .ok_or_else(gemini_canvas_image_edit_post_ack_missing_app_url_error)?;
        let response_id = edit_context
            .signaler_response_id
            .clone()
            .or_else(|| gemini_canvas::extract_stream_generate_response_id(stream_body).ok());
        let base_url = payload.base_url.trim_end_matches('/');
        let bootstrap_page_url = format!("{base_url}{}", gemini_web::GEMINI_WEB_DEFAULT_APP_PATH);
        let source_path = gemini_web::GEMINI_WEB_DEFAULT_APP_PATH.to_string();
        let storage_state = gateway_object_storage()?
            .read_json(&runtime.runtime_state_object_key)
            .await?;
        let auth_user = gemini_canvas::direct_http_auth_user(payload);
        let mut session = if let Some(existing_session) = edit_context.signaler_session.clone() {
            existing_session
        } else {
            gemini_canvas::storage_state_to_pure_http_session(
                &storage_state,
                &app_url,
                base_url,
                &auth_user,
            )?
        };
        let fallback_bootstrap =
            gemini_web::bootstrap_from_payload_cache(payload.extra_body.as_ref());
        let bootstrap_html = self
            .fetch_gemini_canvas_direct_http_page_html_refreshing_session_with_locale(
                payload,
                &mut session,
                &bootstrap_page_url,
                timeout
                    .min(Duration::from_secs(20))
                    .max(Duration::from_secs(8)),
                edit_context.locale_hint.as_deref(),
            )
            .await
            .ok();
        let mut bootstrap = if let Some(html) = bootstrap_html.as_deref() {
            gemini_web::parse_bootstrap_from_app_html(html, Some("en"))
                .or_else(|primary_error| fallback_bootstrap.clone().ok_or(primary_error))?
        } else {
            fallback_bootstrap
                .clone()
                .ok_or_else(gemini_canvas_image_edit_post_ack_bootstrap_missing_error)?
        };
        bootstrap =
            gemini_web::merge_bootstrap_from_fallback(bootstrap, fallback_bootstrap.as_ref());
        if let Some(locale) = gemini_canvas::harvest_image_edit_template_locale(&storage_state) {
            bootstrap.language = locale;
        }

        let batchexecute_header_id = Some(gemini_canvas::new_batchexecute_header_id());
        let trigger_model_header = gemini_canvas::build_text_batchexecute_model_header(
            None,
            batchexecute_header_id.as_deref(),
        );
        let followup_model_header = gemini_canvas::build_text_batchexecute_model_header(
            Some(gemini_canvas::GEMINI_CANVAS_TEXT_BOOTSTRAP_MODEL_ID),
            batchexecute_header_id.as_deref(),
        );

        let trigger_body = if let Some(response_id) = response_id.as_deref() {
            let trigger_request =
                gemini_canvas::build_tts_trigger_request(response_id, &bootstrap, &source_path)?;
            self.send_gemini_canvas_text_batchexecute_request_capture_aligned_refreshing_session(
                payload,
                model,
                &trigger_request,
                &mut session,
                &trigger_model_header,
                timeout
                    .min(Duration::from_secs(15))
                    .max(Duration::from_secs(6)),
            )
            .await
            .map_err(|error| {
                append_gateway_error_summary(error, "image_edit_post_ack_stage", Some("PCck7e"))
            })?
        } else {
            "<skipped: missing response id>".to_string()
        };

        let followup_request =
            gemini_canvas::build_text_bootstrap_preflight_request(&bootstrap, &source_path)?;
        let first_followup_body = self
            .send_gemini_canvas_text_batchexecute_request_capture_aligned_refreshing_session(
                payload,
                model,
                &followup_request,
                &mut session,
                &followup_model_header,
                timeout
                    .min(Duration::from_secs(15))
                    .max(Duration::from_secs(6)),
            )
            .await
            .map_err(|error| {
                append_gateway_error_summary(
                    error,
                    "image_edit_post_ack_stage",
                    Some("aPya6c:first"),
                )
            })?;

        sleep(Duration::from_secs(16)).await;

        let activity_request = gemini_canvas::build_text_batchexecute_request(
            "ESY5D",
            json!([[["bard_activity_enabled"]]]),
            &bootstrap,
            &source_path,
        )?;
        let activity_body = self
            .send_gemini_canvas_text_batchexecute_request_capture_aligned_refreshing_session(
                payload,
                model,
                &activity_request,
                &mut session,
                &trigger_model_header,
                timeout
                    .min(Duration::from_secs(20))
                    .max(Duration::from_secs(8)),
            )
            .await
            .map_err(|error| {
                append_gateway_error_summary(error, "image_edit_post_ack_stage", Some("ESY5D"))
            })?;

        let second_followup_body = self
            .send_gemini_canvas_text_batchexecute_request_capture_aligned_refreshing_session(
                payload,
                model,
                &followup_request,
                &mut session,
                &followup_model_header,
                timeout
                    .min(Duration::from_secs(20))
                    .max(Duration::from_secs(8)),
            )
            .await
            .map_err(|error| {
                append_gateway_error_summary(
                    error,
                    "image_edit_post_ack_stage",
                    Some("aPya6c:second"),
                )
            })?;

        let mut candidate_page_urls = vec![app_url.clone()];
        for candidate in &edit_context.signaler_app_urls {
            if !candidate_page_urls.iter().any(|value| value == candidate) {
                candidate_page_urls.push(candidate.clone());
            }
        }
        let mut page_body = None;
        let mut page_previews = Vec::new();
        for candidate_page_url in &candidate_page_urls {
            match self
                .fetch_gemini_canvas_direct_http_page_html_refreshing_session_with_locale(
                    payload,
                    &mut session,
                    candidate_page_url,
                    timeout
                        .min(Duration::from_secs(20))
                        .max(Duration::from_secs(8)),
                    edit_context.locale_hint.as_deref(),
                )
                .await
            {
                Ok(body) => {
                    let preview = compact_sanitized_response_preview(&body, 120);
                    page_previews.push(format!("{candidate_page_url}={preview}"));
                    if gemini_canvas::extract_page_blob_media_assets(
                        &body,
                        gemini_canvas::GeminiCanvasMediaOperation::Image,
                    )
                    .is_ok()
                    {
                        page_body = Some(body);
                        break;
                    }
                }
                Err(error) => {
                    page_previews.push(format!(
                        "{candidate_page_url}=<fetch failed: {}>",
                        summarize_gateway_error(&error)
                    ));
                }
            }
        }
        let page_preview = if page_previews.is_empty() {
            "<no page fetch>".to_string()
        } else {
            page_previews.join(" || ")
        };

        edit_context.signaler_session = Some(session);

        Ok(GeminiCanvasImageEditPostAckFollowupResult {
            preview: format!(
                "PCck7e={} | aPya6c.first={} | ESY5D={} | aPya6c.second={} | page={}",
                compact_sanitized_response_preview(&trigger_body, 160),
                compact_sanitized_response_preview(&first_followup_body, 160),
                compact_sanitized_response_preview(&activity_body, 160),
                compact_sanitized_response_preview(&second_followup_body, 160),
                page_preview,
            ),
            page_body,
        })
    }
}
