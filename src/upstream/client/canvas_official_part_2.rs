use super::*;

impl UpstreamClient {
    pub(super) async fn execute_gemini_canvas_http_replay_worker(
        &self,
        provider: &str,
        template: &gemini_canvas::GeminiCanvasTextStreamGenerateTemplate,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        operation: Option<&str>,
        timeout: Duration,
    ) -> Result<GeminiCanvasHttpReplayWorkerSuccess, GatewayError> {
        let operation_kind = operation.and_then(|value| match value {
            "image" => Some(gemini_canvas::GeminiCanvasMediaOperation::Image),
            "music" => Some(gemini_canvas::GeminiCanvasMediaOperation::Music),
            "video" => Some(gemini_canvas::GeminiCanvasMediaOperation::Video),
            _ => None,
        });
        let mut effective_template = template.clone();
        let mut xsrf_retry_attempted = false;

        loop {
            if let Some(operation_kind) = operation_kind {
                let mut headers = HeaderMap::new();
                for (key, value) in &effective_template.headers {
                    insert_header_map_value(&mut headers, key, value);
                }
                apply_gemini_canvas_cookie_header(&mut headers, session);
                match self
                    .http
                    .request(Method::POST, &effective_template.url)
                    .headers(headers)
                    .query(&effective_template.query)
                    .timeout(timeout)
                    .body(effective_template.raw_post_data.clone())
                    .send()
                    .await
                {
                    Ok(response) => {
                        let status = response.status().as_u16();
                        let content_type = response
                            .headers()
                            .get(rquest::header::CONTENT_TYPE)
                            .and_then(|value| value.to_str().ok())
                            .map(str::to_string);
                        let body_text = self
                            .collect_gemini_canvas_stream_generate_body(
                                response,
                                provider,
                                operation_kind,
                                operation_kind != gemini_canvas::GeminiCanvasMediaOperation::Music,
                            )
                            .await?;
                        if status == 400 {
                            if let Some(token) =
                                maybe_retry_gemini_canvas_stream_template_access_token(
                                    &mut effective_template,
                                    &body_text,
                                    &mut xsrf_retry_attempted,
                                )
                            {
                                debug!(
                                    provider,
                                    operation = operation.unwrap_or(""),
                                    xsrf_token_preview = %truncate_response_preview(&token, 24),
                                    "retrying gemini canvas HTTP replay worker StreamGenerate with xsrf token extracted from upstream error"
                                );
                                continue;
                            }
                        }
                        if !body_text.is_empty() {
                            return Ok(GeminiCanvasHttpReplayWorkerSuccess {
                                status,
                                content_type,
                                body_text,
                            });
                        }
                        if status >= 400 {
                            debug!(
                                provider,
                                operation = operation.unwrap_or(""),
                                status,
                                "gemini canvas direct Rust StreamGenerate replay returned an empty error body; falling back to node worker for exact response capture"
                            );
                        } else {
                            debug!(
                                provider,
                                operation = operation.unwrap_or(""),
                                "gemini canvas direct Rust StreamGenerate replay returned an empty body; falling back to node worker"
                            );
                        }
                    }
                    Err(error) => {
                        debug!(
                            provider,
                            operation = operation.unwrap_or(""),
                            error = %summarize_gateway_error(&classify_network_error(&error, Some(provider))),
                            "gemini canvas direct Rust StreamGenerate replay failed; falling back to node worker"
                        );
                    }
                }
            }

            let script_path = gemini_canvas_http_replay_worker_script_path();
            let input = gemini_canvas_web_reverse_modular::build_http_replay_worker_input(
                &effective_template.url,
                &effective_template.query,
                &effective_template.headers,
                &effective_template.raw_post_data,
                &session.cookie_header,
                operation,
                timeout,
            );
            let stdin_json = serde_json::to_vec(&input).map_err(|error| {
                gemini_canvas_http_replay_worker_input_serialize_error(error.to_string().as_str())
            })?;
            maybe_dump_gemini_canvas_http_replay_worker_debug(
                "input",
                operation.unwrap_or("unknown"),
                &stdin_json,
            );

            let mut child = Command::new(
                std::env::var("GEMINI_CANVAS_HTTP_NODE_BIN").unwrap_or_else(|_| "node".to_string()),
            )
            .arg(&script_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                gemini_canvas_http_replay_worker_spawn_failed_error(
                    script_path.as_path(),
                    error.to_string().as_str(),
                )
            })?;

            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(&stdin_json).await.map_err(|error| {
                    gemini_canvas_http_replay_worker_stdin_error(error.to_string().as_str())
                })?;
            }

