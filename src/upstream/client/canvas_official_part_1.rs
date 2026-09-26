use super::*;

impl UpstreamClient {
    pub(super) async fn prepare_gemini_canvas_runtime_api_payload(
        &self,
        payload: &ProviderAccountPayload,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        timeout: Duration,
    ) -> Result<GeminiCanvasRuntimeApiContext, GatewayError> {
        let base_url = payload.base_url.trim_end_matches('/');
        let harvest_target_url = gemini_canvas::direct_http_referrer(base_url, &runtime.share_id);
        self.prepare_gemini_canvas_runtime_api_payload_for_target(
            payload,
            runtime,
            &harvest_target_url,
            timeout,
        )
        .await
    }

    pub(super) async fn prepare_gemini_canvas_runtime_api_payload_for_target(
        &self,
        payload: &ProviderAccountPayload,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        harvest_target_url: &str,
        timeout: Duration,
    ) -> Result<GeminiCanvasRuntimeApiContext, GatewayError> {
        let storage_state = gateway_object_storage()?
            .read_json(&runtime.runtime_state_object_key)
            .await?;
        let auth_user = gemini_canvas::direct_http_auth_user(payload);
        let base_url = payload.base_url.trim_end_matches('/');
        let explicit_cookie_header = payload
            .extra_body
            .as_ref()
            .and_then(|extra| {
                extra
                    .get("canvasProgramInvokeContract")
                    .and_then(Value::as_object)
                    .and_then(|contract| contract.get("cookieHeader"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .or_else(|| {
                payload
                    .extra_body
                    .as_ref()
                    .and_then(|extra| extra.get("cookieHeader"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .or_else(|| {
                payload
                    .extra_body
                    .as_ref()
                    .and_then(|extra| {
                        extra
                            .get("canvasProgramInvokeContract")
                            .and_then(Value::as_object)
                            .and_then(|contract| contract.get("cookie_header"))
                            .and_then(Value::as_str)
                    })
                    .map(str::to_string)
            })
            .or_else(|| {
                payload
                    .extra_body
                    .as_ref()
                    .and_then(|extra| extra.get("cookie_header"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .or_else(|| read_json_string(&storage_state, "cookieHeader"));
        let session = if let Some(cookie_header) = explicit_cookie_header
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            gemini_canvas::pure_http_session_from_cookie_header(cookie_header, &auth_user)?
        } else {
            gemini_canvas::storage_state_to_pure_http_session(
                &storage_state,
                harvest_target_url,
                base_url,
                &auth_user,
            )?
        };
        let page_origin = origin_from_url(harvest_target_url)
            .unwrap_or_else(|| gemini_canvas_http_origin(payload));
        let page_referer = if harvest_target_url.trim().is_empty() {
            format!("{}/", page_origin.trim_end_matches('/'))
        } else {
            harvest_target_url.to_string()
        };
        if payload.adapter == "gemini_canvas_program_web_reverse_compatible" {
            ensure_gemini_canvas_program_payload_avoids_official_api_identity(payload)?;
            let discovered_identity =
                gemini_canvas::direct_http_google_api_keys(payload, &storage_state);
            if !discovered_identity.is_empty() {
                return Err(
                    gemini_canvas_program_runtime_material_official_api_key_forbidden_error(),
                );
            }
            let mut runtime_api_payload = payload.clone();
            runtime_api_payload.base_url = runtime.api_base_url.trim_end_matches('/').to_string();
            runtime_api_payload.api_key.clear();
            return Ok(GeminiCanvasRuntimeApiContext {
                payload: runtime_api_payload,
                api_key_candidates: Vec::new(),
                session,
                page_origin: page_origin.clone(),
                page_referer: page_referer.clone(),
            });
        }
        let mut page_harvest_probes = Vec::new();
        let mut google_api_key_candidates =
            gemini_canvas::direct_http_google_api_keys(payload, &storage_state);
        if google_api_key_candidates.is_empty() {
            let (harvested, probes) = self
                .harvest_gemini_canvas_direct_http_api_keys(payload, runtime, &session, timeout)
                .await;
            page_harvest_probes = probes;
            for candidate in harvested {
                if !google_api_key_candidates
                    .iter()
                    .any(|existing| existing == &candidate)
                {
                    google_api_key_candidates.push(candidate);
                }
            }
        }
        let google_api_key = google_api_key_candidates.first().cloned().ok_or_else(|| {
            gemini_canvas_runtime_api_missing_google_api_key_error(&page_harvest_probes)
        })?;
        let mut runtime_api_payload = payload.clone();
        runtime_api_payload.base_url = runtime.api_base_url.trim_end_matches('/').to_string();
        runtime_api_payload.api_key = google_api_key;
        Ok(GeminiCanvasRuntimeApiContext {
            payload: runtime_api_payload,
            api_key_candidates: google_api_key_candidates,
            session,
            page_origin,
            page_referer,
        })
    }

    pub(super) async fn prepare_gemini_canvas_program_app_endpoint_api_context(
        &self,
        payload: &ProviderAccountPayload,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        timeout: Duration,
    ) -> Result<GeminiCanvasProgramAppEndpointApiContext, GatewayError> {
        let relay_config =
            gemini_canvas_program_web_reverse_modular::relay_config_from_payload(payload)?;
        if !relay_config.has_concrete_handle() {
            return Err(
                gemini_canvas_program_web_reverse_modular::missing_gemini_canvas_program_app_endpoint_handle_error(
                    "gemini_canvas_program_web_reverse_compatible",
                ),
            );
        }
        let base_url = payload.base_url.trim_end_matches('/');
        let harvest_target_url =
            gemini_canvas_program_web_reverse_modular::preferred_app_endpoint_harvest_target_url(
                base_url,
                &relay_config,
            );
        let runtime_api = self
            .prepare_gemini_canvas_runtime_api_payload_for_target(
                payload,
                runtime,
                &harvest_target_url,
                timeout,
            )
            .await?;
        let page_url = gemini_canvas_program_web_reverse_modular::preferred_app_endpoint_page_url(
            base_url,
            &relay_config,
        )
        .unwrap_or_else(|| harvest_target_url.clone());
        let official_extra_headers =
            gemini_canvas_program_web_reverse_modular::build_program_app_endpoint_official_extra_headers(
                &gemini_canvas::locale_from_payload(payload),
                &runtime_api.session.auth_user,
                &page_url,
            );
        let invoke_base_url =
            gemini_canvas_program_web_reverse_modular::preferred_app_endpoint_invoke_base_url(
                &runtime.api_base_url,
                &relay_config,
            );
        Ok(GeminiCanvasProgramAppEndpointApiContext {
            relay_config,
            runtime_api,
            official_extra_headers,
            invoke_base_url,
        })
    }

    pub(super) async fn execute_gemini_canvas_official_media(
        &self,
        _provider_account_id: &str,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        gemini_api_modular::execute_official_media(
            &self.http,
            self.timeout,
            payload,
            req,
            model,
            extra_headers,
        )
        .await
    }

    pub(super) async fn execute_producer_media(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        match req.endpoint_kind {
            EndpointKind::ImagesGenerations => {
                return self
                    .execute_producer_image(payload, req, model, extra_headers)
                    .await;
            }
            EndpointKind::MusicGenerations => {
                return self
                    .execute_producer_music(payload, req, model, extra_headers)
                    .await;
            }
            EndpointKind::VideosGenerations => {
                return self
                    .execute_producer_video(payload, req, model, extra_headers)
                    .await;
            }
            _ => {
                return Err(producer::unsupported_media_endpoint_error());
            }
        }
    }

    pub(super) async fn execute_producer_image(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        let headers = maybe_refresh_producer_headers(
            &self.http,
            &build_upstream_headers_with(payload, extra_headers),
        )
        .await;
        let base_url = payload.base_url.trim_end_matches('/');
        let request_timeout = self.timeout.max(Duration::from_secs(180));
        execute_producer_image_http(&self.http, base_url, &headers, req, model, request_timeout)
            .await
    }

    pub(super) async fn execute_producer_browser_backed(
        &self,
        provider_account_id: &str,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        let provider = "producer_compatible";
        let headers = maybe_refresh_producer_headers(
            &self.http,
            &build_upstream_headers_with(payload, extra_headers),
        )
        .await;
        let request_timeout = self.timeout.max(Duration::from_secs(900));
        let prepared = prepare_producer_browser_execution_input(
            &payload.base_url,
            &headers,
            &req.raw_body,
            model,
            request_timeout,
        );
        if let Some(result) = self
            .execute_remote_browser_executor(
                "producer",
                provider_account_id,
                req.endpoint_kind,
                build_producer_browser_executor_payload_from_prepared(
                    &prepared,
                    std::env::var("PRODUCER_BROWSER_EXECUTABLE_PATH").ok(),
                ),
            )
            .await?
        {
            return Ok(result);
        }

        execute_producer_browser_worker(provider, &prepared, false).await
    }

    pub(super) async fn execute_producer_music(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        let headers = maybe_refresh_producer_headers(
            &self.http,
            &build_upstream_headers_with(payload, extra_headers),
        )
        .await;
        let base_url = payload.base_url.trim_end_matches('/');
        let request_timeout = self.timeout.max(Duration::from_secs(180));
        execute_producer_music_http(&self.http, base_url, &headers, req, model, request_timeout)
            .await
    }

    pub(super) async fn execute_producer_video(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        let provider = "producer_compatible";
        let headers = maybe_refresh_producer_headers(
            &self.http,
            &build_upstream_headers_with(payload, extra_headers),
        )
        .await;
        let request_timeout = self.timeout.max(Duration::from_secs(900));
        let prepared = prepare_producer_browser_execution_input(
            &payload.base_url,
            &headers,
            &req.raw_body,
            model,
            request_timeout,
        );
        match execute_producer_video_http(
            &self.http,
            &prepared.base_url,
            &prepared.headers,
            &prepared.request_body,
            &prepared.model,
            prepared.timeout,
        )
        .await
        {
            Ok(result) => Ok(result),
            Err(error) if should_fallback_producer_video_http_error(&error) => {
                debug!(
                    provider,
                    message = %error.message,
                    code = ?error.code,
                    status = ?error.http_status,
                    "producer direct-http video orchestration failed; falling back to browser worker"
                );
                execute_producer_browser_worker(provider, &prepared, true).await
            }
            Err(error) => Err(error),
        }
    }
}
