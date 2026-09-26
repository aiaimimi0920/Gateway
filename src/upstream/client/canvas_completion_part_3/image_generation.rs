use super::*;

impl UpstreamClient {
    pub(super) async fn execute_gemini_canvas_image_generation(
        &self,
        provider_account_id: &str,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        browser_runtime_state_object_key_for: &(dyn Fn(&str) -> String + Sync),
        browser_cdp_url: &Option<String>,
        browser_cookie_header: &Option<String>,
        locale: &str,
        base_url: &str,
    ) -> Result<Value, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let _ = gemini_canvas::resolve_image_model(model)?;
        if gemini_canvas::requested_output_count(req) > 1 {
            return Err(gemini_canvas::unsupported_image_count_error(provider));
        }

        let prompt = gemini_web_reverse_modular::prompt_for_legacy_mixed_lane_media_request(
            req,
            gemini_canvas::GeminiCanvasMediaOperation::Image,
        )?;
        if gemini_canvas::pure_http_enabled(payload) {
            match self
                .execute_gemini_canvas_media_direct_http(
                    payload,
                    req,
                    model,
                    gemini_canvas::GeminiCanvasMediaOperation::Image,
                    prompt.clone(),
                    self.timeout.max(Duration::from_secs(240)),
                )
                .await
            {
                Ok(body) => return Ok(body),
                Err(error) if payload.adapter != "gemini_web_reverse_modular_compatible" => {
                    return Err(error);
                }
                Err(error) => {
                    debug!(
                        provider,
                        adapter = %payload.adapter,
                        error = %summarize_gateway_error(&error),
                        "gemini web reverse modular legacy image direct HTTP lane failed; falling back to browser-owned image invocation"
                    );
                }
            }
        }
        // Resolve browser state only after pure HTTP declines the request.
        let browser_runtime_state_object_key = browser_runtime_state_object_key_for("image");
        let request_timeout = self.timeout.max(Duration::from_secs(240));
        let invocation_input =
            gemini_canvas_web_reverse_modular::build_browser_operation_invocation_input_from_values(
                base_url,
                &runtime.share_id,
                &browser_runtime_state_object_key,
                browser_cdp_url.as_deref(),
                browser_cookie_header.as_deref(),
                "image",
                &prompt,
                locale,
                request_timeout,
            );
        let result = if let Some(result) = self
            .execute_remote_browser_executor(
                "gemini_canvas",
                provider_account_id,
                req.endpoint_kind,
                invocation_input,
            )
            .await?
        {
            gemini_canvas_web_reverse_modular::parse_remote_media_browser_invocation_value(
                provider, result, "image",
            )?
        } else {
            let browser_pool_base_url = self.ensure_gemini_canvas_browser_pool(provider).await?;
            self.execute_gemini_canvas_browser_request_with_recovery(
                provider,
                &browser_pool_base_url,
                base_url,
                &runtime.share_id,
                &browser_runtime_state_object_key,
                browser_cdp_url.as_deref(),
                browser_cookie_header.as_deref(),
                "image",
                &prompt,
                locale,
                request_timeout,
            )
            .await?
        };

        let image_assets = gemini_canvas_web_reverse_modular::collect_image_media_assets(&result)
            .into_iter()
            .map(convert_gemini_canvas_asset)
            .collect::<Vec<_>>();
        if image_assets.is_empty() {
            return Err(
                gemini_canvas_web_reverse_modular::browser_pool_missing_image_asset_error(provider),
            );
        }

        let selected_image_assets = image_assets
            .iter()
            .take(gemini_canvas::requested_output_count(req))
            .collect::<Vec<_>>();
        if gemini_canvas::prefers_url_response(req)?
            && selected_image_assets
                .iter()
                .all(|asset| gemini_canvas_asset_url_is_caller_usable(&asset.url))
        {
            return gemini_canvas::build_openai_images_response_from_urls(
                req,
                &prompt,
                &image_assets,
            );
        }

        let mut downloaded = Vec::with_capacity(image_assets.len());
        for asset in selected_image_assets {
            if let Some(image) =
                gemini_canvas_web_reverse_modular::decode_browser_pool_inline_image_asset(
                    provider, asset,
                )?
            {
                downloaded.push(image);
                continue;
            }

            if asset.url.starts_with("http://")
                || asset.url.starts_with("https://")
                || asset.url.starts_with("//")
                || asset.url.starts_with('/')
            {
                let image = match self
                    .fetch_gemini_canvas_direct_http_image_asset(
                        payload,
                        runtime,
                        asset,
                        request_timeout,
                    )
                    .await
                {
                    Ok(image) => image,
                    Err(download_error) => {
                        let browser_pool_base_url =
                            self.ensure_gemini_canvas_browser_pool(provider).await?;
                        let (bytes, response_content_type) =
                            gemini_canvas_web_reverse_modular::execute_connected_fetch_get_bytes(
                                &self.http,
                                request_timeout,
                                provider,
                                &browser_pool_base_url,
                                base_url,
                                &runtime.share_id,
                                &browser_runtime_state_object_key,
                                browser_cdp_url.as_deref(),
                                browser_cookie_header.as_deref(),
                                &asset.url,
                            )
                            .await
                            .map_err(|browser_fetch_error| {
                                let mut browser_fetch_error = browser_fetch_error;
                                browser_fetch_error.message =
                                    sanitize_provider_error_message(&format!(
                                        "{}; direct_http_download={}",
                                        browser_fetch_error.message,
                                        summarize_gateway_error(&download_error)
                                    ));
                                browser_fetch_error
                            })?;
                        gemini_canvas_web_reverse_modular::build_downloaded_image_from_bytes(
                            asset,
                            response_content_type.as_deref(),
                            &bytes,
                        )
                    }
                };
                downloaded.push(image);
                continue;
            }

            let response = self
                .http
                .request(Method::GET, &asset.url)
                .timeout(request_timeout)
                .redirect(rquest::redirect::Policy::limited(10))
                .send()
                .await
                .map_err(|e| classify_network_error(&e, Some(provider)))?;

            let status = response.status().as_u16();
            if !response.status().is_success() {
                let body_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| String::from("<unreadable body>"));
                return Err(classify_upstream_error(status, &body_text, Some(provider)));
            }

            let mime_type = response
                .headers()
                .get(rquest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
                .filter(|value| value.starts_with("image/"))
                .map(ToString::to_string)
                .unwrap_or_else(|| asset.mime_type.clone());
            let bytes = response
                .bytes()
                .await
                .map_err(|e| classify_network_error(&e, Some(provider)))?;
            downloaded.push(gemini_canvas::GeminiCanvasImage {
                mime_type,
                bytes: bytes.to_vec(),
            });
        }

        gemini_canvas::build_openai_images_response_from_bytes(req, &prompt, &downloaded)
    }
}
