use super::*;

impl UpstreamClient {
    pub(in crate::upstream::client) async fn try_resolve_gemini_canvas_image_edit_async_followups(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        primary_body: &str,
        timeout: Duration,
        mut image_edit_followup_context: Option<&mut GeminiCanvasImageEditFollowupContext>,
        primary_error: &mut GatewayError,
    ) -> Result<Option<(Vec<gemini_canvas::GeminiCanvasMediaAsset>, String)>, GatewayError> {
        append_gemini_canvas_image_edit_trace("followup.signaler.start", || {
            compact_sanitized_response_preview(primary_body, 180)
        });
        if let Some(context) = image_edit_followup_context.as_deref_mut() {
            if context.signaler_response_id.is_none() {
                if let Some(response_id) =
                    gemini_canvas::extract_stream_generate_response_id(primary_body).ok()
                {
                    context.signaler_response_id = Some(response_id);
                }
            }
            if context.signaler_conversation_id.is_none() {
                if let Some(locator) =
                    gemini_canvas::extract_stream_generate_locator(primary_body).ok()
                {
                    context.signaler_conversation_id = Some(locator.conversation_id);
                    if context.signaler_response_id.is_none() {
                        context.signaler_response_id = Some(locator.response_id);
                    }
                }
            }
        }
        let image_edit_locale_hint = image_edit_followup_context
            .as_ref()
            .and_then(|context| context.locale_hint.clone());
        match self
            .poll_gemini_canvas_image_edit_signaler_assets(
                payload,
                model,
                runtime,
                timeout,
                image_edit_followup_context.as_deref_mut(),
            )
            .await
        {
            Ok((assets, body)) => {
                append_gemini_canvas_image_edit_trace("followup.signaler.ok", || {
                    compact_sanitized_response_preview(&body, 220)
                });
                return Ok(Some((assets, body)));
            }
            Err(signaler_error) => {
                append_gemini_canvas_image_edit_trace("followup.signaler.err", || {
                    summarize_gateway_error(&signaler_error)
                });
                append_gateway_error_fields(
                    primary_error,
                    &[(
                        "signaler_followup_failure",
                        summarize_gateway_error(&signaler_error),
                    )],
                );
            }
        }

        if let Some(edit_context) = image_edit_followup_context.as_deref_mut() {
            match self
                .execute_gemini_canvas_image_edit_post_ack_followups(
                    payload,
                    model,
                    runtime,
                    primary_body,
                    timeout,
                    edit_context,
                )
                .await
            {
                Ok(post_ack_result) => {
                    append_gemini_canvas_image_edit_trace("followup.post-ack.ok", || {
                        compact_sanitized_response_preview(&post_ack_result.preview, 220)
                    });
                    append_gateway_error_fields(
                        primary_error,
                        &[(
                            "post_ack_followup_preview",
                            compact_gemini_diagnostic_preview(&post_ack_result.preview, 220),
                        )],
                    );
                    if let Some(page_body) = post_ack_result.page_body {
                        if let Ok(assets) = gemini_canvas::extract_page_blob_media_assets(
                            &page_body,
                            gemini_canvas::GeminiCanvasMediaOperation::Image,
                        ) {
                            return Ok(Some((assets, page_body)));
                        }
                    }
                }
                Err(post_ack_error) => {
                    append_gemini_canvas_image_edit_trace("followup.post-ack.err", || {
                        summarize_gateway_error(&post_ack_error)
                    });
                    append_gateway_error_fields(
                        primary_error,
                        &[(
                            "post_ack_followup_failure",
                            summarize_gateway_error(&post_ack_error),
                        )],
                    );
                }
            }
            append_gemini_canvas_image_edit_trace("followup.conversation.start", || {
                edit_context
                    .signaler_app_url
                    .clone()
                    .unwrap_or_else(|| "<none>".to_string())
            });
            match self
                .poll_gemini_canvas_image_edit_conversation_assets(
                    payload,
                    model,
                    runtime,
                    timeout,
                    edit_context,
                )
                .await
            {
                Ok((assets, body)) => {
                    append_gemini_canvas_image_edit_trace("followup.conversation.ok", || {
                        compact_sanitized_response_preview(&body, 220)
                    });
                    return Ok(Some((assets, body)));
                }
                Err(conversation_error) => {
                    append_gemini_canvas_image_edit_trace("followup.conversation.err", || {
                        summarize_gateway_error(&conversation_error)
                    });
                    append_gateway_error_fields(
                        primary_error,
                        &[(
                            "conversation_followup_failure",
                            summarize_gateway_error(&conversation_error),
                        )],
                    );
                }
            }
        }

        let signaler_session = image_edit_followup_context
            .as_ref()
            .and_then(|context| context.signaler_session.as_ref());
        let signaler_app_url = image_edit_followup_context
            .as_ref()
            .and_then(|context| context.signaler_app_url.as_deref());
        match self
            .poll_gemini_canvas_media_assets_from_conversation_page(
                payload,
                model,
                runtime,
                gemini_canvas::GeminiCanvasMediaOperation::Image,
                primary_body,
                timeout,
                true,
                image_edit_locale_hint.as_deref(),
                signaler_session,
                signaler_app_url,
            )
            .await
        {
            Ok((assets, page_body)) => {
                append_gemini_canvas_image_edit_trace("followup.page.ok", || {
                    compact_sanitized_response_preview(&page_body, 220)
                });
                Ok(Some((assets, page_body)))
            }
            Err(page_poll_error) => {
                append_gemini_canvas_image_edit_trace("followup.page.err", || {
                    summarize_gateway_error(&page_poll_error)
                });
                append_gateway_error_fields(
                    primary_error,
                    &[(
                        "early_media_page_poll_failure",
                        summarize_gateway_error(&page_poll_error),
                    )],
                );
                Ok(None)
            }
        }
    }
}
