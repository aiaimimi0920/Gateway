use super::*;

#[path = "canvas_preflight_part_1/media_selection.rs"]
mod media_selection;

impl UpstreamClient {
    pub(super) async fn prepare_gemini_canvas_direct_http_text_context(
        &self,
        payload: &ProviderAccountPayload,
        _model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        timeout: Duration,
    ) -> Result<
        (
            gemini_canvas::GeminiCanvasPureHttpSession,
            gemini_web::GeminiWebBootstrap,
            Option<String>,
        ),
        GatewayError,
    > {
        let provider = "gemini_canvas_compatible";
        let configured_base_url = payload.base_url.trim_end_matches('/');
        let effective_base_url = if configured_base_url.is_empty() {
            gemini_canvas_http_origin(payload)
        } else {
            configured_base_url.to_string()
        };
        let bootstrap_url = if payload.adapter == "gemini_canvas_program_web_reverse_compatible" {
            gemini_canvas_program_web_reverse_modular::gemini_canvas_program_payload_page_url(
                payload,
                &effective_base_url,
            )
            .unwrap_or_else(|| {
                format!(
                    "{}{}",
                    effective_base_url,
                    gemini_web::GEMINI_WEB_DEFAULT_APP_PATH
                )
            })
        } else {
            format!(
                "{}{}",
                effective_base_url,
                gemini_web::GEMINI_WEB_DEFAULT_APP_PATH
            )
        };
        let storage_state = gateway_object_storage()?
            .read_json(&runtime.runtime_state_object_key)
            .await?;
        let batchexecute_header_id =
            gemini_canvas::harvest_text_batchexecute_header_id(&storage_state);
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
                &bootstrap_url,
                &effective_base_url,
                &auth_user,
            )?
        };

        let mut headers = HeaderMap::new();
        apply_gemini_canvas_navigation_headers(&mut headers);
        apply_gemini_canvas_cookie_header(&mut headers, &session);
        insert_header_map_value(
            &mut headers,
            "accept-language",
            &gemini_canvas::locale_from_payload(payload),
        );

        let bootstrap_response = self
            .http
            .request(Method::GET, &bootstrap_url)
            .headers(headers)
            .timeout(timeout.max(Duration::from_secs(30)))
            .send()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;
        let bootstrap_status = bootstrap_response.status().as_u16();
        let bootstrap_content_type = bootstrap_response
            .headers()
            .get(rquest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let bootstrap_body = bootstrap_response
            .text()
            .await
            .map_err(|error| classify_network_error(&error, Some(provider)))?;
        if !(200..300).contains(&bootstrap_status)
            || gemini_web::response_indicates_browser_challenge(
                bootstrap_status,
                bootstrap_content_type.as_deref(),
                &bootstrap_body,
            )
            || gemini_web::response_indicates_session_invalid(
                bootstrap_status,
                bootstrap_content_type.as_deref(),
                &bootstrap_body,
            )
        {
            return Err(classify_gemini_canvas_pure_http_error(
                bootstrap_status,
                bootstrap_content_type.as_deref(),
                &bootstrap_body,
            ));
        }

        let fallback_bootstrap =
            gemini_web::bootstrap_from_payload_cache(payload.extra_body.as_ref());
        let bootstrap = gemini_web::parse_bootstrap_from_app_html(
            &bootstrap_body,
            payload
                .extra_body
                .as_ref()
                .and_then(|extra| extra.get("language").and_then(Value::as_str)),
        )
        .or_else(|primary_error| fallback_bootstrap.clone().ok_or(primary_error))?;
        let bootstrap =
            gemini_web::merge_bootstrap_from_fallback(bootstrap, fallback_bootstrap.as_ref());
        Ok((session, bootstrap, batchexecute_header_id))
    }

    pub(super) async fn execute_gemini_canvas_page_init_preflight_sequence(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        source_path: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        timeout: Duration,
        continue_on_error: bool,
    ) -> Result<(), GatewayError> {
        for (rpcid, rpc_payload, model_header) in [
            (
                "otAQ7b",
                json!([]),
                gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER_SHORT,
            ),
            (
                "sJBwce",
                json!([[1, 2]]),
                gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER_SHORT,
            ),
            (
                "DYBcR",
                json!([bootstrap.language]),
                gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER_SHORT,
            ),
            (
                "cYRIkd",
                json!([bootstrap.language]),
                gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER_SHORT,
            ),
            (
                "GPRiHf",
                json!([]),
                gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER_SHORT,
            ),
            (
                "maGuAc",
                json!([0]),
                gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER_SHORT,
            ),
            (
                "maGuAc",
                json!([1]),
                gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER_SHORT,
            ),
            (
                "Te6DCf",
                json!([[bootstrap.language], [1]]),
                gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER_SHORT,
            ),
            (
                "mhs1xe",
                json!([[1, 3]]),
                gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER,
            ),
            (
                "K4WWud",
                json!([[0], [bootstrap.language]]),
                gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER,
            ),
            (
                "ku4Jyf",
                json!([
                    bootstrap.language,
                    null,
                    null,
                    null,
                    4,
                    null,
                    null,
                    [2, 3, 7, 17],
                    null,
                    []
                ]),
                gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER,
            ),
            (
                "ozz5Z",
                json!([[
                    [[null, "1", 447]],
                    [[null, "1", 448]],
                    [[null, "1", 702]],
                    [[null, "1", 961]],
                    [[null, "1", 960]],
                    [[null, "1", 1062]],
                    [[null, "1", 1240]],
                    [[null, "1", 1237]],
                    [[null, "1", 1238]],
                    [[null, "1", 1239]],
                    [[null, "1", 1241]]
                ]]),
                gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER,
            ),
            (
                "CNgdBe",
                json!([1, [bootstrap.language], 0]),
                gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER,
            ),
        ] {
            let result = self
                .execute_gemini_canvas_text_generic_preflight(
                    payload,
                    model,
                    bootstrap,
                    source_path,
                    session,
                    rpcid,
                    rpc_payload,
                    model_header,
                    timeout,
                )
                .await;
            if let Err(error) = result {
                if continue_on_error {
                    debug!(
                        provider = "gemini_canvas_compatible",
                        error = %summarize_gateway_error(&error),
                        rpcid,
                        source_path,
                        "gemini canvas page-init preflight failed; continuing"
                    );
                } else {
                    return Err(error);
                }
            }
        }

        let (state_len, tail_index, tail_value, marker) =
            gemini_canvas_direct_http_text_fast_version_preflight_spec();
        let result = self
            .execute_gemini_canvas_text_state_variant_preflight(
                payload,
                model,
                bootstrap,
                source_path,
                session,
                state_len,
                tail_index,
                tail_value,
                marker,
                timeout,
            )
            .await;
        if let Err(error) = result {
            if continue_on_error {
                debug!(
                    provider = "gemini_canvas_compatible",
                    error = %summarize_gateway_error(&error),
                    marker,
                    source_path,
                    "gemini canvas page-init state variant failed; continuing"
                );
            } else {
                return Err(error);
            }
        }

        Ok(())
    }

    pub(super) async fn execute_gemini_canvas_mode_selection_preflight(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        source_path: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        selected_id: &str,
        batchexecute_header_id: Option<&str>,
        timeout: Duration,
    ) -> Result<(), GatewayError> {
        let request = gemini_canvas::build_mode_selection_preflight_request(
            bootstrap,
            source_path,
            selected_id,
        )?;
        let model_header =
            gemini_canvas::build_text_batchexecute_model_header(None, batchexecute_header_id);

        if let Err(mut classified) = self
            .send_gemini_canvas_text_batchexecute_request(
                payload,
                model,
                &request,
                session,
                &model_header,
                timeout,
            )
            .await
        {
            let rpcids = request
                .query
                .iter()
                .find(|(key, _)| key == "rpcids")
                .map(|(_, value)| value.as_str())
                .unwrap_or("<unknown>");
            let source_path = request
                .query
                .iter()
                .find(|(key, _)| key == "source-path")
                .map(|(_, value)| value.as_str())
                .unwrap_or("<unknown>");
            let reqid = request
                .query
                .iter()
                .find(|(key, _)| key == "_reqid")
                .map(|(_, value)| value.as_str())
                .unwrap_or("<unknown>");
            classified.message = sanitize_provider_error_message(&format!(
                "Gemini Canvas batchexecute failed. rpcids={rpcids}; source_path={source_path}; reqid={reqid}; {}",
                classified.message
            ));
            return Err(classified);
        }

        Ok(())
    }

    pub(super) async fn execute_gemini_canvas_text_mode_selection_preflight(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        source_path: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        _origin: &str,
        _authorization: &str,
        timeout: Duration,
    ) -> Result<(), GatewayError> {
        self.execute_gemini_canvas_mode_selection_preflight(
            payload,
            model,
            bootstrap,
            source_path,
            session,
            gemini_canvas::GEMINI_CANVAS_TEXT_LAST_SELECTED_MODE_ID,
            None,
            timeout,
        )
        .await
    }
}
