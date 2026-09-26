use super::*;

struct StreamGeneratePreparation {
    effective_base_url: String,
    app_bootstrap_url: String,
    share_bootstrap_url: String,
    is_text_mode: bool,
    is_image_mode: bool,
    is_image_edit_request: bool,
    image_stream_timeout: Duration,
    bootstrap_url: String,
    stream_url: String,
    storage_state: Value,
    batchexecute_header_id: Option<String>,
    replay_template: Option<gemini_canvas::GeminiCanvasTextStreamGenerateTemplate>,
    session: gemini_canvas::GeminiCanvasPureHttpSession,
}

struct StreamGenerateBootstrap {
    preparation: StreamGeneratePreparation,
    bootstrap: gemini_web::GeminiWebBootstrap,
    origin: String,
    authorization: String,
    page_path: String,
    text_preflight_source_path: String,
    referer: String,
    model_header: String,
}

enum StreamGenerateBootstrapOutcome {
    Completed(String),
    Continue(StreamGenerateBootstrap),
}

enum StreamGenerateReplayOutcome {
    Completed(String),
    ContinueBootstrap,
}

struct StreamGenerateRequestContext {
    preparation: StreamGeneratePreparation,
    bootstrap: gemini_web::GeminiWebBootstrap,
    origin: String,
    authorization: String,
    page_path: String,
    text_preflight_source_path: String,
    referer: String,
    model_header: String,
    request_uuid: String,
    request: gemini_web::GeminiWebRequest,
}

#[path = "canvas_stream_generate/bootstrap.rs"]
mod bootstrap;
#[path = "canvas_stream_generate/prepare.rs"]
mod prepare;
#[path = "canvas_stream_generate/replay.rs"]
mod replay;
#[path = "canvas_stream_generate/request.rs"]
mod request;
#[path = "canvas_stream_generate/send.rs"]
mod send;

impl UpstreamClient {
    pub(super) async fn execute_gemini_canvas_direct_http_stream_generate_body(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        mode_index: i64,
        prompt: &str,
        timeout: Duration,
        allow_replay_template: bool,
        image_edit_uploads: Option<&[gemini_canvas::GeminiCanvasImageEditUpload]>,
        mut image_edit_followup_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
    ) -> Result<String, GatewayError> {
        let prepared = self
            .prepare_gemini_canvas_stream_generate(
                payload,
                model,
                runtime,
                mode_index,
                prompt,
                timeout,
                allow_replay_template,
                image_edit_uploads,
                &mut image_edit_followup_context,
            )
            .await?;
        let bootstrapped = match self
            .bootstrap_gemini_canvas_stream_generate(
                prepared,
                payload,
                model,
                runtime,
                mode_index,
                timeout,
                allow_replay_template,
                &mut image_edit_followup_context,
            )
            .await?
        {
            StreamGenerateBootstrapOutcome::Completed(body) => return Ok(body),
            StreamGenerateBootstrapOutcome::Continue(context) => context,
        };
        let request_context = self
            .build_gemini_canvas_stream_generate_request(
                bootstrapped,
                payload,
                model,
                runtime,
                mode_index,
                prompt,
                timeout,
                allow_replay_template,
                image_edit_uploads,
                &mut image_edit_followup_context,
            )
            .await?;
        self.send_gemini_canvas_stream_generate_request(
            request_context,
            payload,
            model,
            runtime,
            mode_index,
            timeout,
            allow_replay_template,
            &mut image_edit_followup_context,
        )
        .await
    }
}
