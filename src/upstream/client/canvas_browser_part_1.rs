use super::*;

#[path = "canvas_browser_part_1/browser_invocation.rs"]
mod browser_invocation;

impl UpstreamClient {
    pub(super) async fn collect_gemini_canvas_stream_generate_body(
        &self,
        response: rquest::Response,
        provider: &str,
        operation: gemini_canvas::GeminiCanvasMediaOperation,
        allow_early_locator: bool,
    ) -> Result<String, GatewayError> {
        const IMAGE_EDIT_SHORT_ACK_GRACE_SECS: u64 = 30;
        const IMAGE_EDIT_ASYNC_FOLLOWUP_GRACE_SECS: u64 = 45;
        let mut stream = response.bytes_stream();
        let mut pending_utf8 = Vec::new();
        let mut body_text = String::new();
        let mut short_ack_seen_at: Option<Instant> = None;
        let mut async_followup_ready_seen_at: Option<Instant> = None;

        loop {
            let handoff_wait = if allow_early_locator
                && operation == gemini_canvas::GeminiCanvasMediaOperation::Image
            {
                if let Some(seen_at) = async_followup_ready_seen_at {
                    Some((
                        "stream.async-followup-handoff",
                        Duration::from_secs(IMAGE_EDIT_ASYNC_FOLLOWUP_GRACE_SECS)
                            .saturating_sub(seen_at.elapsed()),
                    ))
                } else if let Some(seen_at) = short_ack_seen_at {
                    Some((
                        "stream.short-ack-handoff",
                        Duration::from_secs(IMAGE_EDIT_SHORT_ACK_GRACE_SECS)
                            .saturating_sub(seen_at.elapsed()),
                    ))
                } else {
                    None
                }
            } else {
                None
            };

            let next_chunk = if let Some((trace_label, wait)) = handoff_wait {
                if wait.is_zero() {
                    append_gemini_canvas_image_edit_trace(trace_label, || {
                        compact_sanitized_response_preview(&body_text, 220)
                    });
                    return Ok(body_text);
                }
                match timeout(wait, stream.next()).await {
                    Ok(chunk_result) => chunk_result,
                    Err(_) => {
                        append_gemini_canvas_image_edit_trace(trace_label, || {
                            compact_sanitized_response_preview(&body_text, 220)
                        });
                        return Ok(body_text);
                    }
                }
            } else {
                stream.next().await
            };

            let Some(chunk_result) = next_chunk else {
                break;
            };
            let chunk = match chunk_result {
                Ok(chunk) => chunk,
                Err(error) => {
                    if !body_text.is_empty() {
                        return Ok(body_text);
                    }
                    return Err(classify_network_error(&error, Some(provider)));
                }
            };
            append_utf8(&mut body_text, &mut pending_utf8, &chunk);

            if gemini_canvas::extract_stream_generate_media_assets(&body_text, operation).is_ok() {
                if allow_early_locator
                    && operation == gemini_canvas::GeminiCanvasMediaOperation::Image
                {
                    append_gemini_canvas_image_edit_trace("stream.asset-ready", || {
                        compact_sanitized_response_preview(&body_text, 220)
                    });
                }
                return Ok(body_text);
            }
            if allow_early_locator
                && operation == gemini_canvas::GeminiCanvasMediaOperation::Image
                && gemini_canvas::stream_generate_indicates_image_edit_async_followup_ready(
                    &body_text,
                )
            {
                if async_followup_ready_seen_at.is_none() {
                    async_followup_ready_seen_at = Some(Instant::now());
                    append_gemini_canvas_image_edit_trace("stream.async-followup-ready", || {
                        compact_sanitized_response_preview(&body_text, 220)
                    });
                }
            }
            if allow_early_locator
                && operation == gemini_canvas::GeminiCanvasMediaOperation::Image
                && async_followup_ready_seen_at.is_none()
                && gemini_canvas::stream_generate_is_image_edit_short_ack(&body_text)
            {
                if short_ack_seen_at.is_none() {
                    short_ack_seen_at = Some(Instant::now());
                    append_gemini_canvas_image_edit_trace("stream.short-ack-observed", || {
                        compact_sanitized_response_preview(&body_text, 220)
                    });
                }
            }
            if allow_early_locator
                && operation != gemini_canvas::GeminiCanvasMediaOperation::Image
                && gemini_canvas::extract_stream_generate_locator(&body_text).is_ok()
            {
                return Ok(body_text);
            }
        }

        if !pending_utf8.is_empty() {
            body_text.push(char::REPLACEMENT_CHARACTER);
        }
        Ok(body_text)
    }

