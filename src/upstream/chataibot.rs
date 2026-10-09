use rquest::Method;
use serde_json::Value;

use crate::error::{
    classify_network_error, classify_upstream_error, sanitize_provider_error_message, GatewayError,
};
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind};
use crate::protocol::chataibot;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::client::UpstreamClient;
use crate::upstream::common::RequestPlan;
use crate::upstream::headers::build_upstream_headers_with;
use crate::upstream::response_preview_helpers::compact_response_preview;

fn parse_chataibot_http_response(
    status: u16,
    content_type: Option<&str>,
    body_text: &str,
    provider: &str,
) -> Result<Value, GatewayError> {
    if !(200..300).contains(&status) {
        return Err(classify_upstream_error(status, body_text, Some(provider)));
    }

    if body_text.trim().is_empty() {
        return Err(GatewayError::service_unavailable(format!(
            "Chataibot returned an empty successful response (HTTP {status}, content-type {}).",
            content_type.unwrap_or("<missing>")
        ))
        .with_provider(provider)
        .with_code("chataibot_empty_response"));
    }

    serde_json::from_str(body_text).map_err(|_| {
        let preview = sanitize_provider_error_message(&compact_response_preview(body_text, 160));
        GatewayError::server_error(format!(
            "Chataibot returned a non-JSON successful response (HTTP {status}, content-type {}; body preview: {preview}).",
            content_type.unwrap_or("<missing>")
        ))
        .with_provider(provider)
        .with_code("chataibot_invalid_response")
    })
}

pub(crate) fn unsupported_request_plan_error() -> GatewayError {
    GatewayError::bad_request(
        "Chataibot adapters currently support only image generation/edit passthrough endpoints",
    )
    .with_code("unsupported_chataibot_endpoint")
}

