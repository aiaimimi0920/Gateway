use super::*;

struct ConversationPollState {
    has_concrete_signaler_page: bool,
    session: gemini_canvas::GeminiCanvasPureHttpSession,
    bootstrap: gemini_web::GeminiWebBootstrap,
    bootstrap_page: String,
    conversation_list_source_path: String,
    mode_index: i64,
    batchexecute_header_id: Option<String>,
    neutral_header: String,
    empty_model_header: String,
    completion_model_header: String,
    probe_request: gemini_web::GeminiWebRequest,
    full_request: gemini_web::GeminiWebRequest,
    bootstrap_request: gemini_web::GeminiWebRequest,
    mode_selection_request: gemini_web::GeminiWebRequest,
    started_at: Instant,
    total_budget: Duration,
    attempt: usize,
    failures: Vec<String>,
    last_entries_preview: String,
    last_completion_preview: String,
    last_probe_preview: String,
    last_full_preview: String,
    parity_probe_preview: String,
    parity_full_preview: String,
    parity_o30_preview: String,
    parity_k4_preview: String,
}

#[path = "setup.rs"]
mod setup;
use setup::{
    build_image_edit_conversation_poll_failure, image_edit_conversation_poll_remaining,
    wait_for_image_edit_conversation_poll_retry,
};
#[path = "parity.rs"]
mod parity;

