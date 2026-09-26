use super::*;

impl UpstreamClient {
    pub fn client(&self) -> &Client {
        &self.http
    }

    pub fn browser_executor_runtime_health(&self) -> BrowserExecutorServiceHealth {
        build_browser_executor_runtime_health(self.browser_executor_base_url.clone())
    }

    // ── request plan ─────────────────────────────────────────────────────

    /// Determine the upstream URL and packed body for the given request.
    ///
    /// The endpoint path, method, and body format depend on the adapter type:
    /// - `openai_compatible` → `/v1/chat/completions`, packed with OpenAI format
    /// - `anthropic_compatible` → `/v1/messages`, packed with Anthropic format
    /// - `grok_compatible` → `/rest/app-chat/conversations/new`, packed with Grok format
    /// - `search_api_compatible` → JSON passthrough search-provider endpoints
    /// - `lumalabs_compatible` → custom image/video/audio passthrough via board action + SSE events
    /// - `gemini_canvas_compatible` → Gemini Canvas reverse-web generation via direct HTTP replay with optional legacy browser fallback
    /// - `producer_compatible` → Producer.ai reverse-web image / music / music-video generation via internal web routes
    /// - `suno_compatible` → Suno reverse-web image / audio / video generation via challenge check + v2-web submit + feed/v3 polling
    /// - `udio_compatible` → Udio image / music / video generation via browser worker + page-context generate/poll
    /// - `custom_http` → `base_url` as-is (no path appended), raw body forwarded
    pub fn build_request_plan(
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        stream: bool,
    ) -> Result<RequestPlan, GatewayError> {
        let mut plan = match payload.canonical_adapter() {
            "anthropic_compatible" => {
                if qwen_official_api_modular::owns_payload(payload) {
                    qwen_official_api_modular::build_request_plan(payload, req, model, stream)
                } else if anthropic_messages_upstream::owns_payload(payload) {
                    anthropic_messages_upstream::build_request_plan(payload, req, model, stream)
                } else {
                    anthropic_messages_upstream::build_request_plan(payload, req, model, stream)
                }
            }

            "xfyun_websocket_compatible" => {
                xfyun_websocket::build_request_plan(payload, req, model)
            }

            "openai_compatible" => {
                if chatgpt_official_api_modular::owns_payload(payload) {
                    chatgpt_official_api_modular::build_request_plan(payload, req, model, stream)
                } else if qwen_official_api_modular::owns_payload(payload) {
                    qwen_official_api_modular::build_request_plan(payload, req, model, stream)
                } else if azure_openai_upstream::owns_payload(payload) {
                    azure_openai_upstream::build_request_plan(payload, req, model, stream)
                } else {
                    openai_compatible_request_plan_modular::build_request_plan(
                        payload, req, model, stream,
                    )
                }
            }

            "gemini_api_compatible" | "gemini_api_modular_compatible" => {
                gemini_api_modular::build_request_plan(payload, req, model, stream)
            }

            "bedrock_converse_compatible" => {
                bedrock_converse_common::build_request_plan(payload, req, model, stream)
            }

            "cohere_compatible" => {
                cohere_chat_common::build_request_plan(payload, req, model, stream)
            }

            "accio_compatible" => accio_upstream::build_request_plan(payload, req, model, stream),

            "grok_compatible" => grok_upstream::build_request_plan(payload, req, model),

            "kiro_compatible" => kiro_upstream::build_request_plan(payload, req, model),

            "freebuff_compatible" => freebuff::build_request_plan(payload, req, model, stream),

            "qwen_web_compatible" => {
                Err(qwen_web_reverse_modular::unsupported_request_plan_error())
            }

            "chatgpt_web_reverse_compatible" => {
                Err(chatgpt_web_reverse_modular::unsupported_request_plan_error())
            }

            adapter if aistudio_web_reverse_modular::is_aistudio_web_reverse_adapter(adapter) => {
                Err(aistudio_web_reverse_modular::unsupported_request_plan_error())
            }

            "gemini_web_compatible" => Err(gemini_web::unsupported_request_plan_error()),

            "gemini_web_reverse_modular_compatible" => {
                Err(gemini_web_reverse_modular::unsupported_request_plan_error())
            }

            "gemini_business_compatible" => Err(gemini_business::unsupported_request_plan_error()),

            "chataibot_compatible" => Err(chataibot_upstream::unsupported_request_plan_error()),

            "lumalabs_compatible" => Err(lumalabs::unsupported_request_plan_error()),

            "gemini_canvas_compatible" => Err(gemini_canvas::unsupported_request_plan_error()),

            "gemini_canvas_web_reverse_compatible" => {
                Err(gemini_canvas_web_reverse_modular::unsupported_request_plan_error())
            }

            "gemini_canvas_program_web_reverse_compatible" => {
                Err(gemini_canvas_program_web_reverse_modular::unsupported_request_plan_error())
            }

            "producer_compatible" => Err(producer::unsupported_request_plan_error()),

            "suno_compatible" => Err(crate::upstream::suno::request_plan_unsupported_error()),

            "udio_compatible" => Err(udio::unsupported_request_plan_error()),

            "search_api_compatible" => {
                search_provider_helpers::build_search_provider_request_plan(payload, req)
            }

            _ => Ok(custom_http_request_plan::build_request_plan(payload, req)),
        }?;

        // Merge data-driven provider constraints (e.g. Codex "store": false).
        // Only injects keys NOT already present in the body.
        if let Some(extra) = &payload.extra_body {
            if let Some(Value::Object(map)) = plan.body.as_mut() {
                for (key, value) in extra {
                    if payload.canonical_adapter() == "kiro_compatible"
                        && kiro::is_reserved_payload_extra_key(key)
                    {
                        continue;
                    }
                    if payload.canonical_adapter() == "freebuff_compatible"
                        && freebuff::is_reserved_payload_extra_key(key)
                    {
                        continue;
                    }
                    map.entry(key.clone()).or_insert_with(|| value.clone());
                }
            }
        }

        tracing::debug!(url = %plan.url, "request plan built");
        Ok(plan)
    }

    // ── non-streaming execute ─────────────────────────────────────────────

    /// Execute a non-streaming upstream request and return a canonical response.
    ///
    /// `extra_headers` are merged into the upstream request headers (lower
    /// priority than provider config).  Pass `None` when no additional headers
    /// are needed.
    ///
    /// **Accio special case**: Accio (phoenix-gw) always returns SSE — even
    /// for "non-streaming" requests (it returns 406 for `Accept: application/json`).
    /// So for `accio_compatible` adapters, we force `stream=true` in the
    /// request plan, then read and accumulate the entire SSE stream into a
    /// single canonical response.
    ///
    /// **Grok special case**: Grok always returns NDJSON — even for
    /// "non-streaming" requests. So for `grok_compatible` adapters, we read and
    /// accumulate the entire NDJSON stream into a single canonical response.
    pub async fn execute(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<CanonicalRelayResponse, GatewayError> {
        self.execute_with_provider_account_id("", payload, req, model, extra_headers)
            .await
    }
}
