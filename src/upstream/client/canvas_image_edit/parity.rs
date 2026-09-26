use super::*;

impl UpstreamClient {
    pub(super) async fn run_image_edit_media_capture_parity_preflight(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        conversation_list_source_path: &str,
        session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
        mode_index: i64,
        batchexecute_header_id: Option<&str>,
        remaining: Duration,
        attempt: usize,
        edit_context: &GeminiCanvasImageEditFollowupContext,
        completion_model_header: &str,
        failures: &mut Vec<String>,
        last_entries_preview: &mut String,
        last_completion_preview: &mut String,
        last_probe_preview: &mut String,
        last_full_preview: &mut String,
        parity_probe_preview: &mut String,
        parity_full_preview: &mut String,
        parity_o30_preview: &mut String,
        parity_k4_preview: &mut String,
    ) -> Result<Option<(Vec<gemini_canvas::GeminiCanvasMediaAsset>, String)>, GatewayError> {
        match self
            .execute_gemini_canvas_media_capture_parity_preflight_sequence(
                payload,
                model,
                bootstrap,
                conversation_list_source_path,
                session,
                mode_index,
                batchexecute_header_id,
                true,
                remaining.min(Duration::from_secs(30)),
            )
            .await
        {
            Ok(parity) => {
                if let Some(body) = parity.maziqc_probe_body.as_deref() {
                    *parity_probe_preview = compact_response_preview(body, 240);
                    *last_probe_preview = parity_probe_preview.clone();
                }
                if let Some(body) = parity.maziqc_full_body.as_deref() {
                    *parity_full_preview = compact_response_preview(body, 240);
                    *last_full_preview = parity_full_preview.clone();
                }
                if let Some(body) = parity.o30o0e_body.as_deref() {
                    *parity_o30_preview = compact_response_preview(body, 240);
                }
                if let Some(body) = parity.k4wwud_body.as_deref() {
                    *parity_k4_preview = compact_response_preview(body, 240);
                }

                let parity_entries = parity
                    .maziqc_full_body
                    .as_deref()
                    .map(gemini_canvas::extract_conversation_list_entries)
                    .filter(|entries| !entries.is_empty())
                    .or_else(|| {
                        parity
                            .maziqc_probe_body
                            .as_deref()
                            .map(gemini_canvas::extract_conversation_list_entries)
                            .filter(|entries| !entries.is_empty())
                    })
                    .unwrap_or_default();
                if !parity_entries.is_empty() {
                    *last_entries_preview =
                        preview_gemini_canvas_conversation_entries(&parity_entries, 5);
                    if let Some(selected_entry) = select_gemini_canvas_recent_conversation_entry(
                        &parity_entries,
                        &edit_context.prompt,
                        edit_context.request_started_at,
                    ) {
                        let app_path = selected_entry.app_path().unwrap_or_else(|| {
                            format!(
                                "/app/{}",
                                selected_entry.conversation_id.trim_start_matches("c_")
                            )
                        });
                        let completion_request =
                            gemini_canvas::build_video_completion_followup_request(
                                bootstrap,
                                &app_path,
                                &selected_entry.conversation_id,
                            )?;
                        match self
                            .send_gemini_canvas_text_batchexecute_request_capture_aligned(
                                payload,
                                model,
                                &completion_request,
                                session,
                                completion_model_header,
                                remaining.min(Duration::from_secs(30)),
                            )
                            .await
                        {
                            Ok(body) => {
                                *last_completion_preview = compact_response_preview(&body, 320);
                                if let Ok(assets) =
                                    gemini_canvas::extract_stream_generate_media_assets(
                                        &body,
                                        gemini_canvas::GeminiCanvasMediaOperation::Image,
                                    )
                                {
                                    return Ok(Some((assets, body)));
                                }
                                if let Ok(assets) = gemini_canvas::extract_page_blob_media_assets(
                                    &body,
                                    gemini_canvas::GeminiCanvasMediaOperation::Image,
                                ) {
                                    return Ok(Some((assets, body)));
                                }
                                failures.push(format!(
                                    "attempt={} parity_conversation={} title={} hNvQHb_missing_asset",
                                    attempt,
                                    selected_entry.conversation_id,
                                    truncate_response_preview(selected_entry.title.as_str(), 80)
                                ));
                            }
                            Err(error) => {
                                failures.push(format!(
                                    "attempt={} parity_conversation={} hNvQHb={}",
                                    attempt,
                                    selected_entry.conversation_id,
                                    summarize_gateway_error(&error)
                                ));
                            }
                        }
                    }
                }
            }
            Err(error) => {
                failures.push(format!(
                    "attempt={} parity_preflight={}",
                    attempt,
                    summarize_gateway_error(&error)
                ));
            }
        }
        Ok(None)
    }
}
