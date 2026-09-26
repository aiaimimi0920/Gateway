use super::*;

impl UpstreamClient {
    pub(in crate::upstream::client) async fn execute_gemini_canvas_browser_fetch_form_request(
        &self,
        provider: &str,
        browser_pool_base_url: &str,
        base_url: &str,
        share_id: &str,
        runtime_state_object_key: &str,
        browser_cdp_url: Option<&str>,
        cookie_header: Option<&str>,
        request_url: &str,
        query: &[(String, String)],
        headers: &HeaderMap,
        form: &[(String, String)],
        timeout: Duration,
    ) -> Result<GeminiCanvasBrowserFetchInvocationResult, GatewayError> {
        let final_url = append_query_pairs_to_url(request_url, query);
        let mut request_form = form.to_vec();
        let mut xsrf_retry_token: Option<String> = None;
        let referrer = header_map_string(headers, "referer")
            .or_else(|| {
                if base_url.contains("gemini.google.com") {
                    Some(format!("{}/", base_url.trim_end_matches('/')))
                } else {
                    None
                }
            })
            .unwrap_or_else(|| base_url.to_string());
        loop {
            let input =
                gemini_canvas_web_reverse_modular::build_connected_fetch_form_invocation_input(
                    base_url,
                    share_id,
                    runtime_state_object_key,
                    browser_cdp_url,
                    cookie_header,
                    &final_url,
                    &gemini_canvas_browser_fetch_headers_from_header_map(headers),
                    &serialize_form_urlencoded_pairs(&request_form),
                    &referrer,
                    timeout,
                );
            let response = self
                .http
                .request(Method::POST, format!("{}/fetch", browser_pool_base_url))
                .header(rquest::header::CONTENT_TYPE, "application/json")
                .timeout(timeout.max(Duration::from_secs(30)))
                .json(&input)
                .send()
                .await
                .map_err(|error| classify_network_error(&error, Some(provider)))?;

            let status = response.status().as_u16();
            let body_text = response
                .text()
                .await
                .map_err(|error| classify_network_error(&error, Some(provider)))?;
            match gemini_canvas_program_web_reverse_modular::parse_connected_fetch_invocation_response(
                provider, status, &body_text,
            ) {
                Ok(invocation) => return Ok(invocation),
                Err(error) => {
                    if let Some(token) = maybe_retry_gemini_canvas_form_xsrf_token(
                        &mut request_form,
                        &body_text,
                        &mut xsrf_retry_token,
                    ) {
                        debug!(
                            provider,
                            xsrf_token_preview = %truncate_response_preview(&token, 24),
                            "retrying gemini canvas browser-backed connected fetch with xsrf token extracted from upstream error"
                        );
                        continue;
                    }
                    return Err(error);
                }
            }
        }
    }

    pub(in crate::upstream::client) async fn execute_gemini_canvas_connected_fetch_invocation_with_program_context(
        &self,
        payload: &ProviderAccountPayload,
        provider: &str,
        browser_pool_base_url: &str,
        base_url: &str,
        config: &crate::protocol::gemini::canvas_program_web_reverse::GeminiCanvasProgramRelayConfig,
        runtime_api: Option<&GeminiCanvasRuntimeApiContext>,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
        request_url: &str,
        method: Method,
        request_body: Option<&Value>,
        google_fetch_mode: &str,
        timeout: Duration,
    ) -> Result<GeminiCanvasBrowserFetchInvocationResult, GatewayError> {
        let mut input = gemini_canvas_program_web_reverse_modular::build_connected_fetch_invocation_input_with_method_for_payload(
                payload,
                base_url,
                config,
                request_url,
                method.as_str(),
                request_body,
                google_fetch_mode,
                timeout,
            );
        input["authUser"] = Value::String(gemini_canvas::direct_http_auth_user(payload));
        if gemini_canvas_program_web_reverse_modular::connected_fetch_mode_is_canvas_proxy(
            google_fetch_mode,
        ) {
            gemini_canvas_program_web_reverse_modular::apply_program_connected_fetch_identity_contract(
                &mut input,
                runtime_api.map(|context| context.session.auth_user.as_str()),
                runtime_api.map(|context| context.payload.api_key.as_str()),
                extra_headers,
            )?;
        }
        let response = self
            .http
            .request(Method::POST, format!("{}/fetch", browser_pool_base_url))
            .header(rquest::header::CONTENT_TYPE, "application/json")
            .timeout(timeout.max(Duration::from_secs(30)))
            .json(&input)
            .send()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;

        let status = response.status().as_u16();
        let body_text = response
            .text()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;
        let invocation =
            gemini_canvas_program_web_reverse_modular::parse_connected_fetch_invocation_response(
                provider, status, &body_text,
            )?;
        persist_gemini_canvas_program_runtime_material(
            self.redis_pool.as_ref(),
            self.pg_pool.as_ref(),
            payload,
            gemini_canvas_program_web_reverse_modular::runtime_patch_from_browser_fetch(
                &invocation,
            ),
        )
        .await;
        Ok(invocation)
    }

