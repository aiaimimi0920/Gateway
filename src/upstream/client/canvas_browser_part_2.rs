use super::*;

impl UpstreamClient {
    pub(super) async fn execute_gemini_canvas_remote_or_owned_browser_invocation(
        &self,
        provider_account_id: &str,
        payload: &ProviderAccountPayload,
        provider: &str,
        endpoint_kind: EndpointKind,
        invocation_input: Value,
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
        let skip_remote_browser_executor = payload.adapter
            == "gemini_canvas_program_web_reverse_compatible"
            && endpoint_kind == EndpointKind::VideosGenerations;

        if !skip_remote_browser_executor {
            if let Some(result) = self
                .execute_remote_browser_executor(
                    "gemini_canvas",
                    provider_account_id,
                    endpoint_kind,
                    invocation_input,
                )
                .await?
            {
                return gemini_canvas_web_reverse_modular::parse_remote_modular_media_browser_invocation_value(
                    provider,
                    result,
                    operation,
                );
            }
        }
        self.execute_gemini_canvas_owned_browser_invocation(
            payload,
            provider,
            browser_pool_base_url,
            base_url,
            runtime,
            program_config,
            operation,
            prompt,
            locale,
            timeout,
        )
        .await
    }

    pub(super) async fn execute_gemini_canvas_modular_media_browser_result(
        &self,
        provider_account_id: &str,
        payload: &ProviderAccountPayload,
        provider: &str,
        endpoint_kind: EndpointKind,
        base_url: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        program_config: Option<
            &crate::protocol::gemini::canvas_program_web_reverse::GeminiCanvasProgramRelayConfig,
        >,
        operation: &str,
        prompt: &str,
        locale: &str,
        request_timeout: Duration,
    ) -> Result<GeminiCanvasBrowserInvocationResult, GatewayError> {
        let invocation_input = if payload.adapter == "gemini_canvas_program_web_reverse_compatible"
        {
            gemini_canvas_program_web_reverse_modular::build_browser_operation_invocation_input(
                payload,
                operation,
                prompt,
                locale,
                request_timeout,
            )?
        } else {
            gemini_canvas_web_reverse_modular::build_browser_operation_invocation_input(
                payload,
                operation,
                prompt,
                locale,
                request_timeout,
            )?
        };
        let browser_pool_base_url = self.ensure_gemini_canvas_browser_pool(provider).await?;
        self.execute_gemini_canvas_remote_or_owned_browser_invocation(
            provider_account_id,
            payload,
            provider,
            endpoint_kind,
            invocation_input,
            &browser_pool_base_url,
            base_url,
            runtime,
            program_config,
            operation,
            prompt,
            locale,
            request_timeout,
        )
        .await
    }

    pub(super) async fn execute_gemini_canvas_browser_request_with_program_context(
        &self,
        provider: &str,
        browser_pool_base_url: &str,
        base_url: &str,
        config: &crate::protocol::gemini::canvas_program_web_reverse::GeminiCanvasProgramRelayConfig,
        runtime_state_object_key: &str,
        operation: &str,
        prompt: &str,
        locale: &str,
        timeout: Duration,
    ) -> Result<GeminiCanvasBrowserInvocationResult, GatewayError> {
        let input = gemini_canvas_program_web_reverse_modular::build_browser_operation_invocation_input_from_config(
            base_url,
            config,
            runtime_state_object_key,
            operation,
            prompt,
            locale,
            timeout,
        );
        let response = self
            .http
            .request(Method::POST, format!("{}/invoke", browser_pool_base_url))
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
        gemini_canvas_program_web_reverse_modular::parse_program_browser_invocation_response(
            provider, status, &body_text, operation,
        )
    }