    pub(super) async fn execute_gemini_canvas_direct_http_stream_generate_text(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        timeout: Duration,
    ) -> Result<CanonicalRelayResponse, GatewayError> {
        let wrap_parse_error = wrap_gemini_canvas_stream_parse_error;

        let mut initial_stream_send_failure: Option<String> = None;
        let first_body = match self
            .execute_gemini_canvas_direct_http_stream_generate_body(
                payload,
                model,
                runtime,
                gemini_canvas::GEMINI_CANVAS_TEXT_STREAM_GENERATE_TEXT_MODE_INDEX,
                prompt,
                timeout,
                true,
                None,
                None,
            )
            .await
        {
            Ok(body) => body,
            Err(first_send_error) => {
                let first_send_summary = summarize_gateway_error(&first_send_error);
                initial_stream_send_failure = Some(first_send_summary.clone());
                debug!(
                    provider = "gemini_canvas_compatible",
                    error = %first_send_summary,
                    "Gemini Canvas StreamGenerate primary direct HTTP send failed before parsing; retrying legacy pure HTTP builder"
                );
                self.execute_gemini_canvas_direct_http_stream_generate_body(
                    payload,
                    model,
                    runtime,
                    gemini_canvas::GEMINI_CANVAS_TEXT_STREAM_GENERATE_TEXT_MODE_INDEX,
                    prompt,
                    timeout,
                    false,
                    None,
                    None,
                )
                .await
                .map_err(|retry_error| {
                    let mut retry_error = retry_error;
                    retry_error.message = sanitize_provider_error_message(&format!(
                        "{}; initial_stream_send_failure={first_send_summary}",
                        retry_error.message
                    ));
                    retry_error
                })?
            }
        };
        let canonical = match gemini_web::accumulate_gemini_web_response(&first_body, model) {
            Ok(canonical) => canonical,
            Err(first_error) => {
                let first_error = wrap_parse_error(&first_body, first_error);
                debug!(
                    provider = "gemini_canvas_compatible",
                    error = %summarize_gateway_error(&first_error),
                    "Gemini Canvas StreamGenerate replay did not yield a usable text candidate; retrying legacy pure HTTP builder"
                );
                let retry_body = self
                    .execute_gemini_canvas_direct_http_stream_generate_body(
                        payload,
                        model,
                        runtime,
                        gemini_canvas::GEMINI_CANVAS_TEXT_STREAM_GENERATE_TEXT_MODE_INDEX,
                        prompt,
                        timeout,
                        false,
                        None,
                        None,
                    )
                    .await
                    .map_err(|retry_error| {
                        let mut retry_error = retry_error;
                        let mut retry_detail = format!(
                            "initial_stream_parse_failure={}",
                            summarize_gateway_error(&first_error)
                        );
                        if let Some(initial_send_failure) = initial_stream_send_failure.as_deref() {
                            retry_detail = format!(
                                "{retry_detail}; initial_stream_send_failure={initial_send_failure}"
                            );
                        }
                        retry_error.message = sanitize_provider_error_message(&format!(
                            "{}; {retry_detail}",
                            retry_error.message
                        ));
                        retry_error
                    })?;
                gemini_web::accumulate_gemini_web_response(&retry_body, model).map_err(
                    |retry_parse_error| {
                        let mut retry_parse_error =
                            wrap_parse_error(&retry_body, retry_parse_error);
                        let mut retry_detail = format!(
                            "initial_stream_parse_failure={}",
                            summarize_gateway_error(&first_error)
                        );
                        if let Some(initial_send_failure) = initial_stream_send_failure.as_deref() {
                            retry_detail = format!(
                                "{retry_detail}; initial_stream_send_failure={initial_send_failure}"
                            );
                        }
                        retry_parse_error.message = sanitize_provider_error_message(&format!(
                            "{}; {retry_detail}",
                            retry_parse_error.message
                        ));
                        retry_parse_error
                    },
                )?
            }
        };
        let _ = req;
        Ok(canonical)
    }
}

// Only an incomplete final code point crosses a transport chunk boundary.
fn append_utf8(text: &mut String, pending: &mut Vec<u8>, chunk: &[u8]) {
    pending.extend_from_slice(chunk);
    let mut bytes = pending.as_slice();
    while !bytes.is_empty() {
        match std::str::from_utf8(bytes) {
            Ok(valid) => {
                text.push_str(valid);
                bytes = &[];
            }
            Err(error) => {
                let (valid, tail) = bytes.split_at(error.valid_up_to());
                text.push_str(std::str::from_utf8(valid).expect("validated prefix"));
                let Some(length) = error.error_len() else {
                    bytes = tail;
                    break;
                };
                text.push(char::REPLACEMENT_CHARACTER);
                bytes = &tail[length..];
            }
        }
    }
    let consumed = pending.len() - bytes.len();
    pending.drain(..consumed);
}

#[test]
fn canvas_incremental_utf8_matches_whole_body_at_every_split() {
    let bytes = b"a\xf0\x9f\x90\xbe\xe4\xb8\xad\xff\xc2";
    for split in 0..=bytes.len() {
        let (mut text, mut pending) = (String::new(), Vec::new());
        append_utf8(&mut text, &mut pending, &bytes[..split]);
        assert!(pending.len() <= 3);
        append_utf8(&mut text, &mut pending, &bytes[split..]);
        text.push_str(&String::from_utf8_lossy(&pending));
        assert_eq!(text, String::from_utf8_lossy(bytes));
    }
}
