use super::*;

impl UpstreamClient {
    pub(crate) async fn execute_remote_browser_executor(
        &self,
        provider: &str,
        provider_account_id: &str,
        endpoint_kind: EndpointKind,
        input: Value,
    ) -> Result<Option<Value>, GatewayError> {
        let Some(base_url) = self.browser_executor_base_url.as_deref() else {
            if self.request_time_browser_policy == RequestTimeBrowserPolicy::RemoteOnly {
                return Err(remote_browser_executor_required_unavailable_error(
                    "Remote browser executor is required by GATEWAY_REQUEST_TIME_BROWSER_POLICY=remote_only, but GATEWAY_BROWSER_EXECUTOR_BASE_URL is not configured.",
                ));
            }
            if self.request_time_browser_policy == RequestTimeBrowserPolicy::Disabled {
                return Err(request_time_browser_forbidden_error(
                    "Request-time local browser execution is disabled by GATEWAY_REQUEST_TIME_BROWSER_POLICY=disabled, and GATEWAY_BROWSER_EXECUTOR_BASE_URL is not configured.",
                ));
            }
            return Ok(None);
        };

        let endpoint_kind_key = browser_executor_endpoint_kind_key(endpoint_kind);
        let remote_executor_timeout = browser_executor_remote_request_timeout(self.timeout, &input);
        let request = BrowserExecutorInvocationRequest {
            provider,
            provider_account_id,
            endpoint_kind: endpoint_kind_key,
            execution_mode: "browser_backed",
            input,
        };
        let url = format!("{base_url}/v1/internal/browser-executor/execute");
        let mut builder = self
            .http
            .request(Method::POST, &url)
            .header(rquest::header::CONTENT_TYPE, "application/json")
            .timeout(remote_executor_timeout)
            .json(&request);
        if let Some(token) = self.browser_executor_bearer_token.as_deref() {
            builder = builder.bearer_auth(token);
        }

        let response = match builder.send().await {
            Ok(response) => response,
            Err(error) => {
                debug!(
                    provider,
                    provider_account_id,
                    endpoint_kind = endpoint_kind_key,
                    timed_out = error.is_timeout(),
                    connect_failed = error.is_connect(),
                    "remote browser executor unavailable; falling back to local worker"
                );
                if self.request_time_browser_policy.forbids_local_fallback() {
                    return Err(remote_browser_executor_required_unavailable_error(
                        "Remote browser executor is unavailable while request-time local browser fallback is disabled.",
                    ));
                }
                return Ok(None);
            }
        };

        decode_remote_browser_executor_response(
            response,
            self.request_time_browser_policy,
            provider,
            provider_account_id,
            endpoint_kind_key,
        )
        .await
    }

    pub async fn execute_browser_executor_service_invocation(
        &self,
        request: BrowserExecutorServiceInvocationRequest,
    ) -> BrowserExecutorServiceInvocationResponse {
        let provider_name = request.provider.trim().to_lowercase();
        match self
            .execute_browser_executor_service_invocation_inner(request)
            .await
        {
            Ok(result) => {
                build_browser_executor_service_invocation_success_response(&provider_name, result)
            }
            Err(error) => {
                build_browser_executor_service_invocation_failure_response(&provider_name, error)
            }
        }
    }

    pub(super) async fn execute_browser_executor_service_invocation_inner(
        &self,
        request: BrowserExecutorServiceInvocationRequest,
    ) -> Result<Value, GatewayError> {
        let provider_label = request.provider.clone();
        let provider = provider_label.trim().to_lowercase();
        let input = request.input;

        match provider.as_str() {
            "lumalabs" => {
                self.execute_lumalabs_browser_executor_service_invocation(&input)
                    .await
            }
            "producer" => {
                let prepared = prepare_producer_browser_executor_service_input(&input)?;
                execute_producer_browser_worker("producer_compatible", &prepared, false).await
            }
            "suno" => {
                self.execute_suno_browser_executor_service_invocation(&input)
                    .await
            }
            "udio" => {
                self.execute_udio_browser_executor_service_invocation(&input)
                    .await
            }
            "gemini_canvas" => {
                let prepared = gemini_canvas_web_reverse_modular::
                    prepare_gemini_canvas_browser_executor_service_input(&input)?;
                let browser_pool_base_url = self
                    .ensure_gemini_canvas_browser_pool("gemini_canvas_compatible")
                    .await?;
                let result = self
                    .execute_gemini_canvas_browser_request(
                        "gemini_canvas_compatible",
                        &browser_pool_base_url,
                        &prepared.base_url,
                        &prepared.share_id,
                        &prepared.runtime_state_object_key,
                        prepared.browser_cdp_url.as_deref(),
                        prepared.cookie_header.as_deref(),
                        &prepared.operation,
                        &prepared.prompt,
                        &prepared.locale,
                        prepared.timeout,
                    )
                    .await?;
                Ok(
                    gemini_canvas_web_reverse_modular::
                        build_gemini_canvas_browser_executor_service_result(&result),
                )
            }
            _ => Err(unsupported_browser_executor_provider_error(&provider_label)),
        }
    }
}
