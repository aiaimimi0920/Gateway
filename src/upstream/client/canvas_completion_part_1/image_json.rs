use super::*;

impl UpstreamClient {
    pub(in crate::upstream::client) async fn execute_gemini_canvas_direct_http_image_json(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        runtime: &gemini_canvas::GeminiCanvasRuntime,
        prompt: &str,
        timeout: Duration,
    ) -> Result<Value, GatewayError> {
        let preview_model = gemini_canvas::resolve_direct_http_image_model(model)?;
        let preview_base_url = gemini_canvas::direct_http_image_api_base_url(runtime);
        let google_api_base_url = runtime.api_base_url.trim_end_matches('/').to_string();
        let official_model = gemini_canvas::resolve_official_image_model(model)?;
        let is_edit_request = req.endpoint_kind == EndpointKind::ImagesEdits;
        let storage_state = gateway_object_storage()?
            .read_json(&runtime.runtime_state_object_key)
            .await?;
        let auth_user = gemini_canvas::direct_http_auth_user(payload);
        let share_url = gemini_canvas::direct_http_referrer(
            payload.base_url.trim_end_matches('/'),
            &runtime.share_id,
        );
        let app_url = format!(
            "{}{}",
            payload.base_url.trim_end_matches('/'),
            gemini_web::GEMINI_WEB_DEFAULT_APP_PATH
        );
        let session = gemini_canvas::storage_state_to_pure_http_session(
            &storage_state,
            &share_url,
            payload.base_url.trim_end_matches('/'),
            &auth_user,
        )?;
        let mut api_key_candidates =
            gemini_canvas::direct_http_google_api_keys(payload, &storage_state);
        let (harvested_api_keys, page_harvest_probes) = self
            .harvest_gemini_canvas_direct_http_api_keys(payload, runtime, &session, timeout)
            .await;
        for harvested in harvested_api_keys {
            if api_key_candidates
                .iter()
                .any(|existing| existing == &harvested)
            {
                continue;
            }
            api_key_candidates.push(harvested);
        }
        if api_key_candidates.is_empty() {
            api_key_candidates = GEMINI_CANVAS_PUBLIC_PAGE_API_KEY_FALLBACKS
                .iter()
                .map(|candidate| (*candidate).to_string())
                .collect();
        }
        enum GeminiCanvasDirectHttpImageAttemptKind {
            GenerateContent,
            ImagenPredict,
        }

        struct GeminiCanvasDirectHttpImageAttempt {
            label: &'static str,
            request_url: String,
            request_body: Value,
            kind: GeminiCanvasDirectHttpImageAttemptKind,
            preserve_cross_origin_origin: bool,
            preserve_cross_origin_referer: bool,
            include_signed_headers: bool,
            signed_origin_override: Option<String>,
            referer_override: Option<String>,
        }

        fn normalize_generate_content_image_body(
            mut body: Value,
            keep_image_config: bool,
            response_modalities: Option<&[&str]>,
        ) -> Value {
            let Some(map) = body.as_object_mut() else {
                return body;
            };
            map.remove("tools");
            map.remove("toolConfig");
            map.remove("tool_config");
            map.remove("toolChoice");
            map.remove("systemInstruction");
            if let Some(config) = map
                .get_mut("generationConfig")
                .and_then(Value::as_object_mut)
            {
                config.remove("thinkingConfig");
                config.remove("responseMimeType");
                config.remove("response_mime_type");
                match response_modalities {
                    Some(values) => {
                        config.insert("responseModalities".to_string(), json!(values));
                    }
                    None => {
                        config.remove("responseModalities");
                    }
                }
                if !keep_image_config {
                    config.remove("imageConfig");
                }
                if config.is_empty() {
                    map.remove("generationConfig");
                }
            }
            body
        }

        let page_origin = gemini_canvas_http_origin(payload);
        let clients6_request_url = format!(
            "{}/models/{}:generateContent",
            preview_base_url.trim_end_matches('/'),
            preview_model
        );
        let preview_text_and_image_body =
            gemini_canvas::build_direct_http_image_request_body(req, preview_model);
        let preview_image_only_body = gemini_canvas::build_image_request_body(req, preview_model);
        let preview_contents_only_body =
            normalize_generate_content_image_body(preview_text_and_image_body.clone(), false, None);
        let preview_aspect_only_body =
            normalize_generate_content_image_body(preview_text_and_image_body.clone(), true, None);
        let preview_image_only_no_modalities_body =
            normalize_generate_content_image_body(preview_image_only_body.clone(), true, None);
        let mut attempts = vec![
            GeminiCanvasDirectHttpImageAttempt {
                label: "canvas_web_preview_signed_app_text_image",
                request_url: clients6_request_url.clone(),
                request_body: preview_text_and_image_body.clone(),
                kind: GeminiCanvasDirectHttpImageAttemptKind::GenerateContent,
                preserve_cross_origin_origin: true,
                preserve_cross_origin_referer: true,
                include_signed_headers: true,
                signed_origin_override: Some(page_origin.clone()),
                referer_override: Some(app_url.clone()),
            },
            GeminiCanvasDirectHttpImageAttempt {
                label: "canvas_web_preview_signed_share_text_image",
                request_url: clients6_request_url.clone(),
                request_body: preview_text_and_image_body.clone(),
                kind: GeminiCanvasDirectHttpImageAttemptKind::GenerateContent,
                preserve_cross_origin_origin: true,
                preserve_cross_origin_referer: true,
                include_signed_headers: true,
                signed_origin_override: Some(page_origin.clone()),
                referer_override: Some(share_url.clone()),
            },
            GeminiCanvasDirectHttpImageAttempt {
                label: "canvas_web_preview_signed_share_image_only",
                request_url: clients6_request_url.clone(),
                request_body: preview_image_only_body.clone(),
                kind: GeminiCanvasDirectHttpImageAttemptKind::GenerateContent,
                preserve_cross_origin_origin: true,
                preserve_cross_origin_referer: true,
                include_signed_headers: true,
                signed_origin_override: Some(page_origin.clone()),
                referer_override: Some(share_url.clone()),
            },
            GeminiCanvasDirectHttpImageAttempt {
                label: "canvas_web_preview_plain_share_text_image",
                request_url: clients6_request_url.clone(),
                request_body: preview_text_and_image_body.clone(),
                kind: GeminiCanvasDirectHttpImageAttemptKind::GenerateContent,
                preserve_cross_origin_origin: true,
                preserve_cross_origin_referer: true,
                include_signed_headers: false,
                signed_origin_override: None,
                referer_override: Some(share_url.clone()),
            },
            GeminiCanvasDirectHttpImageAttempt {
                label: "canvas_web_preview_plain_minimal_image_only",
                request_url: clients6_request_url.clone(),
                request_body: preview_image_only_no_modalities_body.clone(),
                kind: GeminiCanvasDirectHttpImageAttemptKind::GenerateContent,
                preserve_cross_origin_origin: false,
                preserve_cross_origin_referer: false,
                include_signed_headers: false,
                signed_origin_override: None,
                referer_override: None,
            },
            GeminiCanvasDirectHttpImageAttempt {
                label: "canvas_web_preview_signed_share_contents_only",
                request_url: clients6_request_url.clone(),
                request_body: preview_contents_only_body.clone(),
                kind: GeminiCanvasDirectHttpImageAttemptKind::GenerateContent,
                preserve_cross_origin_origin: true,
                preserve_cross_origin_referer: true,
                include_signed_headers: true,
                signed_origin_override: Some(page_origin.clone()),
                referer_override: Some(share_url.clone()),
            },
            GeminiCanvasDirectHttpImageAttempt {
                label: "canvas_web_preview_signed_share_aspect_only",
                request_url: clients6_request_url.clone(),
                request_body: preview_aspect_only_body.clone(),
                kind: GeminiCanvasDirectHttpImageAttemptKind::GenerateContent,
                preserve_cross_origin_origin: true,
                preserve_cross_origin_referer: true,
                include_signed_headers: true,
                signed_origin_override: Some(page_origin.clone()),
                referer_override: Some(share_url.clone()),
            },
            GeminiCanvasDirectHttpImageAttempt {
                label: "canvas_web_preview_plain_share_contents_only",
                request_url: clients6_request_url.clone(),
                request_body: preview_contents_only_body,
                kind: GeminiCanvasDirectHttpImageAttemptKind::GenerateContent,
                preserve_cross_origin_origin: true,
                preserve_cross_origin_referer: true,
                include_signed_headers: false,
                signed_origin_override: None,
                referer_override: Some(share_url.clone()),
            },
            GeminiCanvasDirectHttpImageAttempt {
                label: "canvas_web_preview_plain_share_aspect_only",
                request_url: clients6_request_url.clone(),
                request_body: preview_aspect_only_body,
                kind: GeminiCanvasDirectHttpImageAttemptKind::GenerateContent,
                preserve_cross_origin_origin: true,
                preserve_cross_origin_referer: true,
                include_signed_headers: false,
                signed_origin_override: None,
                referer_override: Some(share_url.clone()),
            },
            GeminiCanvasDirectHttpImageAttempt {
                label: "google_api_preview",
                request_url: format!(
                    "{}/models/{}:generateContent",
                    google_api_base_url.trim_end_matches('/'),
                    preview_model
                ),
                request_body: gemini_canvas::build_direct_http_image_request_body(
                    req,
                    preview_model,
                ),
                kind: GeminiCanvasDirectHttpImageAttemptKind::GenerateContent,
                preserve_cross_origin_origin: false,
                preserve_cross_origin_referer: true,
                include_signed_headers: false,
                signed_origin_override: None,
                referer_override: Some(app_url.clone()),
            },
            GeminiCanvasDirectHttpImageAttempt {
                label: "google_api_official",
                request_url: format!(
                    "{}/models/{}:generateContent",
                    google_api_base_url.trim_end_matches('/'),
                    official_model
                ),
                request_body: gemini_canvas::build_image_request_body(req, official_model),
                kind: GeminiCanvasDirectHttpImageAttemptKind::GenerateContent,
                preserve_cross_origin_origin: false,
                preserve_cross_origin_referer: true,
                include_signed_headers: false,
                signed_origin_override: None,
                referer_override: Some(app_url.clone()),
            },
        ];
        if !is_edit_request {
            let imagen_predict_body = gemini_canvas::build_imagen_predict_request(req, prompt);
            for (label, imagen_model) in [
                (
                    "google_api_imagen4_predict",
                    gemini_canvas::GEMINI_CANVAS_IMAGEN_4_MODEL,
                ),
                (
                    "google_api_imagen3_predict",
                    gemini_canvas::GEMINI_CANVAS_IMAGEN_3_MODEL,
                ),
                (
                    "google_api_imagen3_legacy_predict",
                    gemini_canvas::GEMINI_CANVAS_IMAGEN_3_LEGACY_MODEL,
                ),
            ] {
                attempts.push(GeminiCanvasDirectHttpImageAttempt {
                    label,
                    request_url: format!(
                        "{}/models/{}:predict",
                        google_api_base_url.trim_end_matches('/'),
                        imagen_model
                    ),
                    request_body: imagen_predict_body.clone(),
                    kind: GeminiCanvasDirectHttpImageAttemptKind::ImagenPredict,
                    preserve_cross_origin_origin: false,
                    preserve_cross_origin_referer: true,
                    include_signed_headers: false,
                    signed_origin_override: None,
                    referer_override: Some(app_url.clone()),
                });
            }
        }
        let mut failures = Vec::new();
        let mut last_error = None;

        for attempt in attempts {
            let mut candidate_keys: Vec<Option<&str>> = if api_key_candidates.is_empty() {
                vec![None]
            } else {
                api_key_candidates
                    .iter()
                    .map(|candidate| Some(candidate.as_str()))
                    .collect()
            };
            if attempt.include_signed_headers
                && !candidate_keys.iter().any(|candidate| candidate.is_none())
            {
                candidate_keys.insert(0, None);
            }

            for api_key in candidate_keys {
                let transports: Vec<GeminiCanvasDirectHttpApiKeyTransport> = if api_key.is_some() {
                    gemini_canvas_direct_http_api_key_transports(&attempt.request_url).to_vec()
                } else {
                    vec![GeminiCanvasDirectHttpApiKeyTransport::HeaderOnly]
                };
                for transport in transports {
                    let attempt_label = if let Some(api_key) = api_key {
                        format!(
                            "{}[key={}][transport={}]",
                            attempt.label,
                            redact_gemini_canvas_api_key_for_logs(api_key),
                            transport.label()
                        )
                    } else {
                        format!("{}[key=none][transport=none]", attempt.label)
                    };
                    let body = match self
                        .execute_gemini_canvas_direct_http_json_with_options(
                            payload,
                            runtime,
                            &attempt.request_url,
                            &attempt.request_body,
                            timeout.max(Duration::from_secs(120)),
                            api_key,
                            transport,
                            attempt.signed_origin_override.as_deref(),
                            attempt.referer_override.as_deref(),
                            attempt.preserve_cross_origin_origin,
                            attempt.preserve_cross_origin_referer,
                            attempt.include_signed_headers,
                        )
                        .await
                    {
                        Ok(body) => body,
                        Err(error) => {
                            failures.push(format!(
                                "{attempt_label}={}",
                                summarize_gateway_error(&error)
                            ));
                            last_error = Some(error);
                            continue;
                        }
                    };
                    match attempt.kind {
                        GeminiCanvasDirectHttpImageAttemptKind::GenerateContent => {
                            let image = match gemini_canvas::extract_inline_image_from_generate_content_response(&body) {
                                Ok(image) => image,
                                Err(error) => {
                                    failures.push(format!(
                                        "{attempt_label}={}",
                                        summarize_gateway_error(&error)
                                    ));
                                    last_error = Some(error);
                                    continue;
                                }
                            };
                            return gemini_canvas::build_openai_images_response_from_bytes(
                                req,
                                prompt,
                                &[image],
                            );
                        }
                        GeminiCanvasDirectHttpImageAttemptKind::ImagenPredict => {
                            let images =
                                match gemini_canvas::extract_images_from_imagen_predict_response(
                                    &body,
                                ) {
                                    Ok(images) => images,
                                    Err(error) => {
                                        failures.push(format!(
                                            "{attempt_label}={}",
                                            summarize_gateway_error(&error)
                                        ));
                                        last_error = Some(error);
                                        continue;
                                    }
                                };
                            return gemini_canvas::build_openai_images_response_from_bytes(
                                req, prompt, &images,
                            );
                        }
                    }
                }
            }
        }

        let mut error =
            last_error.unwrap_or_else(gemini_canvas_image_json_attempts_exhausted_error);
        if !failures.is_empty() {
            let key_summary = if api_key_candidates.is_empty() {
                "none".to_string()
            } else {
                api_key_candidates
                    .iter()
                    .map(|candidate| redact_gemini_canvas_api_key_for_logs(candidate))
                    .collect::<Vec<_>>()
                    .join(",")
            };
            error.message = sanitize_provider_error_message(&format!(
                "{}; api_key_candidates={}; page_harvest={}; attempts={}",
                error.message,
                key_summary,
                if page_harvest_probes.is_empty() {
                    "none".to_string()
                } else {
                    page_harvest_probes.join(" | ")
                },
                failures.join(" | ")
            ));
        }
        Err(error)
    }
}
