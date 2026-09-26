use base64::Engine;
use serde_json::{json, Value};
use std::collections::HashSet;

use crate::error::GatewayError;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;

#[path = "gemini_business_errors.rs"]
mod errors;
pub use errors::*;

pub const NANO_BANANA_PRO_MODEL: &str = "nano-banana-pro";
pub const GEMINI_3_PRO_IMAGE_PREVIEW_MODEL: &str = "gemini-3-pro-image-preview";
const DEFAULT_LANGUAGE_CODE: &str = "en-US";
const DEFAULT_TIME_ZONE: &str = "UTC";
const DEFAULT_ANSWER_GENERATION_MODE: &str = "NORMAL";
const DEFAULT_ASSIST_SKIPPING_MODE: &str = "REQUEST_ASSIST";
const DEFAULT_STREAM_ASSIST_PATH: &str = "/locations/global/widgetStreamAssist";
const DEFAULT_CONTEXT_FILE_UPLOAD_PATH: &str = "/locations/global/widgetAddContextFile";

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
#[path = "gemini_business_error_tests.rs"]
mod error_tests;
#[cfg(test)]
#[path = "gemini_business_normalization_tests.rs"]
mod normalization_tests;
#[cfg(test)]
#[path = "gemini_business_response_tests.rs"]
mod response_tests;
#[cfg(test)]
#[path = "gemini_business_test_support.rs"]
mod test_support;
