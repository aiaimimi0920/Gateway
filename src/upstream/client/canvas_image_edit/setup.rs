use super::*;

pub(super) fn build_image_edit_conversation_poll_failure(
    edit_context: &GeminiCanvasImageEditFollowupContext,
    bootstrap_page: &str,
    attempt: usize,
    last_entries_preview: &str,
    last_probe_preview: &str,
    last_full_preview: &str,
    last_completion_preview: &str,
    parity_probe_preview: &str,
    parity_full_preview: &str,
    parity_o30_preview: &str,
    parity_k4_preview: &str,
    failures: &[String],
) -> GatewayError {
    let prompt_preview = truncate_response_preview(edit_context.prompt.as_str(), 120);
    let failure_summary = failures.join(" | ");
    gemini_canvas_image_edit_conversation_followup_failed_error(
        bootstrap_page,
        attempt,
        &prompt_preview,
        last_entries_preview,
        last_probe_preview,
        last_full_preview,
        last_completion_preview,
        parity_probe_preview,
        parity_full_preview,
        parity_o30_preview,
        parity_k4_preview,
        &failure_summary,
    )
}

pub(super) fn image_edit_conversation_poll_remaining(
    total_budget: Duration,
    started_at: Instant,
) -> Option<Duration> {
    total_budget
        .checked_sub(started_at.elapsed())
        .filter(|remaining| *remaining > Duration::from_secs(1))
}

pub(super) async fn wait_for_image_edit_conversation_poll_retry(
    total_budget: Duration,
    started_at: Instant,
) {
    if total_budget
        .checked_sub(started_at.elapsed())
        .is_some_and(|time_left| time_left > Duration::from_secs(3))
    {
        sleep(Duration::from_secs(2)).await;
    }
}