    pub(in crate::upstream::client) async fn execute_gemini_canvas_connected_fetch_get_bytes_with_program_context(
        &self,
        payload: &ProviderAccountPayload,
        provider: &str,
        browser_pool_base_url: &str,
        base_url: &str,
        config: &crate::protocol::gemini::canvas_program_web_reverse::GeminiCanvasProgramRelayConfig,
        runtime_api: Option<&GeminiCanvasRuntimeApiContext>,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
        request_url: &str,
        google_fetch_mode: &str,
        timeout: Duration,
    ) -> Result<(bytes::Bytes, Option<String>), GatewayError> {
        let invocation = self
            .execute_gemini_canvas_connected_fetch_invocation_with_program_context(
                payload,
                provider,
                browser_pool_base_url,
                base_url,
                config,
                runtime_api,
                extra_headers,
                request_url,
                Method::GET,
                None,
                google_fetch_mode,
                timeout,
            )
            .await?;
        gemini_canvas_program_web_reverse_modular::decode_connected_fetch_body_bytes(
            provider,
            &invocation,
        )
    }

    pub(in crate::upstream::client) async fn execute_gemini_canvas_browser_request_with_recovery(
        &self,
        provider: &str,
        browser_pool_base_url: &str,
        base_url: &str,
        share_id: &str,
        runtime_state_object_key: &str,
        browser_cdp_url: Option<&str>,
        cookie_header: Option<&str>,
        operation: &str,
        prompt: &str,
        locale: &str,
        timeout: Duration,
    ) -> Result<GeminiCanvasBrowserInvocationResult, GatewayError> {
        gemini_canvas_web_reverse_modular::execute_browser_request_with_recovery(
            &self.http,
            timeout,
            provider,
            browser_pool_base_url,
            base_url,
            share_id,
            runtime_state_object_key,
            browser_cdp_url,
            cookie_header,
            operation,
            prompt,
            locale,
        )
        .await
    }

    pub(in crate::upstream::client) async fn persist_gemini_canvas_program_runtime_result_if_needed(
        &self,
        payload: &ProviderAccountPayload,
        result: &GeminiCanvasBrowserInvocationResult,
    ) {
        if payload.adapter != "gemini_canvas_program_web_reverse_compatible" {
            return;
        }
        persist_gemini_canvas_program_runtime_material(
            self.redis_pool.as_ref(),
            self.pg_pool.as_ref(),
            payload,
            gemini_canvas_program_web_reverse_modular::runtime_patch_from_browser_invocation(
                result,
            ),
        )
        .await;
    }

    pub(in crate::upstream::client) async fn execute_gemini_canvas_owned_browser_invocation(
        &self,
        payload: &ProviderAccountPayload,
        provider: &str,
        browser_pool_base_url: &str,
        base_url: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        program_config: Option<
            &crate::protocol::gemini::canvas_program_web_reverse::GeminiCanvasProgramRelayConfig,
        >,
        operation: &str,
        prompt: &str,
        locale: &str,
        timeout: Duration,
    ) -> Result<GeminiCanvasBrowserInvocationResult, GatewayError> {
        let browser_runtime_state_object_key =
            gemini_canvas::browser_runtime_state_object_key_for_browser_operation(
                payload, operation,
            )
            .unwrap_or_else(|| runtime.runtime_state_object_key.clone());
        let browser_cookie_header = gemini_canvas::browser_cookie_header(payload);
        let result = if let Some(config) = program_config {
            self.execute_gemini_canvas_browser_request_with_program_context_recovery(
                provider,
                browser_pool_base_url,
                base_url,
                config,
                &browser_runtime_state_object_key,
                operation,
                prompt,
                locale,
                timeout,
            )
            .await?
        } else {
            self.execute_gemini_canvas_browser_request_with_recovery(
                provider,
                browser_pool_base_url,
                base_url,
                &runtime.share_id,
                &browser_runtime_state_object_key,
                gemini_canvas::browser_cdp_url(payload).as_deref(),
                browser_cookie_header.as_deref(),
                operation,
                prompt,
                locale,
                timeout,
            )
            .await?
        };
        self.persist_gemini_canvas_program_runtime_result_if_needed(payload, &result)
            .await;
        Ok(result)
    }
}
