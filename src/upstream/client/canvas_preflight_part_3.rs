use super::*;

impl UpstreamClient {
    pub(super) async fn send_gemini_canvas_text_batchexecute_request_capture_aligned(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        request: &gemini_web::GeminiWebRequest,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        model_header: &str,
        timeout: Duration,
    ) -> Result<String, GatewayError> {
        let mut session_clone = session.clone();
        self.send_gemini_canvas_text_batchexecute_request_capture_aligned_refreshing_session(
            payload,
            model,
            request,
            &mut session_clone,
            model_header,
            timeout,
        )
        .await
    }

    pub(super) async fn send_gemini_canvas_text_batchexecute_request_refreshing_session(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        request: &gemini_web::GeminiWebRequest,
        session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
        model_header: &str,
        timeout: Duration,
    ) -> Result<String, GatewayError> {
        self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
            payload,
            model,
            request,
            session,
            model_header,
            None,
            timeout,
        )
        .await
    }

    pub(super) async fn send_gemini_canvas_text_batchexecute_request_capture_aligned_refreshing_session(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        request: &gemini_web::GeminiWebRequest,
        session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
        model_header: &str,
        timeout: Duration,
    ) -> Result<String, GatewayError> {
        self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session_internal(
            payload,
            model,
            request,
            session,
            model_header,
            None,
            timeout,
            true,
        )
        .await
    }

    pub(super) async fn send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        request: &gemini_web::GeminiWebRequest,
        session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
        model_header: &str,
        model_header_2_override: Option<&str>,
        timeout: Duration,
    ) -> Result<String, GatewayError> {
        self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session_internal(
            payload,
            model,
            request,
            session,
            model_header,
            model_header_2_override,
            timeout,
            false,
        )
        .await
    }

    pub(super) async fn send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session_internal(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        request: &gemini_web::GeminiWebRequest,
        session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
        model_header: &str,
        model_header_2_override: Option<&str>,
        timeout: Duration,
        capture_aligned_headers: bool,
    ) -> Result<String, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let configured_base_url = payload.base_url.trim_end_matches('/');
        let effective_base_url = if configured_base_url.is_empty() {
            gemini_canvas_http_origin(payload)
        } else {
            configured_base_url.to_string()
        };
        let batchexecute_url = format!("{effective_base_url}/_/BardChatUi/data/batchexecute");
        let accept_language = gemini_canvas::locale_from_payload(payload);
        let same_origin_referer = format!("{effective_base_url}/");
        let mut effective_request = request.clone();
        let mut xsrf_retry_token: Option<String> = None;
        let header2 = model_header_2_override
            .unwrap_or(gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_MODEL_HEADER_2);

        loop {
            let mut headers = gemini_web_reverse_modular::build_headers(payload, None, model);
            if capture_aligned_headers {
                apply_gemini_canvas_capture_aligned_batchexecute_headers(
                    &mut headers,
                    session,
                    &accept_language,
                    model_header,
                    header2,
                );
            } else {
                apply_gemini_canvas_same_origin_batchexecute_headers(
                    &mut headers,
                    session,
                    &accept_language,
                    model_header,
                    header2,
                );
            }
            insert_header_map_value(&mut headers, "referer", &same_origin_referer);
            insert_header_map_value(
                &mut headers,
                "x-same-domain",
                gemini_web::GEMINI_WEB_DEFAULT_SAME_DOMAIN_HEADER,
            );

            let response = self
                .http
                .request(Method::POST, &batchexecute_url)
                .headers(headers.clone())
                .query(&effective_request.query)
                .timeout(timeout.max(Duration::from_secs(30)))
                .form(&effective_request.form)
                .send()
                .await
                .map_err(|error| classify_network_error(&error, Some(provider)))?;
            apply_gemini_canvas_response_cookies(response.headers(), session);
            let status = response.status().as_u16();
            let content_type = response
                .headers()
                .get(rquest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            let body_text = response
                .text()
                .await
                .map_err(|error| classify_network_error(&error, Some(provider)))?;
            if (200..300).contains(&status)
                && !gemini_web::response_indicates_browser_challenge(
                    status,
                    content_type.as_deref(),
                    &body_text,
                )
                && !gemini_web::response_indicates_session_invalid(
                    status,
                    content_type.as_deref(),
                    &body_text,
                )
            {
                return Ok(body_text);
            }

            if status == 400 {
                if let Some(token) = extract_gemini_canvas_batchexecute_xsrf_token(&body_text) {
                    let current_at = header_map_string_from_form(&effective_request.form, "at");
                    if xsrf_retry_token.as_deref() != Some(token.as_str())
                        && current_at.as_deref() != Some(token.as_str())
                    {
                        debug!(
                            provider,
                            xsrf_token_preview = %truncate_response_preview(&token, 24),
                            "retrying gemini canvas batchexecute with xsrf token extracted from upstream error"
                        );
                        upsert_form_field(&mut effective_request.form, "at", &token);
                        xsrf_retry_token = Some(token);
                        continue;
                    }
                }
            }

            let rpcids = effective_request
                .query
                .iter()
                .find(|(key, _)| key == "rpcids")
                .map(|(_, value)| value.as_str());
            if status == 302 {
                let rpcids_log = rpcids.unwrap_or("<unknown>");
                let browser_fallback = async {
                    let runtime = gemini_canvas::runtime_from_payload(payload)?;
                    let browser_runtime_state_object_key =
                        gemini_canvas::browser_runtime_state_object_key_for_browser_operation(
                            payload, "video",
                        )
                        .unwrap_or_else(|| runtime.runtime_state_object_key.clone());
                    let browser_pool_base_url =
                        self.ensure_gemini_canvas_browser_pool(provider).await?;
                    self.execute_gemini_canvas_browser_fetch_form_request(
                        provider,
                        &browser_pool_base_url,
                        &effective_base_url,
                        &runtime.share_id,
                        &browser_runtime_state_object_key,
                        gemini_canvas::browser_cdp_url(payload).as_deref(),
                        gemini_canvas::browser_cookie_header(payload).as_deref(),
                        &batchexecute_url,
                        &effective_request.query,
                        &headers,
                        &effective_request.form,
                        timeout,
                    )
                    .await
                }
                .await;

                match browser_fallback {
                    Ok(invocation) if (200..300).contains(&invocation.status) => {
                        if let Some(browser_body) = invocation.body_text {
                            let browser_content_type = invocation.content_type.as_deref();
                            if !gemini_web::response_indicates_browser_challenge(
                                invocation.status,
                                browser_content_type,
                                &browser_body,
                            ) && !gemini_web::response_indicates_session_invalid(
                                invocation.status,
                                browser_content_type,
                                &browser_body,
                            ) {
                                debug!(
                                    provider,
                                    rpcids = rpcids_log,
                                    "recovered Gemini Canvas batchexecute through browser-backed fetch after upstream challenge redirect"
                                );
                                return Ok(browser_body);
                            }
                        }
                    }
                    Ok(invocation) => {
                        debug!(
                            provider,
                            rpcids = rpcids_log,
                            browser_status = invocation.status,
                            "Gemini Canvas batchexecute browser-backed fetch did not recover the upstream redirect"
                        );
                    }
                    Err(error) => {
                        debug!(
                            provider,
                            rpcids = rpcids_log,
                            error = %summarize_gateway_error(&error),
                            "Gemini Canvas batchexecute browser-backed fetch failed after upstream redirect"
                        );
                    }
                }
            }

            let mut classified =
                classify_gemini_canvas_pure_http_error(status, content_type.as_deref(), &body_text);
            classified.message = sanitize_provider_error_message(&format!(
                "Gemini Canvas batchexecute failed. status={status}; body_preview={}",
                compact_gemini_diagnostic_preview(&body_text, 240)
            ));
            return Err(classified);
        }
    }
}