impl UpstreamClient {
    pub(crate) async fn execute_chataibot_images(
        &self,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        match req.endpoint_kind {
            EndpointKind::ImagesGenerations | EndpointKind::ImagesEdits => {}
            _ => {
                return Err(GatewayError::bad_request(
                    "Chataibot adapters only support /v1/images/generations and /v1/images/edits.",
                )
                .with_code("unsupported_chataibot_endpoint"));
            }
        }

        if req.raw_body.get("mask").is_some() {
            return Err(GatewayError::bad_request(
                "Chataibot image adapters do not support mask-based image edits.",
            )
            .with_provider("chataibot_compatible")
            .with_code("unsupported_chataibot_mask"));
        }

        let provider = "chataibot_compatible";
        let headers = build_upstream_headers_with(payload, extra_headers);
        let prompt = chataibot::prompt_from_request(req)?;
        let uploads = chataibot::extract_uploads_from_request_body(&req.raw_body)?;
        let ratio = chataibot::aspect_ratio_from_request(req);
        let spec = chataibot::resolve_model_spec(model).ok_or_else(|| {
            GatewayError::bad_request(format!("Unsupported Chataibot image model '{model}'."))
                .with_provider(provider)
                .with_code("unsupported_chataibot_model")
        })?;

        let update_plan = RequestPlan {
            method: Method::POST,
            url: format!("{}/api/user/update", payload.base_url.trim_end_matches('/')),
            query: Vec::new(),
            body: Some(chataibot::build_update_settings_request(&ratio)),
            response_kind: req.endpoint_kind,
        };

        let update_response = self
            .send_plan(&update_plan, headers.clone())
            .send()
            .await
            .map_err(|e| classify_network_error(&e, Some(provider)))?;
        let update_status = update_response.status().as_u16();
        if !update_response.status().is_success() {
            let body_text = update_response
                .text()
                .await
                .unwrap_or_else(|_| String::from("<unreadable body>"));
            return Err(classify_upstream_error(
                update_status,
                &body_text,
                Some(provider),
            ));
        }

        let image_urls = if uploads.len() > 1 {
            let merge_mode = spec.merge_mode.ok_or_else(|| {
                GatewayError::bad_request(format!(
                    "Model '{model}' does not support multi-image merge requests."
                ))
                .with_provider(provider)
                .with_code("unsupported_chataibot_merge_model")
            })?;
            let multipart = chataibot::build_merge_multipart_body(&prompt, &uploads, merge_mode)?;
            self.execute_chataibot_multipart(
                payload,
                provider,
                "/api/file/merge",
                headers.clone(),
                multipart,
            )
            .await?
        } else if let Some(upload) = uploads.first() {
            let edit_mode = spec.edit_mode.ok_or_else(|| {
                GatewayError::bad_request(format!(
                    "Model '{model}' does not support image edit requests."
                ))
                .with_provider(provider)
                .with_code("unsupported_chataibot_edit_model")
            })?;
            let multipart = chataibot::build_edit_multipart_body(&prompt, upload, edit_mode)?;
            self.execute_chataibot_multipart(
                payload,
                provider,
                "/api/file/recognize",
                headers.clone(),
                multipart,
            )
            .await?
        } else {
            let generate_plan = RequestPlan {
                method: Method::POST,
                url: format!(
                    "{}/api/image/generate",
                    payload.base_url.trim_end_matches('/')
                ),
                query: Vec::new(),
                body: Some(chataibot::build_generation_request(&prompt, spec)),
                response_kind: req.endpoint_kind,
            };

            let response = self
                .send_plan(&generate_plan, headers.clone())
                .send()
                .await
                .map_err(|e| classify_network_error(&e, Some(provider)))?;
            let status = response.status().as_u16();
            let content_type = response
                .headers()
                .get(rquest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            let body_text = response
                .text()
                .await
                .map_err(|e| classify_network_error(&e, Some(provider)))?;
            let body = parse_chataibot_http_response(
                status,
                content_type.as_deref(),
                &body_text,
                provider,
            )?;
            chataibot::extract_image_urls(&body)?
        };

        if chataibot::prefers_url_response(req)? {
            return chataibot::build_openai_images_response_from_urls(req, &prompt, &image_urls);
        }

        let mut images = Vec::with_capacity(image_urls.len());
        for url in image_urls
            .iter()
            .take(chataibot::requested_image_count(req))
        {
            let response = self
                .http
                .request(Method::GET, url)
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

            let mime_type = response
                .headers()
                .get(rquest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(str::trim)
                .filter(|value| value.starts_with("image/"))
                .unwrap_or("image/png")
                .to_string();

            let bytes = response
                .bytes()
                .await
                .map_err(|e| classify_network_error(&e, Some(provider)))?;
            images.push((mime_type, bytes.to_vec()));
        }

        Ok(chataibot::build_openai_images_response_from_bytes(
            req, &prompt, &images,
        ))
    }

    async fn execute_chataibot_multipart(
        &self,
        payload: &ProviderAccountPayload,
        provider: &str,
        path: &str,
        mut headers: rquest::header::HeaderMap,
        body: chataibot::MultipartBody,
    ) -> Result<Vec<String>, GatewayError> {
        if let Ok(content_type) = rquest::header::HeaderValue::from_str(&body.content_type) {
            headers.insert(rquest::header::CONTENT_TYPE, content_type);
        }

        let response = self
            .http
            .request(
                Method::POST,
                format!("{}{}", payload.base_url.trim_end_matches('/'), path),
            )
            .headers(headers)
            .body(body.bytes)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|e| classify_network_error(&e, Some(provider)))?;

        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get(rquest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let body_text = response
            .text()
            .await
            .map_err(|e| classify_network_error(&e, Some(provider)))?;
        let body =
            parse_chataibot_http_response(status, content_type.as_deref(), &body_text, provider)?;

        chataibot::extract_image_urls(&body)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::{json, Value};

    use super::{parse_chataibot_http_response, unsupported_request_plan_error};
    use crate::error::ErrorKind;
    use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind, ProtocolFamily};
    use crate::routing::candidate::ProviderAccountPayload;
    use crate::upstream::client::UpstreamClient;

    fn make_payload(adapter: &str, base_url: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
            discovered_protocols: Vec::new(),
            adapter: adapter.to_string(),
            base_url: base_url.to_string(),
            api_key: "sk-test".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: None,
            headers: HashMap::new(),
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: None,
            chat_completions_path: None,
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            extra_body: None,
            session_auth: None,
            keepalive: None,
        }
    }

    fn make_request(protocol: ProtocolFamily, endpoint: EndpointKind) -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: protocol,
            endpoint_kind: endpoint,
            requested_model: Some("test-model".to_string()),
            stream: false,
            messages: vec![],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }

