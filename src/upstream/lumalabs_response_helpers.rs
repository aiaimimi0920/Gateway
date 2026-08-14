use crate::error::{classify_upstream_error, GatewayError};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::browser_worker_types::LumalabsBrowserWorkerResult;
use crate::upstream::lumalabs_runtime_helpers::PreparedLumalabsBrowserExecutorServiceInput;
use serde_json::json;
use std::time::Duration;

#[derive(Debug)]
pub(crate) struct PreparedLumalabsMediaPlan {
    pub(crate) operation: crate::protocol::lumalabs::LumalabsMediaOperation,
    pub(crate) prompt: String,
    pub(crate) action_body: serde_json::Value,
    pub(crate) auto_discover_action_type: bool,
    pub(crate) media_operation: &'static str,
    pub(crate) artifact_field: String,
    pub(crate) base_url: String,
    pub(crate) request_timeout: Duration,
    pub(crate) realm_id: String,
    pub(crate) locale: String,
}

#[derive(Debug)]
pub(crate) struct PreparedLumalabsExecutionContext {
    pub(crate) media_plan: PreparedLumalabsMediaPlan,
    pub(crate) browser_input: PreparedLumalabsBrowserExecutorServiceInput,
}

pub(crate) fn parse_lumalabs_browser_worker_output(
    stdout: &str,
    stderr: &str,
) -> Result<LumalabsBrowserWorkerResult, GatewayError> {
    if stdout.trim().is_empty() {
        return Err(crate::protocol::lumalabs::empty_browser_worker_output_error(stderr));
    }

    serde_json::from_str::<LumalabsBrowserWorkerResult>(stdout).map_err(|error| {
        crate::protocol::lumalabs::browser_worker_output_parse_error(
            error.to_string().as_str(),
            stdout,
        )
    })
}

pub(crate) fn parse_lumalabs_browser_worker_verified_output(
    stdout: &str,
    stderr: &str,
    provider: &str,
) -> Result<String, GatewayError> {
    let result = parse_lumalabs_browser_worker_output(stdout, stderr)?;
    resolve_lumalabs_browser_worker_result(result, stderr, provider)
}

pub(crate) fn classify_lumalabs_browser_worker_failure(
    result: LumalabsBrowserWorkerResult,
    stderr: &str,
    provider: &str,
) -> GatewayError {
    let error = result.error;
    let status = error.as_ref().and_then(|entry| entry.status).unwrap_or(500);
    let body_text = error
        .as_ref()
        .and_then(|entry| entry.body.as_deref())
        .or_else(|| stderr.is_empty().then_some("").or(Some(stderr)))
        .unwrap_or_default();
    let mut gateway_error = classify_upstream_error(status, body_text, Some(provider));
    if let Some(worker_error) = error {
        if let Some(code) = worker_error.code {
            gateway_error.code = Some(code);
        }
        if let Some(message) = worker_error.message {
            gateway_error.message = message;
        }
        if gateway_error.http_status.is_none() {
            gateway_error.http_status = Some(status);
        }
    }
    gateway_error
}

pub(crate) fn extract_lumalabs_browser_worker_success(
    result: LumalabsBrowserWorkerResult,
) -> Result<String, GatewayError> {
    crate::protocol::lumalabs::extract_browser_worker_signed_url(result.signed_url)
}

pub(crate) fn resolve_lumalabs_browser_worker_result(
    result: LumalabsBrowserWorkerResult,
    stderr: &str,
    provider: &str,
) -> Result<String, GatewayError> {
    if result.ok {
        return extract_lumalabs_browser_worker_success(result);
    }

    Err(classify_lumalabs_browser_worker_failure(
        result, stderr, provider,
    ))
}

pub(crate) fn build_lumalabs_browser_executor_service_result(
    signed_url: &str,
) -> serde_json::Value {
    json!({
        "signedUrl": signed_url,
    })
}

pub(crate) fn parse_lumalabs_remote_browser_executor_signed_url(
    value: &serde_json::Value,
) -> Result<String, GatewayError> {
    crate::protocol::lumalabs::extract_remote_executor_signed_url(value)
}

