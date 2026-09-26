use super::*;

impl UpstreamClient {
    pub(super) async fn execute_gemini_business_images(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        match req.endpoint_kind {
            EndpointKind::ImagesGenerations | EndpointKind::ImagesEdits => {}
            _ => return Err(gemini_business_unsupported_images_endpoint_error()),
        }

        let provider = "gemini_business_compatible";
        let headers = build_upstream_headers_with(payload, extra_headers);
        let runtime = gemini_business::runtime_from_payload(payload)?;
        let prompt = gemini_business::prompt_from_request(req)?;
        let uploads = gemini_business::extract_uploads_from_request_body(&req.raw_body)?;
        let mut uploaded_file_ids = Vec::with_capacity(uploads.len());

        for upload in uploads {
            let upload_plan = gemini_business::build_context_file_upload_plan(
                payload,
                &runtime,
                &upload,
                req.endpoint_kind,
            );

            let response = self
                .send_plan(&upload_plan, headers.clone())
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

            let upload_json: Value = response
                .json()
                .await
                .map_err(|e| classify_network_error(&e, Some(provider)))?;

            let file_id = extract_gemini_business_upload_file_id(&upload_json, provider)?;

            uploaded_file_ids.push(file_id.to_string());
        }

        let assist_plan = gemini_business::build_stream_assist_plan(
            payload,
            req,
            model,
            &runtime,
            &uploaded_file_ids,
        )?;

        let response = self
            .send_plan(&assist_plan, headers.clone())
            .send()
            .await
            .map_err(|e| classify_network_error(&e, Some(provider)))?;

        let status = response.status().as_u16();
        let body_text = response
            .text()
            .await
            .map_err(|e| classify_network_error(&e, Some(provider)))?;

        if !(200..300).contains(&status) {
            return Err(classify_upstream_error(status, &body_text, Some(provider)));
        }

        let response_objects = parse_gemini_business_stream_response_objects(&body_text, provider)?;

        let (session_name, generated_files) =
            gemini_business::extract_generated_files(&response_objects, &runtime.session)?;

        let mut images = Vec::with_capacity(generated_files.len());
        for generated in generated_files {
            let download_url = format!(
                "{}/{}:downloadFile?fileId={}&alt=media",
                payload.base_url.trim_end_matches('/'),
                session_name,
                generated.file_id
            );

            let response = self
                .http
                .request(Method::GET, &download_url)
                .headers(headers.clone())
                .timeout(self.timeout)
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

            let bytes = response
                .bytes()
                .await
                .map_err(|e| classify_network_error(&e, Some(provider)))?;
            images.push((generated.mime_type, bytes.to_vec()));
        }

        gemini_business::build_openai_images_response(req, &prompt, &images)
    }
}
