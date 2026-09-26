use super::*;

impl UpstreamClient {
    pub(super) async fn try_resolve_gemini_canvas_program_preview_no_key_music_followup(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        seed_body: &str,
        request_started_at: SystemTime,
        timeout: Duration,
        fallback_conversation_id: Option<&str>,
        fallback_response_id: Option<&str>,
        fallback_app_path: Option<&str>,
    ) -> Result<Option<Value>, GatewayError> {
        if !gemini_canvas_music_body_indicates_accepted_progress(seed_body) {
            return Ok(None);
        }
        let provider = "gemini_canvas_compatible";
        let mut page_payload = payload.clone();
        page_payload.base_url = gemini_canvas_page_base_url(payload);
        let followup_body = match self
            .execute_gemini_canvas_direct_http_media_followup_body(
                &page_payload,
                model,
                runtime,
                gemini_canvas::GeminiCanvasMediaOperation::Music,
                prompt,
                seed_body,
                None,
                request_started_at,
                timeout,
                false,
                None,
            )
            .await
        {
            Ok(body) => body,
            Err(error) => {
                debug!(
                    provider,
                    error = %summarize_gateway_error(&error),
                    "gemini canvas program-owned music accepted progress follow-up did not recover a final asset; preserving pending semantics"
                );
                return Ok(Some(build_gemini_canvas_music_accepted_response_from_body(
                    model,
                    prompt,
                    None,
                    seed_body,
                    fallback_conversation_id,
                    fallback_response_id,
                    fallback_app_path,
                )));
            }
        };
        let extracted_assets = gemini_canvas::extract_stream_generate_media_assets(
            &followup_body,
            gemini_canvas::GeminiCanvasMediaOperation::Music,
        )
        .or_else(|_| {
            gemini_canvas::extract_page_blob_media_assets(
                &followup_body,
                gemini_canvas::GeminiCanvasMediaOperation::Music,
            )
        });
        if let Ok(assets) = extracted_assets {
            if let Some(asset) = select_preferred_gemini_canvas_music_asset(&assets) {
                let asset = self
                    .best_effort_materialize_gemini_canvas_direct_http_media_asset(
                        payload, runtime, asset, timeout, provider,
                    )
                    .await;
                return Ok(Some(gemini_canvas::build_music_generation_response(
                    model,
                    prompt,
                    &asset,
                    Some(&followup_body),
                )));
            }
        }
        Ok(Some(build_gemini_canvas_music_accepted_response_from_body(
            model,
            prompt,
            None,
            &followup_body,
            fallback_conversation_id,
            fallback_response_id,
            fallback_app_path,
        )))
    }

    pub(super) async fn best_effort_materialize_gemini_canvas_direct_http_media_asset(
        &self,
        payload: &ProviderAccountPayload,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        asset: &gemini_canvas::GeminiCanvasMediaAsset,
        timeout: Duration,
        provider: &str,
    ) -> gemini_canvas::GeminiCanvasMediaAsset {
        if asset.body_base64.is_some() {
            return asset.clone();
        }
        match self
            .materialize_gemini_canvas_direct_http_media_asset(
                payload,
                runtime,
                &asset.url,
                Some(asset.kind.as_str()),
                Some(asset.mime_type.as_str()),
                timeout,
            )
            .await
        {
            Ok(materialized) => materialized,
            Err(error) => {
                if payload.adapter == "gemini_canvas_program_web_reverse_compatible" {
                    debug!(
                        provider,
                        error = %summarize_gateway_error(&error),
                        "gemini canvas program-owned no-key direct asset materialization failed; browser-assisted fallback is disabled on this line"
                    );
                    return asset.clone();
                }
                debug!(
                    provider,
                    error = %summarize_gateway_error(&error),
                    "gemini canvas media asset materialization failed; returning URL-only asset"
                );
                asset.clone()
            }
        }
    }