            let output = tokio::time::timeout(timeout, child.wait_with_output())
                .await
                .map_err(|_| gemini_canvas_http_replay_worker_timeout_error())?
                .map_err(|error| {
                    gemini_canvas_http_replay_worker_wait_failed_error(error.to_string().as_str())
                })?;

            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            maybe_dump_gemini_canvas_http_replay_worker_debug(
                "stdout",
                operation.unwrap_or("unknown"),
                stdout.as_bytes(),
            );
            if !stderr.is_empty() {
                maybe_dump_gemini_canvas_http_replay_worker_debug(
                    "stderr",
                    operation.unwrap_or("unknown"),
                    stderr.as_bytes(),
                );
            }
            let result = gemini_canvas_web_reverse_modular::parse_http_replay_worker_output(
                &stdout, &stderr,
            )?;

            if result.ok {
                let success =
                    gemini_canvas_web_reverse_modular::extract_http_replay_worker_success(result)?;
                if success.status == 400 {
                    if let Some(token) = maybe_retry_gemini_canvas_stream_template_access_token(
                        &mut effective_template,
                        &success.body_text,
                        &mut xsrf_retry_attempted,
                    ) {
                        debug!(
                            provider,
                            operation = operation.unwrap_or(""),
                            xsrf_token_preview = %truncate_response_preview(&token, 24),
                            "retrying gemini canvas HTTP replay worker node StreamGenerate with xsrf token extracted from upstream error"
                        );
                        continue;
                    }
                }
                return Ok(success);
            }

