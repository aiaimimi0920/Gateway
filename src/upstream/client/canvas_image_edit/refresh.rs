use super::*;

impl UpstreamClient {
    pub(in crate::upstream::client) async fn trigger_gemini_canvas_image_page_refresh(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        storage_state: &Value,
        session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
        app_url: &str,
        timeout: Duration,
    ) -> Result<String, GatewayError> {
        let fallback_bootstrap =
            gemini_web::bootstrap_from_payload_cache(payload.extra_body.as_ref());
        let locale_override = gemini_canvas::harvest_image_edit_template_locale(storage_state);
        let bootstrap_html = self
            .fetch_gemini_canvas_direct_http_page_html_refreshing_session_with_locale(
                payload,
                session,
                app_url,
                timeout,
                locale_override.as_deref(),
            )
            .await
            .ok();
        let mut bootstrap = if let Some(html) = bootstrap_html.as_deref() {
            gemini_web::parse_bootstrap_from_app_html(html, Some("en"))
                .or_else(|primary_error| fallback_bootstrap.clone().ok_or(primary_error))?
        } else {
            fallback_bootstrap
                .clone()
                .ok_or_else(gemini_canvas_image_page_refresh_bootstrap_missing_error)?
        };
        bootstrap =
            gemini_web::merge_bootstrap_from_fallback(bootstrap, fallback_bootstrap.as_ref());
        if let Some(locale) = locale_override {
            bootstrap.language = locale;
        }
        let source_path = app_url
            .strip_prefix(payload.base_url.trim_end_matches('/'))
            .filter(|candidate| !candidate.trim().is_empty())
            .unwrap_or(gemini_web::GEMINI_WEB_DEFAULT_APP_PATH)
            .to_string();
        let mode_index = gemini_canvas::stream_generate_mode_index(
            gemini_canvas::GeminiCanvasMediaOperation::Image,
        );
        let batchexecute_header_id = Some(gemini_canvas::new_batchexecute_header_id());
        if let Err(error) = self
            .execute_gemini_canvas_page_init_preflight_sequence(
                payload,
                model,
                &bootstrap,
                &source_path,
                session,
                timeout.min(Duration::from_secs(20)),
                true,
            )
            .await
        {
            debug!(
                provider = "gemini_canvas_compatible",
                error = %summarize_gateway_error(&error),
                source_path = %source_path,
                "gemini canvas image page refresh page-init preflight failed; continuing with parity refresh"
            );
        }
        let parity = self
            .execute_gemini_canvas_media_capture_parity_preflight_sequence(
                payload,
                model,
                &bootstrap,
                &source_path,
                session,
                mode_index,
                batchexecute_header_id.as_deref(),
                true,
                timeout.min(Duration::from_secs(30)),
            )
            .await?;
        let mut stage_previews = vec![format!(
            "aPya6c={}",
            compact_response_preview(&parity.selected_bootstrap_body, 180)
        )];
        if let Some(body) = parity.o30o0e_body.as_deref() {
            stage_previews.push(format!("o30O0e={}", compact_response_preview(body, 180)));
        }
        if let Some(body) = parity.k4wwud_body.as_deref() {
            stage_previews.push(format!("K4WWud={}", compact_response_preview(body, 180)));
        }
        if let Some(body) = parity.maziqc_probe_body.as_deref() {
            stage_previews.push(format!(
                "MaZiqc.probe={}",
                compact_response_preview(body, 180)
            ));
        }
        if let Some(body) = parity.maziqc_full_body.as_deref() {
            stage_previews.push(format!(
                "MaZiqc.full={}",
                compact_response_preview(body, 180)
            ));
        }
        Ok(stage_previews.join(" | "))
    }
}