    pub(super) async fn materialize_gemini_canvas_direct_http_media_asset(
        &self,
        payload: &ProviderAccountPayload,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        asset_url: &str,
        asset_kind_hint: Option<&str>,
        asset_mime_hint: Option<&str>,
        timeout: Duration,
    ) -> Result<gemini_canvas::GeminiCanvasMediaAsset, GatewayError> {
        let provider = "gemini_canvas_compatible";
        let base_url = payload.base_url.trim_end_matches('/');
        let page_origin = gemini_canvas_http_origin(payload);
        let page_referer = format!("{}/", page_origin.trim_end_matches('/'));
        let storage_state = gateway_object_storage()?
            .read_json(&runtime.runtime_state_object_key)
            .await?;
        let auth_user = gemini_canvas::direct_http_auth_user(payload);
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
            .or_else(|| read_json_string(&storage_state, "cookieHeader"));
        let mut current_url =
            normalize_gemini_canvas_direct_http_asset_url(Some(page_referer.as_str()), asset_url)
                .ok_or_else(|| gemini_canvas_media_fetch_bad_asset_url_error(asset_url))?;

        for _hop in 0..4 {
            let session = if let Some(cookie_header) = explicit_cookie_header
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                gemini_canvas::pure_http_session_from_cookie_header(cookie_header, &auth_user)?
            } else {
                gemini_canvas::storage_state_to_pure_http_session(
                    &storage_state,
                    &current_url,
                    base_url,
                    &auth_user,
                )?
            };
            let mut headers = build_gemini_canvas_direct_http_media_fetch_headers(
                payload,
                &current_url,
                &page_origin,
                &page_referer,
                asset_kind_hint,
            );
            if should_forward_gemini_canvas_download_cookies(&current_url) {
                apply_gemini_canvas_cookie_header(&mut headers, &session);
            }

            let response = self
                .http
                .request(Method::GET, &current_url)
                .headers(headers)
                .timeout(timeout.max(Duration::from_secs(120)))
                .redirect(rquest::redirect::Policy::none())
                .send()
                .await
                .map_err(|error| classify_network_error(&error, Some(provider)))?;
            let status = response.status().as_u16();
            let content_type = response
                .headers()
                .get(rquest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
                .map(str::to_string);
            let redirect_target = response
                .headers()
                .get(rquest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .and_then(|location| resolve_relative_url(&current_url, location));

            if response.status().is_redirection() {
                let next_url = redirect_target
                    .ok_or_else(gemini_canvas_media_fetch_redirect_missing_location_error)?;
                if next_url.contains("accounts.google.com/ServiceLogin")
                    || next_url.contains("accounts.google.com/CookieMismatch")
                {
                    return Err(gemini_canvas_media_fetch_cookie_mismatch_redirect_error());
                }
                current_url =
                    normalize_gemini_canvas_direct_http_asset_url(Some(&current_url), &next_url)
                        .ok_or_else(|| gemini_canvas_media_fetch_bad_redirect_error(&next_url))?;
                continue;
            }

            if !response.status().is_success() {
                let body_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| String::from("<unreadable body>"));
                return Err(classify_upstream_error(status, &body_text, Some(provider)));
            }

            let bytes = response
                .bytes()
                .await
                .map_err(|error| classify_network_error(&error, Some(provider)))?;
            if content_type
                .as_deref()
                .map(|value| value.starts_with("text/html"))
                .unwrap_or(false)
                || current_url.contains("accounts.google.com/CookieMismatch")
                || current_url.contains("accounts.google.com/ServiceLogin")
            {
                return Err(gemini_canvas_media_fetch_cookie_mismatch_html_error());
            }
            let resolved_mime_type = infer_gemini_canvas_media_mime_type(
                content_type.as_deref(),
                asset_mime_hint,
                &current_url,
                asset_kind_hint,
            );
            let resolved_kind =
                infer_gemini_canvas_media_kind(&resolved_mime_type, asset_kind_hint, &current_url);
            let body_base64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            return Ok(gemini_canvas::GeminiCanvasMediaAsset {
                kind: resolved_kind,
                url: current_url,
                mime_type: resolved_mime_type,
                download_token: None,
                body_base64: Some(body_base64),
                alt: None,
                width: None,
                height: None,
                duration_seconds: None,
            });
        }

        Err(gemini_canvas_media_fetch_redirect_exhausted_error())
    }
}