            let status = result
                .error
                .as_ref()
                .and_then(|entry| entry.status)
                .or(result.status)
                .unwrap_or(500);
            let body_text = result
                .error
                .as_ref()
                .and_then(|entry| entry.body_text.as_deref())
                .or_else(|| stderr.is_empty().then_some("").or(Some(stderr.as_str())))
                .unwrap_or_default();
            if status == 400 {
                if let Some(token) = maybe_retry_gemini_canvas_stream_template_access_token(
                    &mut effective_template,
                    &body_text,
                    &mut xsrf_retry_attempted,
                ) {
                    debug!(
                        provider,
                        operation = operation.unwrap_or(""),
                        xsrf_token_preview = %truncate_response_preview(&token, 24),
                        "retrying gemini canvas HTTP replay worker error path with xsrf token extracted from upstream error"
                    );
                    continue;
                }
            }
            return Err(
                gemini_canvas_web_reverse_modular::classify_http_replay_worker_failure(
                    result, &stderr, provider,
                ),
            );
        }
    }

    pub(super) async fn ensure_gemini_canvas_browser_pool(
        &self,
        _provider: &str,
    ) -> Result<String, GatewayError> {
        let base_url = gemini_canvas_browser_pool_base_url();
        if self
            .http
            .request(Method::GET, format!("{}/health", base_url))
            .timeout(Duration::from_secs(2))
            .send()
            .await
            .ok()
            .is_some_and(|response| response.status().is_success())
        {
            return Ok(base_url);
        }

        let _startup_guard = gemini_canvas_browser_pool_start_mutex().lock().await;
        if self
            .http
            .request(Method::GET, format!("{}/health", base_url))
            .timeout(Duration::from_secs(2))
            .send()
            .await
            .ok()
            .is_some_and(|response| response.status().is_success())
        {
            return Ok(base_url);
        }

        let script_path = gemini_canvas_browser_pool_script_path();
        let log_path = gemini_canvas_browser_pool_log_path();
        if let Some(parent) = log_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let stdout = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map(Stdio::from)
            .map_err(|error| {
                gemini_canvas_browser_pool_log_open_error(
                    log_path.as_path(),
                    error.to_string().as_str(),
                )
            })?;
        let stderr = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map(Stdio::from)
            .map_err(|error| {
                gemini_canvas_browser_pool_log_open_error(
                    log_path.as_path(),
                    error.to_string().as_str(),
                )
            })?;
        Command::new(
            std::env::var("GEMINI_CANVAS_BROWSER_NODE_BIN").unwrap_or_else(|_| "node".to_string()),
        )
        .arg(&script_path)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .spawn()
        .map_err(|error| {
            gemini_canvas_browser_pool_spawn_failed_error(
                script_path.as_path(),
                error.to_string().as_str(),
            )
        })?;

        let deadline = std::time::Instant::now() + Duration::from_secs(45);
        loop {
            if std::time::Instant::now() >= deadline {
                return Err(gemini_canvas_browser_pool_start_timeout_error(
                    log_path.as_path(),
                ));
            }

            if self
                .http
                .request(Method::GET, format!("{}/health", base_url))
                .timeout(Duration::from_secs(2))
                .send()
                .await
                .ok()
                .is_some_and(|response| response.status().is_success())
            {
                return Ok(base_url);
            }

            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    }

    pub(super) async fn execute_gemini_canvas_browser_request(
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
        gemini_canvas_web_reverse_modular::execute_browser_request(
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

    pub(super) async fn execute_gemini_canvas_connected_fetch_json_with_mode(
        &self,
        provider: &str,
        browser_pool_base_url: &str,
        base_url: &str,
        share_id: &str,
        runtime_state_object_key: &str,
        browser_cdp_url: Option<&str>,
        cookie_header: Option<&str>,
        request_url: &str,
        request_body: &Value,
        google_fetch_mode: &str,
        timeout: Duration,
    ) -> Result<Value, GatewayError> {
        gemini_canvas_web_reverse_modular::execute_connected_fetch_json_with_mode(
            &self.http,
            timeout,
            provider,
            browser_pool_base_url,
            base_url,
            share_id,
            runtime_state_object_key,
            browser_cdp_url,
            cookie_header,
            request_url,
            request_body,
            google_fetch_mode,
        )
        .await
    }

    // Direct HTTP helpers for the active `gemini_canvas_compatible` hot path.
    // Legacy browser-connected and browser-pool flows remain below as explicit
    // compatibility fallbacks when pure HTTP replay is disabled.
    pub(super) async fn execute_gemini_canvas_direct_http_json_with_options(
        &self,
        payload: &ProviderAccountPayload,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        request_url: &str,
        request_body: &Value,
        timeout: Duration,
        api_key_override: Option<&str>,
        api_key_transport: GeminiCanvasDirectHttpApiKeyTransport,
        signed_origin_override: Option<&str>,
        referer_override: Option<&str>,
        preserve_cross_origin_origin: bool,
        preserve_cross_origin_referer: bool,
        include_signed_headers: bool,
    ) -> Result<Value, GatewayError> {
        send_gemini_canvas_direct_http_json_with_options(
            &self.http,
            payload,
            runtime,
            request_url,
            request_body,
            timeout,
            api_key_override,
            api_key_transport,
            signed_origin_override,
            referer_override,
            preserve_cross_origin_origin,
            preserve_cross_origin_referer,
            include_signed_headers,
        )
        .await
    }

    pub(super) async fn upload_gemini_canvas_image_edit_inputs(
        &self,
        payload: &ProviderAccountPayload,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        uploads: &[gemini_canvas::GeminiCanvasImageEditUpload],
        timeout: Duration,
    ) -> Result<Vec<gemini_canvas::GeminiCanvasUploadedFileRef>, GatewayError> {
        upload_gemini_canvas_image_edit_inputs_with_http(
            &self.http, payload, session, bootstrap, uploads, timeout,
        )
        .await
    }

    pub(super) async fn send_gemini_canvas_signaler_poll_request_refreshing_session(
        &self,
        payload: &ProviderAccountPayload,
        session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
        url: &str,
        timeout: Duration,
        aid_hint: u64,
        locale_override: Option<&str>,
    ) -> Result<String, GatewayError> {
        send_gemini_canvas_signaler_poll_request_refreshing_session_with_http(
            &self.http,
            payload,
            session,
            url,
            timeout,
            aid_hint,
            locale_override,
        )
        .await
    }

    pub(super) async fn prewarm_gemini_canvas_image_edit_signaler(
        &self,
        payload: &ProviderAccountPayload,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
        locale_override: Option<&str>,
        timeout: Duration,
    ) -> Result<GeminiCanvasSignalerChannel, GatewayError> {
        prewarm_gemini_canvas_image_edit_signaler_with_http(
            &self.http,
            &self.plain_http,
            payload,
            runtime,
            session,
            locale_override,
            timeout,
        )
        .await
    }
}
