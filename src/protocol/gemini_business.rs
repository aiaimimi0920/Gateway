use base64::Engine;
use serde_json::{json, Value};
use std::collections::HashSet;

use crate::error::GatewayError;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;

pub const NANO_BANANA_PRO_MODEL: &str = "nano-banana-pro";
pub const GEMINI_3_PRO_IMAGE_PREVIEW_MODEL: &str = "gemini-3-pro-image-preview";
const DEFAULT_LANGUAGE_CODE: &str = "en-US";
const DEFAULT_TIME_ZONE: &str = "UTC";
const DEFAULT_ANSWER_GENERATION_MODE: &str = "NORMAL";
const DEFAULT_ASSIST_SKIPPING_MODE: &str = "REQUEST_ASSIST";
const DEFAULT_STREAM_ASSIST_PATH: &str = "/locations/global/widgetStreamAssist";
const DEFAULT_CONTEXT_FILE_UPLOAD_PATH: &str = "/locations/global/widgetAddContextFile";

pub fn unsupported_request_plan_error() -> GatewayError {
    GatewayError::bad_request(
        "Gemini Business adapters currently support only image generation/edit passthrough endpoints",
    )
    .with_provider("gemini_business_compatible")
    .with_code("unsupported_gemini_business_endpoint")
}

pub fn missing_input_image_error() -> GatewayError {
    GatewayError::bad_request("Image edit requests require at least one input image.")
        .with_code("missing_input_image")
}

pub fn missing_gemini_business_runtime_error() -> GatewayError {
    GatewayError::bad_request(
        "Gemini Business credentials require extra_body runtime material (configId + session).",
    )
    .with_code("missing_gemini_business_runtime")
}

pub fn missing_gemini_business_runtime_field_error(field: &str) -> GatewayError {
    GatewayError::bad_request(format!("Missing required runtime field `{field}`."))
        .with_code("missing_gemini_business_runtime_field")
}

pub fn missing_gemini_business_prompt_error() -> GatewayError {
    GatewayError::bad_request("Gemini Business image requests require a prompt.")
        .with_code("missing_prompt")
}

pub fn invalid_image_response_format_error() -> GatewayError {
    GatewayError::bad_request("response_format must be a string when provided.")
        .with_code("invalid_image_response_format")
}

pub fn unsupported_image_response_format_error() -> GatewayError {
    GatewayError::bad_request(
        "Gemini Business image endpoints currently support response_format=b64_json or url.",
    )
    .with_code("unsupported_image_response_format")
}

pub fn gemini_business_no_images_error() -> GatewayError {
    GatewayError::server_error(
        "Gemini Business image request completed without any generated files.",
    )
    .with_provider("gemini_business_compatible")
    .with_code("gemini_business_no_images")
}

pub fn gemini_business_rate_limited_error(message: &str) -> GatewayError {
    GatewayError::rate_limited(message, 1_000)
        .with_provider("gemini_business_compatible")
        .with_code("gemini_business_rate_limited")
}

pub fn gemini_business_upstream_error(message: &str) -> GatewayError {
    GatewayError::server_error(message)
        .with_provider("gemini_business_compatible")
        .with_code("gemini_business_upstream_error")
}

pub fn invalid_image_upload_error() -> GatewayError {
    GatewayError::bad_request("Gemini Business image uploads must be JSON objects.")
        .with_code("invalid_image_upload")
}

