use super::*;

#[path = "canvas_completion_part_1/image_json.rs"]
mod image_json;

impl UpstreamClient {
    pub(super) async fn poll_gemini_canvas_direct_http_video_completion_body(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        source_path: &str,
        conversation_id: &str,
        response_id: &str,
        model_header: &str,
        primary_body: &str,
        timeout: Duration,
    ) -> Result<String, GatewayError> {
        let requests = build_gemini_canvas_video_completion_requests(
            bootstrap,
            source_path,
            conversation_id,
            response_id,
            primary_body,
        )?;
        let started_at = Instant::now();
        let total_budget = timeout
            .max(Duration::from_secs(45))
            .min(Duration::from_secs(120));
        let mut poll_state = GeminiCanvasVideoCompletionPollState::default();

        loop {
            let Some(remaining) = total_budget.checked_sub(started_at.elapsed()) else {
                break;
            };
            if remaining <= Duration::from_secs(5) {
                break;
            }
            let attempt = poll_state.next_attempt();

            if let Some(job_body) = self
                .try_send_gemini_canvas_video_job_poll(
                    payload,
                    model,
                    session,
                    attempt,
                    source_path,
                    conversation_id,
                    model_header,
                    remaining,
                    requests.job_poll_request.as_ref(),
                )
                .await
            {
                if let Ok(completed_body) = classify_gemini_canvas_video_stage_body(job_body) {
                    return Ok(completed_body);
                }
            }

            let attempt_state = match self
                .execute_gemini_canvas_video_completion_attempt(
                    payload,
                    model,
                    session,
                    attempt,
                    source_path,
                    conversation_id,
                    response_id,
                    &requests,
                    model_header,
                    remaining,
                )
                .await?
            {
                GeminiCanvasVideoCompletionAttemptOutcome::Completed(body) => return Ok(body),
                GeminiCanvasVideoCompletionAttemptOutcome::Continue(state) => state,
            };

            let next_sleep_for = poll_state.record_attempt_progress(&attempt_state);

            let Some(sleep_for) = next_sleep_for else {
                break;
            };
            if let Some(remaining_after_sleep) = total_budget.checked_sub(started_at.elapsed()) {
                if remaining_after_sleep > sleep_for + Duration::from_secs(2) {
                    sleep(sleep_for).await;
                    continue;
                }
            }
            break;
        }

        Err(poll_state.build_missing_asset_error(
            source_path,
            conversation_id,
            response_id,
            requests.job_id.as_deref(),
        ))
    }
}