    pub(super) async fn execute_gemini_canvas_browser_request_with_program_context_recovery(
        &self,
        provider: &str,
        browser_pool_base_url: &str,
        base_url: &str,
        config: &crate::protocol::gemini::canvas_program_web_reverse::GeminiCanvasProgramRelayConfig,
        runtime_state_object_key: &str,
        operation: &str,
        prompt: &str,
        locale: &str,
        timeout: Duration,
    ) -> Result<GeminiCanvasBrowserInvocationResult, GatewayError> {
        let mut attempts = 0usize;
        loop {
            match self
                .execute_gemini_canvas_browser_request_with_program_context(
                    provider,
                    browser_pool_base_url,
                    base_url,
                    config,
                    runtime_state_object_key,
                    operation,
                    prompt,
                    locale,
                    timeout,
                )
                .await
            {
                Ok(result) => return Ok(result),
                Err(error) => {
                    let code = error.code.as_deref();
                    let explicit_quota_gate =
                        matches!(code, Some("gemini_canvas_video_quota_reached"));
                    let retryable = matches!(
                        code,
                        Some("gemini_canvas_context_busy")
                            | Some("gemini_canvas_auth_required")
                            | Some("gemini_canvas_browser_worker_failed")
                            | Some("gemini_canvas_program_auth_redirect")
                    ) || (error.http_status == Some(429) && !explicit_quota_gate);
                    if !retryable || attempts >= 2 {
                        return Err(error);
                    }
                    let delay_ms = match code {
                        Some("gemini_canvas_auth_required")
                        | Some("gemini_canvas_program_auth_redirect") => 2_000,
                        Some("gemini_canvas_context_busy") => 3_000 + (attempts as u64 * 1_500),
                        _ => 2_500,
                    };
                    attempts += 1;
                    sleep(Duration::from_millis(delay_ms)).await;
                }
            }
        }
    }

    pub(super) async fn execute_gemini_canvas_program_bootstrap_request(
        &self,
        provider: &str,
        browser_pool_base_url: &str,
        payload: &ProviderAccountPayload,
        bootstrap_operation: &str,
        bootstrap_prompt: &str,
        locale: &str,
        timeout: Duration,
    ) -> Result<GeminiCanvasBrowserInvocationResult, GatewayError> {
        let input =
            gemini_canvas_program_web_reverse_modular::build_program_bootstrap_invocation_input(
                payload,
                bootstrap_operation,
                bootstrap_prompt,
                locale,
                timeout,
            )?;
        let response = self
            .http
            .request(Method::POST, format!("{}/invoke", browser_pool_base_url))
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
        gemini_canvas_program_web_reverse_modular::parse_program_bootstrap_invocation_response(
            provider, status, &body_text,
        )
    }