pub fn missing_required_field_error(field: &str) -> GatewayError {
    GatewayError::bad_request(format!("Missing required field `{field}`."))
        .with_code("missing_required_field")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiBusinessUpload {
    pub mime_type: String,
    pub base64_data: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiBusinessGeneratedFile {
    pub file_id: String,
    pub mime_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiBusinessRuntime {
    pub config_id: String,
    pub session: String,
    pub language_code: String,
    pub time_zone: String,
    pub answer_generation_mode: String,
    pub assist_skipping_mode: String,
    pub additional_token: String,
    pub stream_assist_path: String,
    pub context_file_upload_path: String,
}

pub fn resolve_image_model(model: &str) -> &str {
    match model {
        NANO_BANANA_PRO_MODEL => GEMINI_3_PRO_IMAGE_PREVIEW_MODEL,
        _ => model,
    }
}

pub fn normalize_image_generations(body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    let prompt = read_required_string(&body, &["prompt"])?;
    let model = read_optional_string(&body, &["model"])
        .unwrap_or_else(|| NANO_BANANA_PRO_MODEL.to_string());

    Ok(CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ImagesGenerations,
        requested_model: Some(model),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: prompt.to_string(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: body,
        previous_response_id: None,
        explicit_session_key: None,
        extra: std::collections::HashMap::new(),
    })
}

pub fn normalize_image_edits(body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    let prompt = read_required_string(&body, &["prompt"])?;
    let model = read_optional_string(&body, &["model"])
        .unwrap_or_else(|| NANO_BANANA_PRO_MODEL.to_string());
    let uploads = extract_uploads_from_request_body(&body)?;
    if uploads.is_empty() {
        return Err(missing_input_image_error());
    }

    let mut content = uploads
        .iter()
        .map(|upload| ContentPart::ImageUrl {
            image_url: format!("data:{};base64,{}", upload.mime_type, upload.base64_data),
            detail: None,
        })
        .collect::<Vec<_>>();
    content.push(ContentPart::Text {
        text: prompt.to_string(),
    });

    Ok(CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ImagesEdits,
        requested_model: Some(model),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content,
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: body,
        previous_response_id: None,
        explicit_session_key: None,
        extra: std::collections::HashMap::new(),
    })
}

pub fn extract_uploads_from_request_body(
    body: &Value,
) -> Result<Vec<GeminiBusinessUpload>, GatewayError> {
    let mut uploads = Vec::new();

    if let Some(images) = body.get("images").and_then(|v| v.as_array()) {
        for image in images {
            uploads.push(parse_upload_value(image)?);
        }
    }

    if let Some(mask) = body.get("mask") {
        uploads.push(parse_upload_value(mask)?);
    }

    Ok(uploads)
}

pub fn runtime_from_payload(
    payload: &ProviderAccountPayload,
) -> Result<GeminiBusinessRuntime, GatewayError> {
    let extra = payload
        .extra_body
        .as_ref()
        .ok_or_else(missing_gemini_business_runtime_error)?;

    let config_id = read_required_hash_string(extra, &["configId", "config_id"])?;
    let session = read_required_hash_string(
        extra,
        &[
            "session",
            "upstreamSessionId",
            "upstream_session_id",
            "widgetSession",
            "widget_session",
        ],
    )?;

    Ok(GeminiBusinessRuntime {
        config_id,
        session,
        language_code: read_optional_hash_string(extra, &["languageCode", "language_code"])
            .unwrap_or_else(|| DEFAULT_LANGUAGE_CODE.to_string()),
        time_zone: read_optional_hash_string(extra, &["timeZone", "time_zone"])
            .unwrap_or_else(|| DEFAULT_TIME_ZONE.to_string()),
        answer_generation_mode: read_optional_hash_string(
            extra,
            &["answerGenerationMode", "answer_generation_mode"],
        )
        .unwrap_or_else(|| DEFAULT_ANSWER_GENERATION_MODE.to_string()),
        assist_skipping_mode: read_optional_hash_string(
            extra,
            &["assistSkippingMode", "assist_skipping_mode"],
        )
        .unwrap_or_else(|| DEFAULT_ASSIST_SKIPPING_MODE.to_string()),
        additional_token: read_optional_hash_string(
            extra,
            &["additionalToken", "additional_token"],
        )
        .unwrap_or_else(|| "-".to_string()),
        stream_assist_path: payload
            .chat_completions_path
            .clone()
            .unwrap_or_else(|| DEFAULT_STREAM_ASSIST_PATH.to_string()),
        context_file_upload_path: payload
            .responses_path
            .clone()
            .unwrap_or_else(|| DEFAULT_CONTEXT_FILE_UPLOAD_PATH.to_string()),
    })
}

pub fn build_stream_assist_request(
    req: &CanonicalRelayRequest,
    model: &str,
    runtime: &GeminiBusinessRuntime,
    file_ids: &[String],
) -> Result<Value, GatewayError> {
    let prompt = prompt_from_request(req)?;
    Ok(json!({
        "configId": runtime.config_id,
        "additionalParams": {
            "token": runtime.additional_token,
        },
        "streamAssistRequest": {
            "session": runtime.session,
            "query": {
                "parts": [{
                    "text": prompt,
                }],
            },
            "filter": "",
            "fileIds": file_ids,
            "answerGenerationMode": runtime.answer_generation_mode,
            "toolsSpec": {
                "imageGenerationSpec": {},
            },
            "languageCode": runtime.language_code,
            "userMetadata": {
                "timeZone": runtime.time_zone,
            },
            "assistSkippingMode": runtime.assist_skipping_mode,
            "assistGenerationConfig": {
                "modelId": resolve_image_model(model),
            },
        },
    }))
}

pub fn build_context_file_upload_request(
    runtime: &GeminiBusinessRuntime,
    upload: &GeminiBusinessUpload,
) -> Value {
    json!({
        "configId": runtime.config_id,
        "additionalParams": {
            "token": runtime.additional_token,
        },
        "addContextFileRequest": {
            "name": runtime.session,
            "fileName": build_upload_file_name(&upload.mime_type),
            "mimeType": upload.mime_type,
            "fileContents": upload.base64_data,
        },
    })
}

pub fn build_context_file_upload_plan(
    payload: &ProviderAccountPayload,
    runtime: &GeminiBusinessRuntime,
    upload: &GeminiBusinessUpload,
    response_kind: EndpointKind,
) -> RequestPlan {
    RequestPlan {
        method: rquest::Method::POST,
        url: format!(
            "{}{}",
            payload.base_url.trim_end_matches('/'),
            runtime.context_file_upload_path
        ),
        query: Vec::new(),
        body: Some(build_context_file_upload_request(runtime, upload)),
        response_kind,
    }
}

pub fn build_stream_assist_plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    runtime: &GeminiBusinessRuntime,
    file_ids: &[String],
) -> Result<RequestPlan, GatewayError> {
    Ok(RequestPlan {
        method: rquest::Method::POST,
        url: format!(
            "{}{}",
            payload.base_url.trim_end_matches('/'),
            runtime.stream_assist_path
        ),
        query: Vec::new(),
        body: Some(build_stream_assist_request(req, model, runtime, file_ids)?),
        response_kind: req.endpoint_kind,
    })
}

pub fn prompt_from_request(req: &CanonicalRelayRequest) -> Result<String, GatewayError> {
    if let Some(prompt) = read_optional_string(&req.raw_body, &["prompt"]) {
        if !prompt.is_empty() {
            return Ok(prompt);
        }
    }

    let prompt = req.messages_text().trim().to_string();
    if prompt.is_empty() {
        return Err(missing_gemini_business_prompt_error());
    }
    Ok(prompt)
}

pub fn requested_image_count(req: &CanonicalRelayRequest) -> usize {
    req.raw_body
        .get("n")
        .and_then(|v| v.as_u64())
        .map(|value| value.max(1) as usize)
        .unwrap_or(1)
}

pub fn response_format(req: &CanonicalRelayRequest) -> Result<Option<&str>, GatewayError> {
    let Some(value) = req.raw_body.get("response_format") else {
        return Ok(None);
    };
    let Some(format) = value.as_str() else {
        return Err(invalid_image_response_format_error());
    };
    if format == "b64_json" || format == "url" {
        Ok(Some(format))
    } else {
        Err(unsupported_image_response_format_error())
    }
}

pub fn extract_generated_files(
    response_objects: &[Value],
    default_session: &str,
) -> Result<(String, Vec<GeminiBusinessGeneratedFile>), GatewayError> {
    let mut session_name = default_session.to_string();
    let mut seen = HashSet::new();
    let mut files = Vec::new();

    for entry in response_objects {
        if let Some(error) = entry.get("error") {
            let message = error
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Gemini Business upstream returned an error.");
            let code = error.get("code").and_then(|v| v.as_i64()).unwrap_or(500);
            return Err(if code == 429 {
                gemini_business_rate_limited_error(message)
            } else {
                gemini_business_upstream_error(message)
            });
        }

        let Some(stream_response) = entry.get("streamAssistResponse") else {
            continue;
        };

        if let Some(session) = stream_response
            .get("sessionInfo")
            .and_then(|v| v.get("session"))
            .and_then(|v| v.as_str())
        {
            session_name = session.to_string();
        }

        let replies = stream_response
            .get("answer")
            .and_then(|v| v.get("replies"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        for reply in replies {
            let Some(file) = reply
                .get("groundedContent")
                .and_then(|v| v.get("content"))
                .and_then(|v| v.get("file"))
            else {
                continue;
            };

            let Some(file_id) = file.get("fileId").and_then(|v| v.as_str()) else {
                continue;
            };

            if !seen.insert(file_id.to_string()) {
                continue;
            }

            let mime_type = file
                .get("mimeType")
                .and_then(|v| v.as_str())
                .unwrap_or("image/png");

            files.push(GeminiBusinessGeneratedFile {
                file_id: file_id.to_string(),
                mime_type: mime_type.to_string(),
            });
        }
    }

    if files.is_empty() {
        return Err(gemini_business_no_images_error());
    }

    Ok((session_name, files))
}

pub fn build_openai_images_response(
    req: &CanonicalRelayRequest,
    prompt: &str,
    images: &[(String, Vec<u8>)],
) -> Result<Value, GatewayError> {
    let format = response_format(req)?;
    let max_images = requested_image_count(req);
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut data = Vec::new();
    for (mime_type, bytes) in images.iter().take(max_images) {
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        let entry = match format {
            Some("url") => json!({
                "url": format!("data:{};base64,{}", mime_type, encoded),
                "revised_prompt": prompt,
            }),
            _ => json!({
                "b64_json": encoded,
                "mime_type": mime_type,
                "revised_prompt": prompt,
            }),
        };
        data.push(entry);
    }

    Ok(json!({
        "created": created,
        "data": data,
    }))
}

fn parse_upload_value(value: &Value) -> Result<GeminiBusinessUpload, GatewayError> {
    let Some(record) = value.as_object() else {
        return Err(invalid_image_upload_error());
    };

    let mime_type = read_required_map_string(record, &["mimeType", "mime_type"])?;
    let base64_data = read_required_map_string(record, &["base64", "data"])?;

    Ok(GeminiBusinessUpload {
        mime_type: mime_type.to_string(),
        base64_data: base64_data.to_string(),
    })
}

fn read_required_string(value: &Value, keys: &[&str]) -> Result<String, GatewayError> {
    read_optional_string(value, keys).ok_or_else(|| missing_required_field_error(keys[0]))
}

fn read_optional_string(value: &Value, keys: &[&str]) -> Option<String> {
    let map = value.as_object()?;
    read_optional_map_string(map, keys)
}

fn read_required_map_string<'a>(
    map: &'a serde_json::Map<String, Value>,
    keys: &[&str],
) -> Result<&'a str, GatewayError> {
    for key in keys {
        if let Some(value) = map.get(*key).and_then(|v| v.as_str()) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed);
            }
        }
    }

    Err(missing_gemini_business_runtime_field_error(keys[0]))
}