impl UpstreamClient {
    pub(in crate::upstream::client) async fn poll_gemini_canvas_image_edit_conversation_assets(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        timeout: Duration,
        edit_context: &mut GeminiCanvasImageEditFollowupContext,
    ) -> Result<(Vec<gemini_canvas::GeminiCanvasMediaAsset>, String), GatewayError> {
        let state = self
            .prepare_image_edit_conversation_poll(payload, model, runtime, timeout, edit_context)
            .await?;
        let ConversationPollState {
            has_concrete_signaler_page,
            mut session,
            bootstrap,
            bootstrap_page,
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
            mut attempt,
            mut failures,
            mut last_entries_preview,
            mut last_completion_preview,
            mut last_probe_preview,
            mut last_full_preview,
            mut parity_probe_preview,
            mut parity_full_preview,
            mut parity_o30_preview,
            mut parity_k4_preview,
        } = state;
        loop {
            let Some(remaining) = image_edit_conversation_poll_remaining(total_budget, started_at)
            else {
                break;
            };
            attempt += 1;

            if attempt == 1 {
                if has_concrete_signaler_page {
                    if let Err(error) = self
                        .execute_gemini_canvas_page_init_preflight_sequence(
                            payload,
                            model,
                            &bootstrap,
                            &conversation_list_source_path,
                            &session,
                            remaining.min(Duration::from_secs(20)),
                            true,
                        )
                        .await
                    {
                        failures.push(format!(
                            "attempt={} page_init={} source_path={}",
                            attempt,
                            summarize_gateway_error(&error),
                            conversation_list_source_path
                        ));
                    }
                    let capture_aligned_steps = [
                        ("fast_full", &full_request, neutral_header.as_str()),
                        (
                            "fast_bootstrap",
                            &bootstrap_request,
                            empty_model_header.as_str(),
                        ),
                        (
                            "fast_mode_selection",
                            &mode_selection_request,
                            neutral_header.as_str(),
                        ),
                        ("fast_probe", &probe_request, neutral_header.as_str()),
                    ];
                    let mut fast_full_body: Option<String> = None;
                    let mut fast_probe_body: Option<String> = None;
                    for (label, request, header) in capture_aligned_steps {
                        match self
                            .send_gemini_canvas_text_batchexecute_request_capture_aligned_refreshing_session(
                                payload,
                                model,
                                request,
                                &mut session,
                                header,
                                remaining.min(Duration::from_secs(12)),
                            )
                            .await
                        {
                            Ok(body) => {
                                let preview = compact_response_preview(&body, 240);
                                match label {
                                    "fast_full" => {
                                        last_full_preview = preview.clone();
                                        fast_full_body = Some(body);
                                    }
                                    "fast_probe" => {
                                        last_probe_preview = preview.clone();
                                        fast_probe_body = Some(body);
                                    }
                                    _ => {
                                        failures.push(format!(
                                            "attempt={} {}={}",
                                            attempt, label, preview
                                        ));
                                    }
                                }
                            }
                            Err(error) => failures.push(format!(
                                "attempt={} {}={}",
                                attempt,
                                label,
                                summarize_gateway_error(&error)
                            )),
                        }
                    }

                    let fast_entries = fast_full_body
                        .as_deref()
                        .map(gemini_canvas::extract_conversation_list_entries)
                        .filter(|entries| !entries.is_empty())
                        .or_else(|| {
                            fast_probe_body
                                .as_deref()
                                .map(gemini_canvas::extract_conversation_list_entries)
                                .filter(|entries| !entries.is_empty())
                        })
                        .unwrap_or_default();
                    if !fast_entries.is_empty() {
                        last_entries_preview =
                            preview_gemini_canvas_conversation_entries(&fast_entries, 5);
                        if let Some(selected_entry) = select_gemini_canvas_recent_conversation_entry(
                            &fast_entries,
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
                                    &bootstrap,
                                    &app_path,
                                    &selected_entry.conversation_id,
                                )?;
                            match self
                                .send_gemini_canvas_text_batchexecute_request_capture_aligned_refreshing_session(
                                    payload,
                                    model,
                                    &completion_request,
                                    &mut session,
                                    &completion_model_header,
                                    remaining.min(Duration::from_secs(20)),
                                )
                                .await
                            {
                                Ok(body) => {
                                    last_completion_preview =
                                        compact_response_preview(&body, 320);
                                    if let Ok(assets) =
                                        gemini_canvas::extract_stream_generate_media_assets(
                                            &body,
                                            gemini_canvas::GeminiCanvasMediaOperation::Image,
                                        )
                                    {
                                        return Ok((assets, body));
                                    }
                                    if let Ok(assets) =
                                        gemini_canvas::extract_page_blob_media_assets(
                                            &body,
                                            gemini_canvas::GeminiCanvasMediaOperation::Image,
                                        )
                                    {
                                        return Ok((assets, body));
                                    }
                                }
                                Err(error) => failures.push(format!(
                                    "attempt={} fast_completion={}",
                                    attempt,
                                    summarize_gateway_error(&error)
                                )),
                            }
                        }
                    }
                }

                if let Some(result) = self
                    .run_image_edit_media_capture_parity_preflight(
                        payload,
                        model,
                        &bootstrap,
                        &conversation_list_source_path,
                        &mut session,
                        mode_index,
                        batchexecute_header_id.as_deref(),
                        remaining,
                        attempt,
                        edit_context,
                        &completion_model_header,
                        &mut failures,
                        &mut last_entries_preview,
                        &mut last_completion_preview,
                        &mut last_probe_preview,
                        &mut last_full_preview,
                        &mut parity_probe_preview,
                        &mut parity_full_preview,
                        &mut parity_o30_preview,
                        &mut parity_k4_preview,
                    )
                    .await?
                {
                    return Ok(result);
                }
            }

            let probe_body = match self
                .send_gemini_canvas_text_batchexecute_request_capture_aligned_refreshing_session(
                    payload,
                    model,
                    &probe_request,
                    &mut session,
                    &neutral_header,
                    remaining.min(Duration::from_secs(20)),
                )
                .await
            {
                Ok(body) => {
                    last_probe_preview = compact_response_preview(&body, 240);
                    Some(body)
                }
                Err(error) => {
                    failures.push(format!(
                        "attempt={} mazqic_probe={}",
                        attempt,
                        summarize_gateway_error(&error)
                    ));
                    None
                }
            };

            let full_body = match self
                .send_gemini_canvas_text_batchexecute_request_capture_aligned_refreshing_session(
                    payload,
                    model,
                    &full_request,
                    &mut session,
                    &neutral_header,
                    remaining.min(Duration::from_secs(20)),
                )
                .await
            {
                Ok(body) => {
                    last_full_preview = compact_response_preview(&body, 240);
                    Some(body)
                }
                Err(error) => {
                    failures.push(format!(
                        "attempt={} mazqic_full={}",
                        attempt,
                        summarize_gateway_error(&error)
                    ));
                    None
                }
            };

            let entries = full_body
                .as_deref()
                .map(gemini_canvas::extract_conversation_list_entries)
                .filter(|entries| !entries.is_empty())
                .or_else(|| {
                    probe_body
                        .as_deref()
                        .map(gemini_canvas::extract_conversation_list_entries)
                        .filter(|entries| !entries.is_empty())
                })
                .unwrap_or_default();

            if !entries.is_empty() {
                last_entries_preview = preview_gemini_canvas_conversation_entries(&entries, 5);
            }

            let Some(selected_entry) = select_gemini_canvas_recent_conversation_entry(
                &entries,
                &edit_context.prompt,
                edit_context.request_started_at,
            ) else {
                failures.push(format!(
                    "attempt={} no_recent_conversation probe_preview={} full_preview={} entries={}",
                    attempt, last_probe_preview, last_full_preview, last_entries_preview
                ));
                if total_budget
                    .checked_sub(started_at.elapsed())
                    .is_some_and(|time_left| time_left > Duration::from_secs(3))
                {
                    sleep(Duration::from_secs(2)).await;
                    continue;
                }
                break;
            };

            let app_path = selected_entry.app_path().unwrap_or_else(|| {
                format!(
                    "/app/{}",
                    selected_entry.conversation_id.trim_start_matches("c_")
                )
            });
            let completion_request = gemini_canvas::build_video_completion_followup_request(
                &bootstrap,
                &app_path,
                &selected_entry.conversation_id,
            )?;
            match self
                .send_gemini_canvas_text_batchexecute_request_capture_aligned_refreshing_session(
                    payload,
                    model,
                    &completion_request,
                    &mut session,
                    &completion_model_header,
                    remaining.min(Duration::from_secs(30)),
                )
                .await
            {
                Ok(body) => {
                    last_completion_preview = compact_response_preview(&body, 320);
                    if let Ok(assets) = gemini_canvas::extract_stream_generate_media_assets(
                        &body,
                        gemini_canvas::GeminiCanvasMediaOperation::Image,
                    ) {
                        return Ok((assets, body));
                    }
                    if let Ok(assets) = gemini_canvas::extract_page_blob_media_assets(
                        &body,
                        gemini_canvas::GeminiCanvasMediaOperation::Image,
                    ) {
                        return Ok((assets, body));
                    }
                    failures.push(format!(
                        "attempt={} conversation={} title={} hNvQHb_missing_asset",
                        attempt,
                        selected_entry.conversation_id,
                        truncate_response_preview(selected_entry.title.as_str(), 80)
                    ));
                }
                Err(error) => {
                    failures.push(format!(
                        "attempt={} conversation={} hNvQHb={}",
                        attempt,
                        selected_entry.conversation_id,
                        summarize_gateway_error(&error)
                    ));
                }
            }

            wait_for_image_edit_conversation_poll_retry(total_budget, started_at).await;
        }

        Err(build_image_edit_conversation_poll_failure(
            edit_context,
            &bootstrap_page,
            attempt,
            &last_entries_preview,
            &last_probe_preview,
            &last_full_preview,
            &last_completion_preview,
            &parity_probe_preview,
            &parity_full_preview,
            &parity_o30_preview,
            &parity_k4_preview,
            &failures,
        ))
    }
}