    #[test]
    fn chataibot_unsupported_request_plan_error_matches_contract() {
        let err = unsupported_request_plan_error();
        assert_eq!(err.http_status, Some(400));
        assert_eq!(err.code.as_deref(), Some("unsupported_chataibot_endpoint"));
        assert!(err.message.contains("image generation/edit"));
    }

    #[test]
    fn plan_chataibot_chat_endpoint_rejected_locally() {
        let payload = make_payload("chataibot_compatible", "https://chataibot.pro");
        let req = make_request(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
        let err = UpstreamClient::build_request_plan(&payload, &req, "google-nano-banana", false)
            .expect_err("chataibot chat requests should be rejected");
        assert_eq!(err.http_status, Some(400));
        assert_eq!(err.code.as_deref(), Some("unsupported_chataibot_endpoint"));
    }

    #[tokio::test]
    async fn execute_chataibot_mask_edit_rejected_before_send() {
        let payload = make_payload("chataibot_compatible", "https://chataibot.pro");
        let mut req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesEdits);
        req.requested_model = Some("qwen-lora".to_string());
        req.raw_body = json!({
            "prompt": "mask should fail",
            "image": "data:image/png;base64,aGVsbG8=",
            "mask": "data:image/png;base64,aGVsbG8=",
        });
        let client = UpstreamClient::new(5);
        let err = client
            .execute_chataibot_images(&payload, &req, "qwen-lora", None)
            .await
            .expect_err("mask-based chataibot edits should be rejected");
        assert_eq!(err.http_status, Some(400));
        assert_eq!(err.code.as_deref(), Some("unsupported_chataibot_mask"));
    }

    #[test]
    fn chataibot_non_success_plain_text_is_classified_before_json_decode() {
        let err = parse_chataibot_http_response(
            401,
            Some("text/plain"),
            "Unauthorized",
            "chataibot_compatible",
        )
        .expect_err("plain-text 401 must be classified as an upstream error");

        assert_eq!(err.kind, ErrorKind::Authentication);
        assert_eq!(err.http_status, Some(401));
        assert_eq!(err.message, "Unauthorized");
    }

    #[test]
    fn chataibot_empty_success_has_explicit_error_code() {
        let err = parse_chataibot_http_response(
            204,
            Some("application/json"),
            "  ",
            "chataibot_compatible",
        )
        .expect_err("an empty successful response must not be accepted");

        assert_eq!(err.code.as_deref(), Some("chataibot_empty_response"));
        assert_eq!(err.provider_name.as_deref(), Some("chataibot_compatible"));
    }

    #[test]
    fn chataibot_non_json_success_has_explicit_error_code_and_safe_preview() {
        let err = parse_chataibot_http_response(
            200,
            Some("text/html; charset=utf-8"),
            "<html> token=secret-value </html>",
            "chataibot_compatible",
        )
        .expect_err("a non-JSON successful response must not be accepted");

        assert_eq!(err.code.as_deref(), Some("chataibot_invalid_response"));
        assert!(err.message.contains("text/html; charset=utf-8"));
        assert!(!err.message.contains("secret-value"));
    }

    #[test]
    fn chataibot_valid_json_success_is_preserved() {
        let body = parse_chataibot_http_response(
            200,
            Some("application/json"),
            r#"{"imageUrl":"https://example.com/paris.png"}"#,
            "chataibot_compatible",
        )
        .expect("valid JSON response should pass");

        assert_eq!(
            body.get("imageUrl").and_then(Value::as_str),
            Some("https://example.com/paris.png")
        );
    }
}