pub(crate) fn prepare_lumalabs_media_plan(
    payload: &ProviderAccountPayload,
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    model: &str,
    default_timeout: Duration,
) -> Result<PreparedLumalabsMediaPlan, GatewayError> {
    let operation =
        crate::protocol::lumalabs::LumalabsMediaOperation::from_endpoint_kind(req.endpoint_kind)?;

    if crate::protocol::lumalabs::requested_output_count(req) > 1 {
        return Err(operation.unsupported_output_count_error());
    }

    if operation == crate::protocol::lumalabs::LumalabsMediaOperation::Image {
        let has_input_images = req.raw_body.get("image").is_some()
            || req
                .raw_body
                .get("images")
                .and_then(|value| value.as_array())
                .map(|values| !values.is_empty())
                .unwrap_or(false);
        if has_input_images || req.raw_body.get("mask").is_some() {
            return Err(crate::protocol::lumalabs::unsupported_image_inputs_error());
        }
    }

    let runtime = crate::protocol::lumalabs::runtime_from_payload(payload)?;
    let prompt = crate::protocol::lumalabs::prompt_from_request_for_operation(req, operation)?;
    let optimistic_output_id = crate::protocol::lumalabs::generate_optimistic_output_id();
    let action_body = crate::protocol::lumalabs::build_action_request_for_operation(
        req,
        &runtime,
        operation,
        model,
        &prompt,
        &optimistic_output_id,
    );
    let auto_discover_action_type =
        crate::protocol::lumalabs::should_auto_discover_action_type(req, &runtime, operation);
    let media_operation = crate::protocol::lumalabs::media_operation_name(operation);
    let artifact_field =
        crate::protocol::lumalabs::output_artifact_field_for_operation(req, &runtime, operation);
    let page_base_url = payload.base_url.trim_end_matches('/').to_string();
    let request_timeout = match operation {
        crate::protocol::lumalabs::LumalabsMediaOperation::Image => {
            default_timeout.max(Duration::from_secs(90))
        }
        crate::protocol::lumalabs::LumalabsMediaOperation::Video => {
            default_timeout.max(Duration::from_secs(600))
        }
        crate::protocol::lumalabs::LumalabsMediaOperation::Audio => {
            default_timeout.max(Duration::from_secs(300))
        }
    };

    Ok(PreparedLumalabsMediaPlan {
        operation,
        prompt,
        action_body,
        auto_discover_action_type,
        media_operation,
        artifact_field,
        base_url: page_base_url,
        request_timeout,
        realm_id: runtime.realm_id,
        locale: crate::upstream::lumalabs_runtime_helpers::lumalabs_locale(payload).to_string(),
    })
}

pub(crate) fn prepare_lumalabs_execution_context(
    payload: &ProviderAccountPayload,
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    model: &str,
    default_timeout: Duration,
) -> Result<PreparedLumalabsExecutionContext, GatewayError> {
    let media_plan = prepare_lumalabs_media_plan(payload, req, model, default_timeout)?;
    Ok(PreparedLumalabsExecutionContext {
        browser_input: prepare_lumalabs_browser_execution_input(&media_plan, &payload.api_key),
        media_plan,
    })
}

pub(crate) fn prepare_lumalabs_browser_execution_input(
    media_plan: &PreparedLumalabsMediaPlan,
    session_token: &str,
) -> PreparedLumalabsBrowserExecutorServiceInput {
    PreparedLumalabsBrowserExecutorServiceInput {
        base_url: media_plan.base_url.clone(),
        realm_id: media_plan.realm_id.clone(),
        media_operation: Some(media_plan.media_operation.to_string()),
        artifact_field: media_plan.artifact_field.clone(),
        session_token: session_token.to_string(),
        action_body: media_plan.action_body.clone(),
        auto_discover_action_type: media_plan.auto_discover_action_type,
        locale: media_plan.locale.clone(),
        timeout: media_plan.request_timeout,
    }
}

pub(crate) fn resolve_lumalabs_image_generation_plan(
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    prompt: &str,
    signed_url: &str,
) -> Result<Option<serde_json::Value>, GatewayError> {
    if crate::protocol::lumalabs::prefers_url_response(req)? {
        return Ok(Some(
            crate::protocol::lumalabs::build_openai_images_response_from_url(
                req, prompt, signed_url,
            ),
        ));
    }
    Ok(None)
}