fn read_optional_map_string(map: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(value) = map.get(*key).and_then(|v| v.as_str()) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn read_required_hash_string(
    map: &std::collections::HashMap<String, Value>,
    keys: &[&str],
) -> Result<String, GatewayError> {
    read_optional_hash_string(map, keys)
        .ok_or_else(|| missing_gemini_business_runtime_field_error(keys[0]))
}

fn read_optional_hash_string(
    map: &std::collections::HashMap<String, Value>,
    keys: &[&str],
) -> Option<String> {
    for key in keys {
        if let Some(value) = map.get(*key).and_then(|v| v.as_str()) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn build_upload_file_name(mime_type: &str) -> String {
    let ext = match mime_type {
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/png" => "png",
        _ => "bin",
    };
    format!("upload-{}.{}", uuid::Uuid::new_v4(), ext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use crate::upstream::client::UpstreamClient;
    use rquest::Method;
    use serde_json::json;

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

    fn make_runtime() -> GeminiBusinessRuntime {
        GeminiBusinessRuntime {
            config_id: "cfg-123".to_string(),
            session: "projects/demo/sessions/abc".to_string(),
            language_code: "en-US".to_string(),
            time_zone: "UTC".to_string(),
            answer_generation_mode: "NORMAL".to_string(),
            assist_skipping_mode: "REQUEST_ASSIST".to_string(),
            additional_token: "-".to_string(),
            stream_assist_path: "/locations/global/widgetStreamAssist".to_string(),
            context_file_upload_path: "/locations/global/widgetAddContextFile".to_string(),
        }
    }

    fn make_upload() -> GeminiBusinessUpload {
        GeminiBusinessUpload {
            mime_type: "image/png".to_string(),
            base64_data: "aGVsbG8=".to_string(),
        }
    }

    fn assert_request_plan(
        plan: &RequestPlan,
        expected_url: &str,
        expected_response_kind: EndpointKind,
    ) {
        assert_eq!(plan.method, Method::POST);
        assert_eq!(plan.url, expected_url);
        assert_eq!(plan.response_kind, expected_response_kind);
        assert!(plan.query.is_empty());
    }

    #[test]
    fn missing_gemini_business_runtime_error_matches_contract() {
        let error = missing_gemini_business_runtime_error();
        assert_eq!(error.http_status, Some(400));
        assert_eq!(
            error.code.as_deref(),
            Some("missing_gemini_business_runtime")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Business credentials require extra_body runtime material (configId + session)."
        );
    }

    #[test]
    fn missing_gemini_business_runtime_field_error_matches_contract() {
        let error = missing_gemini_business_runtime_field_error("configId");
        assert_eq!(error.http_status, Some(400));
        assert_eq!(
            error.code.as_deref(),
            Some("missing_gemini_business_runtime_field")
        );
        assert_eq!(
            error.message.as_str(),
            "Missing required runtime field `configId`."
        );
    }

    #[test]
    fn gemini_business_no_images_error_matches_contract() {
        let error = gemini_business_no_images_error();
        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_business_compatible")
        );
        assert_eq!(error.code.as_deref(), Some("gemini_business_no_images"));
        assert_eq!(
            error.message.as_str(),
            "Gemini Business image request completed without any generated files."
        );
    }

    #[test]
    fn invalid_image_upload_error_matches_contract() {
        let error = invalid_image_upload_error();
        assert_eq!(error.http_status, Some(400));
        assert_eq!(error.code.as_deref(), Some("invalid_image_upload"));
        assert_eq!(
            error.message.as_str(),
            "Gemini Business image uploads must be JSON objects."
        );
    }

    #[test]
    fn missing_required_field_error_matches_contract() {
        let error = missing_required_field_error("prompt");
        assert_eq!(error.http_status, Some(400));
        assert_eq!(error.code.as_deref(), Some("missing_required_field"));
        assert_eq!(error.message.as_str(), "Missing required field `prompt`.");
    }

    #[test]
    fn gemini_business_rate_limited_error_matches_contract() {
        let error = gemini_business_rate_limited_error("too many requests");
        assert_eq!(error.http_status, Some(429));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_business_compatible")
        );
        assert_eq!(error.code.as_deref(), Some("gemini_business_rate_limited"));
        assert_eq!(error.message.as_str(), "too many requests");
    }

    #[test]
    fn gemini_business_upstream_error_matches_contract() {
        let error = gemini_business_upstream_error("internal upstream failure");
        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_business_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_business_upstream_error")
        );
        assert_eq!(error.message.as_str(), "internal upstream failure");
    }

    #[test]
    fn gemini_business_unsupported_request_plan_error_matches_contract() {
        let error = unsupported_request_plan_error();
        assert_eq!(error.http_status, Some(400));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_business_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("unsupported_gemini_business_endpoint")
        );
    }

    #[test]
    fn plan_gemini_business_chat_endpoint_rejected_locally() {
        let payload = make_payload(
            "gemini_business_compatible",
            "https://biz-discoveryengine.googleapis.com/v1alpha",
        );
        let req = make_request(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
        let err = UpstreamClient::build_request_plan(&payload, &req, "nano-banana-pro", false)
            .expect_err("gemini business chat requests should be rejected");
        assert_eq!(err.http_status, Some(400));
        assert_eq!(
            err.code.as_deref(),
            Some("unsupported_gemini_business_endpoint")
        );
    }

    #[test]
    fn missing_gemini_business_prompt_error_matches_contract() {
        let error = missing_gemini_business_prompt_error();
        assert_eq!(error.http_status, Some(400));
        assert_eq!(error.code.as_deref(), Some("missing_prompt"));
        assert_eq!(
            error.message.as_str(),
            "Gemini Business image requests require a prompt."
        );
    }

    #[test]
    fn invalid_image_response_format_error_matches_contract() {
        let error = invalid_image_response_format_error();
        assert_eq!(error.http_status, Some(400));
        assert_eq!(error.code.as_deref(), Some("invalid_image_response_format"));
        assert_eq!(
            error.message.as_str(),
            "response_format must be a string when provided."
        );
    }

    #[test]
    fn unsupported_image_response_format_error_matches_contract() {
        let error = unsupported_image_response_format_error();
        assert_eq!(error.http_status, Some(400));
        assert_eq!(
            error.code.as_deref(),
            Some("unsupported_image_response_format")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Business image endpoints currently support response_format=b64_json or url."
        );
    }

    #[test]
    fn missing_input_image_error_matches_contract() {
        let error = missing_input_image_error();
        assert_eq!(error.http_status, Some(400));
        assert_eq!(error.code.as_deref(), Some("missing_input_image"));
        assert_eq!(
            error.message.as_str(),
            "Image edit requests require at least one input image."
        );
    }

    #[test]
    fn normalize_image_generations_uses_virtual_model_default() {
        let req = normalize_image_generations(json!({
            "prompt": "draw a tiny banana robot"
        }))
        .unwrap();

        assert_eq!(req.endpoint_kind, EndpointKind::ImagesGenerations);
        assert_eq!(req.requested_model.as_deref(), Some(NANO_BANANA_PRO_MODEL));
        assert_eq!(req.messages[0].text_content(), "draw a tiny banana robot");
    }

    #[test]
    fn normalize_image_edits_collects_prompt_and_images() {
        let req = normalize_image_edits(json!({
            "prompt": "turn this into a watercolor poster",
            "model": "nano-banana-pro",
            "images": [{
                "mime_type": "image/png",
                "base64": "abcd"
            }]
        }))
        .unwrap();

        assert_eq!(req.endpoint_kind, EndpointKind::ImagesEdits);
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.messages[0].content.len(), 2);
        assert_eq!(
            req.messages[0].text_content(),
            "turn this into a watercolor poster"
        );
    }

    #[test]
    fn normalize_image_edits_requires_input_image() {
        let error = normalize_image_edits(json!({
            "prompt": "turn this into a watercolor poster"
        }))
        .expect_err("missing image uploads should be rejected");

        assert_eq!(error.http_status, Some(400));
        assert_eq!(error.code.as_deref(), Some("missing_input_image"));
        assert_eq!(
            error.message.as_str(),
            "Image edit requests require at least one input image."
        );
    }

    #[test]
    fn runtime_from_payload_reads_required_runtime_fields() {
        let payload = ProviderAccountPayload {
            adapter: "gemini_business_compatible".to_string(),
            base_url: "https://biz-discoveryengine.googleapis.com/v1alpha".to_string(),
            api_key: "jwt".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: Some(NANO_BANANA_PRO_MODEL.to_string()),
            headers: std::collections::HashMap::new(),
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
            extra_body: Some(
                [
                    ("configId".to_string(), json!("cfg-123")),
                    ("session".to_string(), json!("projects/x/sessions/y")),
                ]
                .into_iter()
                .collect(),
            ),
            session_auth: None,
            keepalive: None,
        };

        let runtime = runtime_from_payload(&payload).unwrap();
        assert_eq!(runtime.config_id, "cfg-123");
        assert_eq!(runtime.session, "projects/x/sessions/y");
        assert_eq!(runtime.language_code, DEFAULT_LANGUAGE_CODE);
    }

    #[test]
    fn build_context_file_upload_plan_uses_upload_endpoint() {
        let payload = make_payload(
            "gemini_business_compatible",
            "https://biz-discoveryengine.googleapis.com/v1alpha",
        );
        let runtime = make_runtime();
        let upload = make_upload();
        let plan = build_context_file_upload_plan(
            &payload,
            &runtime,
            &upload,
            EndpointKind::ImagesGenerations,
        );
        assert_request_plan(
            &plan,
            "https://biz-discoveryengine.googleapis.com/v1alpha/locations/global/widgetAddContextFile",
            EndpointKind::ImagesGenerations,
        );
        let body = plan.body.as_ref().expect("upload plan body");
        assert_eq!(body["configId"], "cfg-123");
        assert_eq!(body["additionalParams"]["token"], "-");
        assert_eq!(
            body["addContextFileRequest"]["name"],
            "projects/demo/sessions/abc"
        );
        assert_eq!(body["addContextFileRequest"]["mimeType"], "image/png");
        assert_eq!(body["addContextFileRequest"]["fileContents"], "aGVsbG8=");
        assert!(
            body["addContextFileRequest"]["fileName"]
                .as_str()
                .is_some_and(|value| value.starts_with("upload-") && value.ends_with(".png")),
            "upload plan should synthesize a file name with png extension"
        );
    }

    #[test]
    fn build_stream_assist_plan_uses_stream_assist_endpoint() {
        let payload = make_payload(
            "gemini_business_compatible",
            "https://biz-discoveryengine.googleapis.com/v1alpha",
        );
        let runtime = make_runtime();
        let mut req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesGenerations);
        req.raw_body = json!({
            "prompt": "tiny watercolor cat astronaut"
        });
        let file_ids = vec!["file-1".to_string(), "file-2".to_string()];
        let plan =
            build_stream_assist_plan(&payload, &req, NANO_BANANA_PRO_MODEL, &runtime, &file_ids)
                .expect("stream assist plan");
        assert_request_plan(
            &plan,
            "https://biz-discoveryengine.googleapis.com/v1alpha/locations/global/widgetStreamAssist",
            EndpointKind::ImagesGenerations,
        );
        let body = plan.body.as_ref().expect("stream assist plan body");
        assert_eq!(body["configId"], "cfg-123");
        assert_eq!(
            body["streamAssistRequest"]["session"],
            "projects/demo/sessions/abc"
        );
        assert_eq!(
            body["streamAssistRequest"]["query"]["parts"][0]["text"],
            "tiny watercolor cat astronaut"
        );
        assert_eq!(
            body["streamAssistRequest"]["fileIds"],
            json!(["file-1", "file-2"])
        );
        assert_eq!(
            body["streamAssistRequest"]["assistGenerationConfig"]["modelId"],
            GEMINI_3_PRO_IMAGE_PREVIEW_MODEL
        );
    }

    #[test]
    fn runtime_from_payload_requires_extra_body_runtime_material() {
        let payload = ProviderAccountPayload {
            adapter: "gemini_business_compatible".to_string(),
            base_url: "https://biz-discoveryengine.googleapis.com/v1alpha".to_string(),
            api_key: "jwt".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: Some(NANO_BANANA_PRO_MODEL.to_string()),
            headers: std::collections::HashMap::new(),
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
        };

        let error = runtime_from_payload(&payload)
            .expect_err("missing extra_body runtime material should fail");
        assert_eq!(error.http_status, Some(400));
        assert_eq!(
            error.code.as_deref(),
            Some("missing_gemini_business_runtime")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Business credentials require extra_body runtime material (configId + session)."
        );
    }

    #[test]
    fn runtime_from_payload_requires_config_id() {
        let payload = ProviderAccountPayload {
            adapter: "gemini_business_compatible".to_string(),
            base_url: "https://biz-discoveryengine.googleapis.com/v1alpha".to_string(),
            api_key: "jwt".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: Some(NANO_BANANA_PRO_MODEL.to_string()),
            headers: std::collections::HashMap::new(),
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
            extra_body: Some(
                [("session".to_string(), json!("projects/x/sessions/y"))]
                    .into_iter()
                    .collect(),
            ),
            session_auth: None,
            keepalive: None,
        };

        let error = runtime_from_payload(&payload).expect_err("missing configId should fail");
        assert_eq!(error.http_status, Some(400));
        assert_eq!(
            error.code.as_deref(),
            Some("missing_gemini_business_runtime_field")
        );
        assert_eq!(
            error.message.as_str(),
            "Missing required runtime field `configId`."
        );
    }

    #[test]
    fn extract_generated_files_requires_generated_files() {
        let error = extract_generated_files(&[json!({})], "projects/demo/sessions/fallback")
            .expect_err("missing generated files should fail");
        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_business_compatible")
        );
        assert_eq!(error.code.as_deref(), Some("gemini_business_no_images"));
        assert_eq!(
            error.message.as_str(),
            "Gemini Business image request completed without any generated files."
        );
    }

    #[test]
    fn extract_generated_files_preserves_rate_limited_contract() {
        let error = extract_generated_files(
            &[json!({
                "error": {
                    "message": "too many requests",
                    "code": 429
                }
            })],
            "projects/demo/sessions/fallback",
        )
        .expect_err("429 error should surface as rate limited");
        assert_eq!(error.http_status, Some(429));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_business_compatible")
        );
        assert_eq!(error.code.as_deref(), Some("gemini_business_rate_limited"));
        assert_eq!(error.message.as_str(), "too many requests");
    }

    #[test]
    fn extract_generated_files_preserves_generic_upstream_error_contract() {
        let error = extract_generated_files(
            &[json!({
                "error": {
                    "message": "internal upstream failure",
                    "code": 500
                }
            })],
            "projects/demo/sessions/fallback",
        )
        .expect_err("non-429 upstream errors should surface as generic upstream errors");
        assert_eq!(error.http_status, Some(500));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_business_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_business_upstream_error")
        );
        assert_eq!(error.message.as_str(), "internal upstream failure");
    }

    #[test]
    fn extract_uploads_from_request_body_rejects_non_object_images() {
        let error = extract_uploads_from_request_body(&json!({
            "images": ["not-an-object"]
        }))
        .expect_err("non-object uploads should fail");
        assert_eq!(error.http_status, Some(400));
        assert_eq!(error.code.as_deref(), Some("invalid_image_upload"));
        assert_eq!(
            error.message.as_str(),
            "Gemini Business image uploads must be JSON objects."
        );
    }

    #[test]
    fn normalize_image_generations_requires_prompt_field() {
        let error =
            normalize_image_generations(json!({})).expect_err("missing prompt field should fail");
        assert_eq!(error.http_status, Some(400));
        assert_eq!(error.code.as_deref(), Some("missing_required_field"));
        assert_eq!(error.message.as_str(), "Missing required field `prompt`.");
    }

    #[test]
    fn prompt_from_request_requires_prompt() {
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ImagesGenerations,
            requested_model: Some(NANO_BANANA_PRO_MODEL.to_string()),
            stream: false,
            messages: Vec::new(),
            tools: Vec::new(),
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: std::collections::HashMap::new(),
        };

        let error = prompt_from_request(&req).expect_err("missing prompt should fail");
        assert_eq!(error.http_status, Some(400));
        assert_eq!(error.code.as_deref(), Some("missing_prompt"));
        assert_eq!(
            error.message.as_str(),
            "Gemini Business image requests require a prompt."
        );
    }

    #[test]
    fn response_format_rejects_non_string_values() {
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ImagesGenerations,
            requested_model: Some(NANO_BANANA_PRO_MODEL.to_string()),
            stream: false,
            messages: Vec::new(),
            tools: Vec::new(),
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({
                "response_format": 123
            }),
            previous_response_id: None,
            explicit_session_key: None,
            extra: std::collections::HashMap::new(),
        };

        let error = response_format(&req).expect_err("non-string response_format should fail");
        assert_eq!(error.http_status, Some(400));
        assert_eq!(error.code.as_deref(), Some("invalid_image_response_format"));
        assert_eq!(
            error.message.as_str(),
            "response_format must be a string when provided."
        );
    }

    #[test]
    fn response_format_rejects_unsupported_string_values() {
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ImagesGenerations,
            requested_model: Some(NANO_BANANA_PRO_MODEL.to_string()),
            stream: false,
            messages: Vec::new(),
            tools: Vec::new(),
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({
                "response_format": "json"
            }),
            previous_response_id: None,
            explicit_session_key: None,
            extra: std::collections::HashMap::new(),
        };

        let error = response_format(&req).expect_err("unsupported response_format should fail");
        assert_eq!(error.http_status, Some(400));
        assert_eq!(
            error.code.as_deref(),
            Some("unsupported_image_response_format")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Business image endpoints currently support response_format=b64_json or url."
        );
    }

    #[test]
    fn extract_generated_files_deduplicates_file_ids() {
        let (session, files) = extract_generated_files(
            &[json!({
                "streamAssistResponse": {
                    "sessionInfo": { "session": "projects/demo/sessions/123" },
                    "answer": {
                        "replies": [
                            { "groundedContent": { "content": { "file": { "fileId": "file-1", "mimeType": "image/png" } } } },
                            { "groundedContent": { "content": { "file": { "fileId": "file-1", "mimeType": "image/png" } } } }
                        ]
                    }
                }
            })],
            "projects/demo/sessions/fallback",
        )
        .unwrap();

        assert_eq!(session, "projects/demo/sessions/123");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].file_id, "file-1");
    }

    #[test]
    fn build_openai_images_response_uses_b64_by_default() {
        let req = normalize_image_generations(json!({
            "prompt": "banana"
        }))
        .unwrap();

        let body = build_openai_images_response(
            &req,
            "banana",
            &[("image/png".to_string(), vec![1, 2, 3])],
        )
        .unwrap();

        assert!(body["data"][0].get("b64_json").is_some());
        assert_eq!(body["data"][0]["mime_type"], "image/png");
    }

    #[test]
    fn resolve_image_model_maps_virtual_alias() {
        assert_eq!(
            resolve_image_model(NANO_BANANA_PRO_MODEL),
            GEMINI_3_PRO_IMAGE_PREVIEW_MODEL
        );
        assert_eq!(
            resolve_image_model(GEMINI_3_PRO_IMAGE_PREVIEW_MODEL),
            GEMINI_3_PRO_IMAGE_PREVIEW_MODEL
        );
    }
}
