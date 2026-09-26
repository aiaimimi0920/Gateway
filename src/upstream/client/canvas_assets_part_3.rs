use super::*;

impl UpstreamClient {
    pub(super) async fn prepare_gemini_canvas_direct_http_media_followup_context(
        &self,
        payload: &ProviderAccountPayload,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        operation: gemini_canvas::GeminiCanvasMediaOperation,
        stream_body: &str,
        locator_override: Option<gemini_canvas::GeminiCanvasStreamGenerateLocator>,
        timeout: Duration,
        force_root_app_followup: bool,
        locale_override: Option<&str>,
    ) -> Result<GeminiCanvasMediaFollowupContext, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let mut page_seed = prepare_gemini_canvas_page_seed(
            payload,
            runtime,
            operation,
            stream_body,
            force_root_app_followup,
            None,
            false,
            provider,
            "gemini canvas media follow-up",
        )?;
        if let Some(locator) = locator_override {
            let page_base_url = gemini_canvas_page_base_url(payload);
            page_seed.conversation_page_url = Some(format!(
                "{}{}",
                page_base_url.trim_end_matches('/'),
                locator.app_path
            ));
            page_seed.prefer_root_app_path = false;
            page_seed.locator = Some(locator);
        }
        let locator = page_seed.locator;
        let initial_page_target_mode = classify_gemini_canvas_page_target_mode(
            force_root_app_followup,
            false,
            locator.is_some(),
            false,
        );
        let force_root_app_path = page_seed.prefer_root_app_path;
        let base_url = payload.base_url.trim_end_matches('/');
        let app_bootstrap_url = page_seed.app_bootstrap_url;
        let share_bootstrap_url = page_seed.share_bootstrap_url;
        let conversation_url = page_seed.conversation_page_url;
        let storage_state = gateway_object_storage()?
            .read_json(&runtime.runtime_state_object_key)
            .await?;
        let auth_user = gemini_canvas::direct_http_auth_user(payload);
        let session = gemini_canvas_pure_http_session_from_payload_or_storage(
            payload,
            &storage_state,
            conversation_url
                .as_deref()
                .unwrap_or(app_bootstrap_url.as_str()),
            base_url,
            &auth_user,
        )?;
        let bootstrap_candidates = build_gemini_canvas_followup_bootstrap_candidates(
            force_root_app_path,
            conversation_url.as_deref(),
            &app_bootstrap_url,
            &share_bootstrap_url,
        );
        let mut bootstrap_failures = Vec::new();
        let mut selected_bootstrap: Option<(String, String)> = None;

        for candidate_url in &bootstrap_candidates {
            match self
                .fetch_gemini_canvas_direct_http_page_html_with_locale(
                    payload,
                    &session,
                    candidate_url.as_str(),
                    timeout,
                    locale_override,
                )
                .await
            {
                Ok(body) => {
                    selected_bootstrap = Some((body, candidate_url.clone()));
                    break;
                }
                Err(error) => bootstrap_failures.push(format!(
                    "{}: {}",
                    candidate_url,
                    summarize_gateway_error(&error)
                )),
            }
        }

        let (bootstrap_body, bootstrap_page_url) = selected_bootstrap.ok_or_else(|| {
            gemini_canvas_media_followup_bootstrap_failed_error(
                initial_page_target_mode,
                locator
                    .as_ref()
                    .map(|locator| locator.app_path.as_str())
                    .unwrap_or(gemini_web::GEMINI_WEB_DEFAULT_APP_PATH),
                bootstrap_candidates.join(",").as_str(),
                bootstrap_failures.join(" | ").as_str(),
            )
        })?;

        let fallback_bootstrap =
            gemini_web::bootstrap_from_payload_cache(payload.extra_body.as_ref());
        let mut bootstrap =
            gemini_web::parse_bootstrap_from_app_html(&bootstrap_body, Some(&bootstrap_page_url))
                .or_else(|primary_error| fallback_bootstrap.clone().ok_or(primary_error))?;
        bootstrap =
            gemini_web::merge_bootstrap_from_fallback(bootstrap, fallback_bootstrap.as_ref());
        if force_root_app_followup && operation == gemini_canvas::GeminiCanvasMediaOperation::Image
        {
            if let Some(locale) = gemini_canvas::harvest_image_edit_template_locale(&storage_state)
            {
                bootstrap.language = locale;
            }
        }

        let followup_target = GeminiCanvasFollowupTarget::from_bootstrap(
            force_root_app_path,
            bootstrap.app_page_path.as_deref(),
            locator,
            initial_page_target_mode,
        );

        Ok(GeminiCanvasMediaFollowupContext {
            bootstrap,
            bootstrap_page_url,
            session,
            followup_target,
        })
    }
}
