use super::*;

impl UpstreamClient {
    pub(super) async fn execute_gemini_canvas_video_continuation(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        continuation: GeminiCanvasVideoContinuation,
        timeout: Duration,
    ) -> Result<Value, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let locator = gemini_canvas::GeminiCanvasStreamGenerateLocator {
            conversation_id: continuation.conversation_id.clone(),
            response_id: continuation.response_id.clone(),
            app_path: continuation.app_path.clone(),
        };
        let seed_body = build_gemini_canvas_video_continuation_seed_body(&continuation);
        let followup_body = match self
            .execute_gemini_canvas_direct_http_media_followup_body(
                payload,
                model,
                runtime,
                gemini_canvas::GeminiCanvasMediaOperation::Video,
                prompt,
                &seed_body,
                Some(locator),
                SystemTime::now(),
                timeout,
                false,
                None,
            )
            .await
        {
            Ok(body) => body,
            Err(error) if should_preserve_gemini_canvas_video_continuation_as_pending(&error) => {
                debug!(
                    provider,
                    error = %summarize_gateway_error(&error),
                    "gemini canvas video continuation remains pending"
                );
                return Ok(build_gemini_canvas_video_accepted_response_from_body(
                    model,
                    prompt,
                    &seed_body,
                    Some(&continuation.conversation_id),
                    Some(&continuation.response_id),
                    Some(&continuation.app_path),
                ));
            }
            Err(error) => return Err(error),
        };

        let assets = gemini_canvas::extract_stream_generate_media_assets(
            &followup_body,
            gemini_canvas::GeminiCanvasMediaOperation::Video,
        )
        .or_else(|_| {
            gemini_canvas::extract_page_blob_media_assets(
                &followup_body,
                gemini_canvas::GeminiCanvasMediaOperation::Video,
            )
        });
        let assets = match assets {
            Ok(assets) if !assets.is_empty() => assets,
            _ if gemini_canvas::response_indicates_video_generation_pending(&followup_body)
                || gemini_canvas::response_indicates_video_generation_quota_reached(
                    &followup_body,
                ) =>
            {
                return Ok(build_gemini_canvas_video_accepted_response_from_body(
                    model,
                    prompt,
                    &followup_body,
                    Some(&continuation.conversation_id),
                    Some(&continuation.response_id),
                    Some(&continuation.app_path),
                ));
            }
            _ => {
                return Err(build_gemini_canvas_direct_http_video_missing_asset_error(
                    provider,
                ));
            }
        };
        if gemini_canvas::video_body_indicates_music_modality_mismatch(&followup_body) {
            return Err(gemini_canvas_video_music_modality_mismatch_error(provider));
        }
        let asset = assets
            .first()
            .expect("non-empty continuation assets were checked above");
        let asset = if asset.body_base64.is_some() {
            asset.clone()
        } else {
            self.materialize_gemini_canvas_direct_http_media_asset(
                payload,
                runtime,
                &asset.url,
                Some(asset.kind.as_str()),
                Some(asset.mime_type.as_str()),
                timeout,
            )
            .await?
        };
        Ok(gemini_canvas::build_video_generation_response(
            model,
            prompt,
            &asset,
            Some(&followup_body),
        ))
    }
}
