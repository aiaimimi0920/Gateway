use super::*;

impl UpstreamClient {
    pub(in crate::upstream::client) async fn build_gemini_canvas_direct_http_image_response(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        timeout: Duration,
        image_assets: &[gemini_canvas::GeminiCanvasMediaAsset],
        image_json_policy: &GeminiCanvasDirectHttpImageJsonPolicy,
    ) -> Result<Value, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let selected_image_assets =
            match gemini_canvas_web_reverse_modular::plan_direct_http_image_response(
                req,
                prompt,
                image_assets,
                provider,
            )? {
                GeminiCanvasImageResponsePlan::FinalResponse(body) => return Ok(body),
                GeminiCanvasImageResponsePlan::Materialize(assets) => assets,
            };
        let request_timeout = timeout.max(Duration::from_secs(240));
        match self
            .build_gemini_canvas_materialized_image_response(
                payload,
                req,
                runtime,
                prompt,
                request_timeout,
                &selected_image_assets,
                provider,
            )
            .await
        {
            Ok(body) => Ok(body),
            Err(download_error) => {
                self.recover_gemini_canvas_direct_http_image_materialize_failure(
                    payload,
                    req,
                    model,
                    runtime,
                    prompt,
                    timeout,
                    image_json_policy,
                    download_error,
                )
                .await
            }
        }
    }

    pub(in crate::upstream::client) async fn recover_gemini_canvas_direct_http_image_materialize_failure(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        timeout: Duration,
        image_json_policy: &GeminiCanvasDirectHttpImageJsonPolicy,
        download_error: GatewayError,
    ) -> Result<Value, GatewayError> {
        match image_json_policy.on_materialize_failure() {
            GeminiCanvasDirectHttpImageJsonAction::TryJsonWithErrorContext { context_key } => {
                match self
                    .execute_gemini_canvas_direct_http_image_json(
                        payload, req, model, runtime, prompt, timeout,
                    )
                    .await
                {
                    Ok(body) => Ok(body),
                    Err(image_json_error) => {
                        let image_json_summary = summarize_gateway_error(&image_json_error);
                        Err(append_gateway_error_summary(
                            download_error,
                            context_key,
                            Some(&image_json_summary),
                        ))
                    }
                }
            }
            GeminiCanvasDirectHttpImageJsonAction::ReturnOriginal
            | GeminiCanvasDirectHttpImageJsonAction::ReturnOriginalWithSummary { .. }
            | GeminiCanvasDirectHttpImageJsonAction::Skip
            | GeminiCanvasDirectHttpImageJsonAction::TryJson => Err(download_error),
        }
    }

    pub(in crate::upstream::client) async fn build_gemini_canvas_materialized_image_response(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        request_timeout: Duration,
        selected_image_assets: &[&gemini_canvas::GeminiCanvasMediaAsset],
        provider: &str,
    ) -> Result<Value, GatewayError> {
        let downloaded = self
            .materialize_gemini_canvas_direct_http_images(
                payload,
                runtime,
                selected_image_assets,
                request_timeout,
                provider,
            )
            .await?;

        gemini_canvas::build_openai_images_response_from_bytes(req, prompt, &downloaded)
    }
}
