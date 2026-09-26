use super::*;

struct MediaCaptureParityPreflightState {
    neutral_header: String,
    flagged_neutral_header: String,
    flagged_selected_header: String,
    language: String,
    selection_mode_index: i64,
    primary_mode_index: i64,
    tool_menu_soft_badge_impression_counts: Value,
    popup_zs_visits_cooldown: Value,
    xhau0b_payload: Value,
    is_image_mode: bool,
    initial_header2: &'static str,
    selected_header2: &'static str,
    maziqc_probe_body: Option<String>,
    maziqc_full_body: Option<String>,
    o30o0e_body: Option<String>,
    k4wwud_body: Option<String>,
}

enum MediaCaptureParityPreflightOutcome {
    Completed(GeminiCanvasMediaCaptureParityPreflightResult),
    Continue(MediaCaptureParityPreflightState),
}

#[path = "finish.rs"]
mod finish;
#[path = "prepare.rs"]
mod prepare;

impl UpstreamClient {
    pub(in crate::upstream::client) async fn execute_gemini_canvas_media_capture_parity_preflight_sequence(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        source_path: &str,
        session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
        mode_index: i64,
        batchexecute_header_id: Option<&str>,
        is_image_edit_request: bool,
        timeout: Duration,
    ) -> Result<GeminiCanvasMediaCaptureParityPreflightResult, GatewayError> {
        let prepared = self
            .prepare_media_capture_parity_preflight(
                payload,
                model,
                bootstrap,
                source_path,
                session,
                mode_index,
                batchexecute_header_id,
                is_image_edit_request,
                timeout,
            )
            .await?;
        let state = match prepared {
            MediaCaptureParityPreflightOutcome::Completed(result) => return Ok(result),
            MediaCaptureParityPreflightOutcome::Continue(state) => state,
        };
        self.finish_media_capture_parity_preflight(
            state,
            payload,
            model,
            bootstrap,
            source_path,
            session,
            timeout,
        )
        .await
    }
}