impl UpstreamClient {
    pub(super) async fn prepare_image_edit_conversation_poll(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        timeout: Duration,
        edit_context: &mut GeminiCanvasImageEditFollowupContext,
    ) -> Result<ConversationPollState, GatewayError> {
        let base_url = payload.base_url.trim_end_matches('/');
        let app_url = format!("{base_url}{}", gemini_web::GEMINI_WEB_DEFAULT_APP_PATH);
        let bootstrap_page_url = app_url.clone();
        let has_concrete_signaler_page = edit_context.signaler_app_url.is_some();
        append_gemini_canvas_image_edit_trace("conversation.bootstrap", || {
            bootstrap_page_url.as_str()
        });
        let storage_state = gateway_object_storage()?
            .read_json(&runtime.runtime_state_object_key)
            .await?;
        let auth_user = gemini_canvas::direct_http_auth_user(payload);
        // Image-edit completion appears to depend on the same page-owned
        // lifecycle that the signaler lane has already advanced. Reuse that
        // refreshed session when available instead of rebooting follow-up from
        // the original imported storage state.
        let mut session = if let Some(existing_session) = edit_context.signaler_session.clone() {
            existing_session
        } else {
            gemini_canvas::storage_state_to_pure_http_session(
                &storage_state,
                &bootstrap_page_url,
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
                timeout,
                edit_context.locale_hint.as_deref(),
            )
            .await
            .ok();
        let bootstrap_page = if bootstrap_html.is_some() {
            bootstrap_page_url.as_str()
        } else {
            "<payload-cache>"
        };
        let mut bootstrap = if let Some(html) = bootstrap_html.as_deref() {
            gemini_web::parse_bootstrap_from_app_html(html, Some("en"))
                .or_else(|primary_error| fallback_bootstrap.clone().ok_or(primary_error))?
        } else {
            fallback_bootstrap
                .clone()
                .ok_or_else(gemini_canvas_image_edit_conversation_bootstrap_missing_error)?
        };
        bootstrap =
            gemini_web::merge_bootstrap_from_fallback(bootstrap, fallback_bootstrap.as_ref());
        if let Some(locale) = gemini_canvas::harvest_image_edit_template_locale(&storage_state) {
            bootstrap.language = locale;
        }

        // Concrete signaler pages are useful for direct page fetches and
        // hNvQHb completion requests, but the conversation-list probes in the
        // successful /app capture shape are rooted at the generic app surface.
        // Reusing /app/<id> as source-path for MaZiqc/aPya6c/L5adhe has been
        // yielding only generic [7] frames with no recoverable entries.
        let conversation_list_source_path = gemini_web::GEMINI_WEB_DEFAULT_APP_PATH.to_string();
        let mode_index = gemini_canvas::stream_generate_mode_index(
            gemini_canvas::GeminiCanvasMediaOperation::Image,
        );
        // Successful image-edit follow-up captures use a fresh batchexecute header id
        // instead of reusing the stale id embedded in the imported StreamGenerate template.
        let batchexecute_header_id = Some(gemini_canvas::new_batchexecute_header_id());
        let neutral_header = gemini_canvas::build_text_batchexecute_model_header_variant(
            None,
            batchexecute_header_id.as_deref(),
            false,
            false,
        );
        let empty_model_header = gemini_canvas::build_text_batchexecute_model_header_variant(
            None,
            batchexecute_header_id.as_deref(),
            false,
            true,
        );
        let completion_model_header = gemini_canvas::build_text_batchexecute_model_header_variant(
            Some(gemini_canvas::GEMINI_CANVAS_TEXT_SELECTED_MODEL_HEADER_ID),
            batchexecute_header_id.as_deref(),
            true,
            false,
        );
        let probe_request = gemini_canvas::build_conversation_list_probe_request(
            &bootstrap,
            &conversation_list_source_path,
        )?;
        let full_request = gemini_canvas::build_conversation_list_full_request(
            &bootstrap,
            &conversation_list_source_path,
        )?;
        let bootstrap_request = gemini_canvas::build_text_bootstrap_preflight_request(
            &bootstrap,
            &conversation_list_source_path,
        )?;
        let mode_selection_request = gemini_canvas::build_text_mode_selection_preflight_request(
            &bootstrap,
            &conversation_list_source_path,
        )?;

        let started_at = Instant::now();
        let total_budget = if has_concrete_signaler_page {
            // Successful browser captures settle image-edit assets in the primary
            // StreamGenerate response within seconds, not minutes. Once we have
            // already fallen back to generic conversation probing, keeping this
            // lane alive for the old ~90s budget only pushes caller-visible HTTP
            // requests into the outer transport timeout without surfacing any
            // better signal than repeated generic [7] frames.
            timeout
                .min(Duration::from_secs(60))
                .max(Duration::from_secs(30))
        } else {
            timeout
                .min(Duration::from_secs(30))
                .max(Duration::from_secs(12))
        };
        let mut attempt = 0usize;
        let mut failures = Vec::new();
        let mut last_entries_preview = "<none>".to_string();
        let mut last_completion_preview = "<none>".to_string();
        let mut last_probe_preview = "<none>".to_string();
        let mut last_full_preview = "<none>".to_string();
        let mut parity_probe_preview = "<none>".to_string();
        let mut parity_full_preview = "<none>".to_string();
        let mut parity_o30_preview = "<none>".to_string();
        let mut parity_k4_preview = "<none>".to_string();
        Ok(ConversationPollState {
            has_concrete_signaler_page,
            session,
            bootstrap,
            bootstrap_page: bootstrap_page.to_string(),
            conversation_list_source_path,
            mode_index,
            batchexecute_header_id,
            neutral_header,
            empty_model_header,
            completion_model_header,
            probe_request,
            full_request,
            bootstrap_request,
            mode_selection_request,
            started_at,
            total_budget,
            attempt,
            failures,
            last_entries_preview,
            last_completion_preview,
            last_probe_preview,
            last_full_preview,
            parity_probe_preview,
            parity_full_preview,
            parity_o30_preview,
            parity_k4_preview,
        })
    }
}