    pub(super) async fn try_ensure_gemini_canvas_program_payload_handle_pure_http(
        &self,
        payload: &ProviderAccountPayload,
        bootstrap_operation: &str,
        locale: &str,
        timeout: Duration,
    ) -> Result<ProviderAccountPayload, GatewayError> {
        let provider = "gemini_canvas_program_web_reverse_compatible";
        let runtime = gemini_canvas::runtime_from_payload(payload)?;
        let config = gemini_canvas_program_web_reverse_modular::relay_config_from_payload(payload)?;
        let base_url = payload.base_url.trim_end_matches('/');
        let share_url = format!("{base_url}/share/{}", config.bootstrap.share_id);
        let storage_state = gateway_object_storage()?
            .read_json(&runtime.runtime_state_object_key)
            .await?;
        let auth_user = gemini_canvas::direct_http_auth_user(payload);
        let mut session = gemini_canvas::storage_state_to_pure_http_session(
            &storage_state,
            &share_url,
            base_url,
            &auth_user,
        )?;

        let mut share_headers = HeaderMap::new();
        apply_gemini_canvas_navigation_headers(&mut share_headers);
        apply_gemini_canvas_cookie_header(&mut share_headers, &session);
        insert_header_map_value(&mut share_headers, "accept-language", locale);

        let share_response = self
            .http
            .request(Method::GET, &share_url)
            .headers(share_headers)
            .timeout(timeout.max(Duration::from_secs(30)))
            .send()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;
        apply_gemini_canvas_response_cookies(share_response.headers(), &mut session);
        let share_status = share_response.status().as_u16();
        let share_content_type = share_response
            .headers()
            .get(rquest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let share_body = share_response
            .text()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;
        if !(200..300).contains(&share_status)
            || gemini_web::response_indicates_browser_challenge(
                share_status,
                share_content_type.as_deref(),
                &share_body,
            )
            || gemini_web::response_indicates_session_invalid(
                share_status,
                share_content_type.as_deref(),
                &share_body,
            )
        {
            return Err(classify_gemini_canvas_pure_http_error(
                share_status,
                share_content_type.as_deref(),
                &share_body,
            ));
        }

        let fallback_bootstrap =
            gemini_web::bootstrap_from_payload_cache(payload.extra_body.as_ref());
        let bootstrap = gemini_web::parse_bootstrap_from_app_html(
            &share_body,
            payload
                .extra_body
                .as_ref()
                .and_then(|extra| extra.get("language").and_then(Value::as_str)),
        )
        .or_else(|primary_error| fallback_bootstrap.clone().ok_or(primary_error))?;
        let bootstrap =
            gemini_web::merge_bootstrap_from_fallback(bootstrap, fallback_bootstrap.as_ref());
        let mut create_request =
            gemini_canvas_program_web_reverse_modular::build_program_create_pure_http_request(
                &config.bootstrap.share_id,
                &bootstrap,
                gemini_canvas_program_web_reverse_modular::current_program_create_reqid(),
            )?;
        let request_url = format!("{base_url}/_/BardChatUi/data/batchexecute");
        let origin = url::Url::parse(base_url)
            .map(|parsed| parsed.origin().ascii_serialization())
            .unwrap_or_else(|_| base_url.to_string());
        let authorization = gemini_canvas::build_sapisid_authorization(
            &session.sapisid,
            &origin,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
        )?;
        let mut xsrf_retry_token: Option<String> = None;

        loop {
            let mut headers = HeaderMap::new();
            insert_header_map_value(&mut headers, "accept", "*/*");
            insert_header_map_value(
                &mut headers,
                "content-type",
                "application/x-www-form-urlencoded;charset=UTF-8",
            );
            insert_header_map_value(&mut headers, "accept-language", &bootstrap.language);
            insert_header_map_value(&mut headers, "origin", &origin);
            insert_header_map_value(&mut headers, "referer", &share_url);
            insert_header_map_value(
                &mut headers,
                "user-agent",
                gemini_web::GEMINI_WEB_DEFAULT_USER_AGENT,
            );
            insert_header_map_value(
                &mut headers,
                "x-same-domain",
                gemini_web::GEMINI_WEB_DEFAULT_SAME_DOMAIN_HEADER,
            );
            apply_browser_fetch_client_hints(&mut headers, &request_url, base_url);
            apply_gemini_canvas_signed_headers(
                &mut headers,
                &session,
                &origin,
                &share_url,
                &authorization,
                true,
            );

            let response = self
                .http
                .request(Method::POST, &request_url)
                .headers(headers)
                .query(&create_request.query)
                .timeout(timeout.max(Duration::from_secs(30)))
                .form(&create_request.form)
                .send()
                .await
                .map_err(|error| classify_network_error(&error, Some(provider)))?;
            apply_gemini_canvas_response_cookies(response.headers(), &mut session);
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
                let patch = gemini_canvas_program_web_reverse_modular::runtime_patch_from_pure_http_create_response(
                    &config.bootstrap.share_id,
                    Some(bootstrap_operation),
                    &body_text,
                    base_url,
                )
                .ok_or_else(|| {
                    gemini_canvas_program_web_reverse_modular::gemini_canvas_program_create_missing_handle_patch_error(
                        provider,
                    )
                })?;
                let ensured_payload = merge_extra_body_patch_into_payload(payload, &patch);
                persist_gemini_canvas_program_runtime_material(
                    self.redis_pool.as_ref(),
                    self.pg_pool.as_ref(),
                    payload,
                    Some(patch),
                )
                .await;
                return Ok(ensured_payload);
            }

            if status == 400 {
                if let Some(token) = extract_gemini_canvas_batchexecute_xsrf_token(&body_text) {
                    let current_at = header_map_string_from_form(&create_request.form, "at");
                    if xsrf_retry_token.as_deref() != Some(token.as_str())
                        && current_at.as_deref() != Some(token.as_str())
                    {
                        debug!(
                            provider,
                            xsrf_token_preview = %truncate_response_preview(&token, 24),
                            "retrying Gemini Canvas pure HTTP program create with xsrf token extracted from upstream error"
                        );
                        upsert_form_field(&mut create_request.form, "at", &token);
                        xsrf_retry_token = Some(token);
                        continue;
                    }
                }
            }

            return Err(classify_gemini_canvas_pure_http_error(
                status,
                content_type.as_deref(),
                &body_text,
            ));
        }
    }
}