pub(crate) fn build_lumalabs_non_image_generation_response(
    operation: crate::protocol::lumalabs::LumalabsMediaOperation,
    model: &str,
    prompt: &str,
    signed_url: &str,
) -> serde_json::Value {
    match operation {
        crate::protocol::lumalabs::LumalabsMediaOperation::Video => {
            crate::protocol::lumalabs::build_video_generation_response(model, prompt, signed_url)
        }
        crate::protocol::lumalabs::LumalabsMediaOperation::Audio => {
            crate::protocol::lumalabs::build_audio_generation_response(model, prompt, signed_url)
        }
        crate::protocol::lumalabs::LumalabsMediaOperation::Image => {
            unreachable!("image responses use the image generation plan helper")
        }
    }
}

pub(crate) fn classify_lumalabs_media_fetch_error(status: u16, body_text: &str) -> GatewayError {
    classify_upstream_error(status, body_text, Some("lumalabs_compatible"))
}

pub(crate) fn ensure_successful_lumalabs_media_fetch_status(
    status: u16,
    body_text: &str,
) -> Result<(), GatewayError> {
    if (200..300).contains(&status) {
        return Ok(());
    }

    Err(classify_lumalabs_media_fetch_error(status, body_text))
}

pub(crate) fn resolve_lumalabs_downloaded_image_mime_type(
    headers: &rquest::header::HeaderMap,
    signed_url: &str,
) -> String {
    headers
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| value.starts_with("image/"))
        .map(ToString::to_string)
        .unwrap_or_else(|| crate::protocol::lumalabs::infer_mime_type_from_url(signed_url))
}

pub(crate) fn build_lumalabs_downloaded_image_response(
    prompt: &str,
    signed_url: &str,
    headers: &rquest::header::HeaderMap,
    bytes: &[u8],
) -> serde_json::Value {
    let mime_type = resolve_lumalabs_downloaded_image_mime_type(headers, signed_url);
    crate::protocol::lumalabs::build_openai_images_response_from_bytes(prompt, &mime_type, bytes)
}

#[cfg(test)]
mod tests {
    use super::{
        build_lumalabs_browser_executor_service_result, build_lumalabs_downloaded_image_response,
        build_lumalabs_non_image_generation_response, classify_lumalabs_browser_worker_failure,
        classify_lumalabs_media_fetch_error, ensure_successful_lumalabs_media_fetch_status,
        extract_lumalabs_browser_worker_success, parse_lumalabs_browser_worker_output,
        parse_lumalabs_browser_worker_verified_output,
        parse_lumalabs_remote_browser_executor_signed_url,
        prepare_lumalabs_browser_execution_input, prepare_lumalabs_execution_context,
        prepare_lumalabs_media_plan, resolve_lumalabs_browser_worker_result,
        resolve_lumalabs_downloaded_image_mime_type, resolve_lumalabs_image_generation_plan,
    };
    use crate::routing::candidate::ProviderAccountPayload;
    use std::collections::HashMap;
    use std::time::Duration;

