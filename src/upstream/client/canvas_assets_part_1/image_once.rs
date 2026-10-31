//! Web image generation/editing has no idempotency key; never regenerate to recover bytes.
use super::*;

impl UpstreamClient {
    pub(in crate::upstream::client) async fn execute_gemini_web_image_http_once(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        timeout: Duration,
        context: &mut GeminiCanvasDirectHttpImageContext,
    ) -> Result<Value, GatewayError> {
        let mut session = None;
        let body = self
            .execute_gemini_canvas_stream_body_with_session(
                payload,
                model,
                runtime,
                context.mode_index,
                prompt,
                timeout,
                context.initial_stream_allows_replay_template,
                context.image_edit_uploads.as_deref(),
                context.image_edit_followup_context.as_mut(),
                Some(&mut session),
            )
            .await?;
        // Missing/provisional media is not proof that generation was rejected.
        // Do not enter legacy JSON, template resubmission or prompt-based follow-ups.
        let session = session.ok_or_else(|| {
            gemini_canvas::image_recovery::image_read_error("gemini_web_image_read_missing_session")
        })?;
        let assets = self
            .resolve_gemini_web_image_assets(
                payload,
                &session.session,
                session.cookies,
                &body,
                timeout,
            )
            .await?;
        let selected = match gemini_canvas_web_reverse_modular::plan_direct_http_image_response(
            req,
            prompt,
            &assets,
            "gemini_web_reverse_modular_compatible",
        )? {
            GeminiCanvasImageResponsePlan::FinalResponse(body) => return Ok(body),
            GeminiCanvasImageResponsePlan::Materialize(assets) => assets,
        };
        self.build_gemini_canvas_materialized_image_response(
            payload,
            req,
            runtime,
            prompt,
            timeout,
            &selected,
            "gemini_web_reverse_modular_compatible",
        )
        .await
    }
}