    fn make_payload(adapter: &str, base_url: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
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
            extra_body: Some(HashMap::from([(
                "realmId".to_string(),
                serde_json::json!("realm-123"),
            )])),
            session_auth: None,
            keepalive: None,
        }
    }

    fn image_generation_request(
        raw_body: serde_json::Value,
    ) -> crate::protocol::canonical::CanonicalRelayRequest {
        crate::protocol::canonical::CanonicalRelayRequest {
            protocol_family: crate::protocol::canonical::ProtocolFamily::OpenAi,
            endpoint_kind: crate::protocol::canonical::EndpointKind::ImagesGenerations,
            requested_model: None,
            stream: false,
            messages: Vec::new(),
            tools: Vec::new(),
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body,
            previous_response_id: None,
            explicit_session_key: None,
            extra: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn parse_lumalabs_browser_worker_output_reads_result_contract() {
        let parsed = parse_lumalabs_browser_worker_output(
            "{\"ok\":true,\"signedUrl\":\"https://cdn.example.com/file.mp4\"}",
            "",
        )
        .expect("lumalabs worker output");

        assert!(parsed.ok);
        assert_eq!(
            parsed.signed_url.as_deref(),
            Some("https://cdn.example.com/file.mp4")
        );
    }

    #[test]
    fn parse_lumalabs_browser_worker_output_rejects_empty_stdout_contract() {
        let error = parse_lumalabs_browser_worker_output("", "permission denied")
            .expect_err("empty stdout should fail");
        assert_eq!(
            error.code.as_deref(),
            Some("lumalabs_browser_worker_empty_output")
        );
        assert_eq!(error.provider_name.as_deref(), Some("lumalabs_compatible"));
    }

    #[test]
    fn prepare_lumalabs_media_plan_builds_image_contract() {
        let payload = make_payload("lumalabs_compatible", "https://app.lumalabs.ai/");
        let req = image_generation_request(serde_json::json!({
            "prompt": "surreal glass flower"
        }));

        let plan = prepare_lumalabs_media_plan(
            &payload,
            &req,
            crate::protocol::lumalabs::LUMALABS_DEFAULT_IMAGE_MODEL,
            Duration::from_secs(30),
        )
        .expect("lumalabs media plan");

        assert_eq!(
            plan.operation,
            crate::protocol::lumalabs::LumalabsMediaOperation::Image
        );
        assert_eq!(plan.prompt, "surreal glass flower");
        assert_eq!(plan.base_url, "https://app.lumalabs.ai");
        assert_eq!(plan.realm_id, "realm-123");
        assert_eq!(plan.media_operation, "image");
        assert_eq!(plan.artifact_field, "image");
        assert_eq!(plan.request_timeout, Duration::from_secs(90));
        assert!(plan.auto_discover_action_type);
        assert_eq!(plan.locale, "zh-CN");
        assert_eq!(plan.action_body["type"], "create_image_uni_1");
    }

    #[test]
    fn prepare_lumalabs_media_plan_preserves_unsupported_image_inputs_contract() {
        let payload = make_payload("lumalabs_compatible", "https://app.lumalabs.ai");
        let req = image_generation_request(serde_json::json!({
            "prompt": "edit this",
            "image": "data:image/png;base64,AAAA"
        }));

        let error = prepare_lumalabs_media_plan(
            &payload,
            &req,
            crate::protocol::lumalabs::LUMALABS_DEFAULT_IMAGE_MODEL,
            Duration::from_secs(30),
        )
        .expect_err("image inputs should fail");

        assert_eq!(
            error.code.as_deref(),
            Some("unsupported_lumalabs_image_inputs")
        );
        assert_eq!(error.provider_name.as_deref(), Some("lumalabs_compatible"));
    }

    #[test]
    fn prepare_lumalabs_browser_execution_input_reads_contract() {
        let payload = make_payload("lumalabs_compatible", "https://app.lumalabs.ai/");
        let req = image_generation_request(serde_json::json!({
            "prompt": "surreal glass flower"
        }));

        let plan = prepare_lumalabs_media_plan(
            &payload,
            &req,
            crate::protocol::lumalabs::LUMALABS_DEFAULT_IMAGE_MODEL,
            Duration::from_secs(30),
        )
        .expect("lumalabs media plan");
        let prepared = prepare_lumalabs_browser_execution_input(&plan, "sk-test");

        assert_eq!(prepared.base_url, "https://app.lumalabs.ai");
        assert_eq!(prepared.realm_id, "realm-123");
        assert_eq!(prepared.media_operation.as_deref(), Some("image"));
        assert_eq!(prepared.artifact_field, "image");
        assert_eq!(prepared.session_token, "sk-test");
        assert_eq!(prepared.action_body["type"], "create_image_uni_1");
        assert!(prepared.auto_discover_action_type);
        assert_eq!(prepared.locale, "zh-CN");
        assert_eq!(prepared.timeout, Duration::from_secs(90));
    }

    #[test]
    fn prepare_lumalabs_execution_context_reads_session_contract() {
        let payload = make_payload("lumalabs_compatible", "https://app.lumalabs.ai/");
        let req = image_generation_request(serde_json::json!({
            "prompt": "surreal glass flower"
        }));

        let prepared = prepare_lumalabs_execution_context(
            &payload,
            &req,
            crate::protocol::lumalabs::LUMALABS_DEFAULT_IMAGE_MODEL,
            Duration::from_secs(30),
        )
        .expect("lumalabs execution context");

        assert_eq!(prepared.browser_input.session_token, "sk-test");
        assert_eq!(prepared.media_plan.prompt, "surreal glass flower");
        assert_eq!(prepared.browser_input.base_url, "https://app.lumalabs.ai");
        assert_eq!(prepared.browser_input.realm_id, "realm-123");
        assert_eq!(prepared.browser_input.timeout, Duration::from_secs(90));
    }

    #[test]
    fn prepare_lumalabs_execution_context_preserves_image_input_rejection_contract() {
        let payload = make_payload("lumalabs_compatible", "https://app.lumalabs.ai");
        let req = image_generation_request(serde_json::json!({
            "prompt": "edit this",
            "image": "data:image/png;base64,AAAA"
        }));

        let error = prepare_lumalabs_execution_context(
            &payload,
            &req,
            crate::protocol::lumalabs::LUMALABS_DEFAULT_IMAGE_MODEL,
            Duration::from_secs(30),
        )
        .expect_err("image inputs should fail");

        assert_eq!(
            error.code.as_deref(),
            Some("unsupported_lumalabs_image_inputs")
        );
        assert_eq!(error.provider_name.as_deref(), Some("lumalabs_compatible"));
    }

    #[test]
    fn classify_lumalabs_browser_worker_failure_prefers_worker_error_contract() {
        let result = parse_lumalabs_browser_worker_output(
            "{\"ok\":false,\"error\":{\"code\":\"lumalabs_browser_failed\",\"message\":\"challenge required\",\"status\":422,\"body\":\"{\\\"detail\\\":\\\"Unauthorized\\\"}\"}}",
            "",
        )
        .expect("lumalabs worker output");

        let error = classify_lumalabs_browser_worker_failure(result, "", "lumalabs_compatible");
        assert_eq!(error.provider_name.as_deref(), Some("lumalabs_compatible"));
        assert_eq!(error.code.as_deref(), Some("lumalabs_browser_failed"));
        assert_eq!(error.message, "challenge required");
        assert_eq!(error.http_status, Some(422));
    }

    #[test]
    fn classify_lumalabs_browser_worker_failure_uses_stderr_when_body_missing() {
        let result = parse_lumalabs_browser_worker_output("{\"ok\":false,\"error\":{}}", "")
            .expect("lumalabs worker output");

        let error = classify_lumalabs_browser_worker_failure(
            result,
            "permission denied",
            "lumalabs_compatible",
        );
        assert_eq!(error.provider_name.as_deref(), Some("lumalabs_compatible"));
        assert_eq!(error.http_status, Some(500));
        assert!(error.message.contains("permission denied"));
    }

    #[test]
    fn extract_lumalabs_browser_worker_success_reads_signed_url_contract() {
        let result = parse_lumalabs_browser_worker_output(
            "{\"ok\":true,\"signedUrl\":\"https://cdn.example.com/file.mp4\"}",
            "",
        )
        .expect("lumalabs worker output");

        let signed_url =
            extract_lumalabs_browser_worker_success(result).expect("lumalabs worker success");
        assert_eq!(signed_url, "https://cdn.example.com/file.mp4");
    }

    #[test]
    fn extract_lumalabs_browser_worker_success_rejects_missing_signed_url_contract() {
        let result =
            parse_lumalabs_browser_worker_output("{\"ok\":true}", "").expect("worker output");

        let error = extract_lumalabs_browser_worker_success(result)
            .expect_err("missing signed url should fail");
        assert_eq!(
            error.code.as_deref(),
            Some("lumalabs_browser_worker_missing_signed_url")
        );
        assert_eq!(error.provider_name.as_deref(), Some("lumalabs_compatible"));
    }

    #[test]
    fn resolve_lumalabs_browser_worker_result_reads_success_contract() {
        let result = parse_lumalabs_browser_worker_output(
            "{\"ok\":true,\"signedUrl\":\"https://cdn.example.com/file.mp4\"}",
            "",
        )
        .expect("lumalabs worker output");

        let signed_url = resolve_lumalabs_browser_worker_result(result, "", "lumalabs_compatible")
            .expect("ok worker result should succeed");
        assert_eq!(signed_url, "https://cdn.example.com/file.mp4");
    }

    #[test]
    fn resolve_lumalabs_browser_worker_result_preserves_failure_contract() {
        let result = parse_lumalabs_browser_worker_output(
            "{\"ok\":false,\"error\":{\"code\":\"lumalabs_browser_failed\",\"message\":\"challenge required\",\"status\":422,\"body\":\"{\\\"detail\\\":\\\"Unauthorized\\\"}\"}}",
            "",
        )
        .expect("lumalabs worker output");

        let error = resolve_lumalabs_browser_worker_result(result, "", "lumalabs_compatible")
            .expect_err("failed worker result should error");

        assert_eq!(error.provider_name.as_deref(), Some("lumalabs_compatible"));
        assert_eq!(error.code.as_deref(), Some("lumalabs_browser_failed"));
        assert_eq!(error.message, "challenge required");
        assert_eq!(error.http_status, Some(422));
    }

    #[test]
    fn parse_lumalabs_browser_worker_verified_output_reads_success_contract() {
        let signed_url = parse_lumalabs_browser_worker_verified_output(
            "{\"ok\":true,\"signedUrl\":\"https://cdn.example.com/file.mp4\"}",
            "",
            "lumalabs_compatible",
        )
        .expect("verified worker output");

        assert_eq!(signed_url, "https://cdn.example.com/file.mp4");
    }

    #[test]
    fn parse_lumalabs_browser_worker_verified_output_preserves_failure_contract() {
        let error = parse_lumalabs_browser_worker_verified_output(
            "{\"ok\":false,\"error\":{\"code\":\"lumalabs_browser_failed\",\"message\":\"challenge required\",\"status\":422,\"body\":\"{\\\"detail\\\":\\\"Unauthorized\\\"}\"}}",
            "",
            "lumalabs_compatible",
        )
        .expect_err("failed worker output should error");

        assert_eq!(error.provider_name.as_deref(), Some("lumalabs_compatible"));
        assert_eq!(error.code.as_deref(), Some("lumalabs_browser_failed"));
        assert_eq!(error.message, "challenge required");
        assert_eq!(error.http_status, Some(422));
    }

    #[test]
    fn parse_lumalabs_remote_browser_executor_signed_url_reads_contract() {
        let signed_url = parse_lumalabs_remote_browser_executor_signed_url(&serde_json::json!({
            "signedUrl": "https://cdn.example.com/out.png?sig=test"
        }))
        .expect("signed url should be extracted");

        assert_eq!(signed_url, "https://cdn.example.com/out.png?sig=test");
    }

    #[test]
    fn parse_lumalabs_remote_browser_executor_signed_url_preserves_missing_contract() {
        let error = parse_lumalabs_remote_browser_executor_signed_url(&serde_json::json!({
            "ok": true
        }))
        .expect_err("missing signedUrl should fail");

        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.code.as_deref(),
            Some("lumalabs_browser_executor_missing_signed_url")
        );
        assert_eq!(error.provider_name.as_deref(), Some("lumalabs_compatible"));
    }

    #[test]
    fn build_lumalabs_browser_executor_service_result_wraps_signed_url_contract() {
        let body =
            build_lumalabs_browser_executor_service_result("https://cdn.example.com/file.mp4");

        assert_eq!(body["signedUrl"], "https://cdn.example.com/file.mp4");
    }

    #[test]
    fn build_lumalabs_browser_executor_service_result_preserves_empty_string_contract() {
        let body = build_lumalabs_browser_executor_service_result("");

        assert_eq!(body["signedUrl"], "");
    }

    #[test]
    fn classify_lumalabs_media_fetch_error_preserves_generic_upstream_contract() {
        let error = classify_lumalabs_media_fetch_error(503, "service unavailable");

        assert_eq!(error.provider_name.as_deref(), Some("lumalabs_compatible"));
        assert_eq!(error.http_status, Some(503));
        assert!(error.message.contains("service unavailable"));
    }

    #[test]
    fn ensure_successful_lumalabs_media_fetch_status_accepts_2xx_contract() {
        ensure_successful_lumalabs_media_fetch_status(204, "").expect("2xx should pass");
    }

    #[test]
    fn ensure_successful_lumalabs_media_fetch_status_preserves_failure_contract() {
        let error = ensure_successful_lumalabs_media_fetch_status(503, "service unavailable")
            .expect_err("non-2xx should fail");

        assert_eq!(error.provider_name.as_deref(), Some("lumalabs_compatible"));
        assert_eq!(error.http_status, Some(503));
        assert!(error.message.contains("service unavailable"));
    }

    #[test]
    fn resolve_lumalabs_downloaded_image_mime_type_prefers_image_header_contract() {
        let mut headers = rquest::header::HeaderMap::new();
        headers.insert(
            rquest::header::CONTENT_TYPE,
            rquest::header::HeaderValue::from_static("image/webp"),
        );

        let mime_type = resolve_lumalabs_downloaded_image_mime_type(
            &headers,
            "https://cdn.example.com/render.png",
        );

        assert_eq!(mime_type, "image/webp");
    }

    #[test]
    fn resolve_lumalabs_downloaded_image_mime_type_falls_back_to_signed_url_contract() {
        let headers = rquest::header::HeaderMap::new();

        let mime_type = resolve_lumalabs_downloaded_image_mime_type(
            &headers,
            "https://cdn.example.com/render.jpg",
        );

        assert_eq!(mime_type, "image/jpeg");
    }

    #[test]
    fn resolve_lumalabs_image_generation_plan_preserves_url_response_contract() {
        let req = image_generation_request(serde_json::json!({}));

        let response = resolve_lumalabs_image_generation_plan(
            &req,
            "crystal portrait",
            "https://cdn.example.com/luma.png",
        )
        .expect("image plan")
        .expect("url response");

        assert_eq!(
            response["data"][0]["url"],
            "https://cdn.example.com/luma.png"
        );
        assert_eq!(response["data"][0]["revised_prompt"], "crystal portrait");
    }

    #[test]
    fn resolve_lumalabs_image_generation_plan_preserves_b64_contract() {
        let req = image_generation_request(serde_json::json!({ "response_format": "b64_json" }));

        let response = resolve_lumalabs_image_generation_plan(
            &req,
            "crystal portrait",
            "https://cdn.example.com/luma.png",
        )
        .expect("image plan");

        assert!(response.is_none());
    }

    #[test]
    fn build_lumalabs_non_image_generation_response_preserves_video_contract() {
        let body = build_lumalabs_non_image_generation_response(
            crate::protocol::lumalabs::LumalabsMediaOperation::Video,
            "ray3.14",
            "ocean flythrough",
            "https://cdn.example.com/video.mp4",
        );

        assert_eq!(body["object"], "video.generation");
        assert_eq!(body["model"], "ray3.14");
        assert_eq!(body["prompt"], "ocean flythrough");
        assert_eq!(body["data"][0]["url"], "https://cdn.example.com/video.mp4");
    }

    #[test]
    fn build_lumalabs_non_image_generation_response_preserves_audio_contract() {
        let body = build_lumalabs_non_image_generation_response(
            crate::protocol::lumalabs::LumalabsMediaOperation::Audio,
            "elevenlabs-music-v1",
            "ambient pulse",
            "https://cdn.example.com/audio.mp3",
        );

        assert_eq!(body["object"], "audio.generation");
        assert_eq!(body["model"], "elevenlabs-music-v1");
        assert_eq!(body["prompt"], "ambient pulse");
        assert_eq!(body["data"][0]["url"], "https://cdn.example.com/audio.mp3");
    }

    #[test]
    fn build_lumalabs_downloaded_image_response_preserves_header_mime_contract() {
        let mut headers = rquest::header::HeaderMap::new();
        headers.insert(
            rquest::header::CONTENT_TYPE,
            rquest::header::HeaderValue::from_static("image/webp"),
        );

        let body = build_lumalabs_downloaded_image_response(
            "luma portrait",
            "https://cdn.example.com/render.png",
            &headers,
            b"png-bytes",
        );

        assert_eq!(body["data"][0]["revised_prompt"], "luma portrait");
        assert_eq!(body["data"][0]["mime_type"], "image/webp");
        assert!(body["data"][0]["b64_json"].as_str().is_some());
    }

    #[test]
    fn build_lumalabs_downloaded_image_response_falls_back_to_signed_url_contract() {
        let headers = rquest::header::HeaderMap::new();

        let body = build_lumalabs_downloaded_image_response(
            "luma portrait",
            "https://cdn.example.com/render.jpg",
            &headers,
            b"jpg-bytes",
        );

        assert_eq!(body["data"][0]["mime_type"], "image/jpeg");
    }
}
