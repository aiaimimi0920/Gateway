use serde_json::{json, Value};
use std::collections::HashMap;
use url::Url;

use super::{template_refresh::serialize_stream_generate_form_pairs, types::*};
use crate::error::GatewayError;

pub fn build_text_stream_generate_request_from_template(
    storage_state: &Value,
    prompt: &str,
) -> Result<Option<GeminiCanvasTextStreamGenerateTemplate>, GatewayError> {
    build_stream_generate_request_from_template_keys(
        storage_state,
        &["textStreamGenerateTemplate", "streamGenerateTemplate"],
        prompt,
        None,
        None,
    )
}

pub fn build_image_stream_generate_request_from_template(
    storage_state: &Value,
    prompt: &str,
    request_uuid: &str,
) -> Result<Option<GeminiCanvasTextStreamGenerateTemplate>, GatewayError> {
    build_stream_generate_request_from_template_keys(
        storage_state,
        &["imageStreamGenerateTemplate", "mediaStreamGenerateTemplate"],
        prompt,
        Some(request_uuid),
        None,
    )
}

pub fn build_image_edit_stream_generate_request_from_template(
    storage_state: &Value,
    prompt: &str,
    request_uuid: &str,
    uploaded_files: &[GeminiCanvasUploadedFileRef],
) -> Result<Option<GeminiCanvasTextStreamGenerateTemplate>, GatewayError> {
    build_stream_generate_request_from_template_keys(
        storage_state,
        &["imageEditStreamGenerateTemplate", "imageEditStreamTemplate"],
        prompt,
        Some(request_uuid),
        Some(uploaded_files),
    )
}

pub fn harvest_image_edit_stream_generate_seed(
    storage_state: &Value,
) -> Option<GeminiCanvasStreamGenerateSeed> {
    let template = storage_state
        .get("imageEditStreamGenerateTemplate")
        .or_else(|| storage_state.get("imageEditStreamTemplate"))?;
    let template_obj = template.as_object()?;
    let post_data = template_obj
        .get("postData")
        .or_else(|| template_obj.get("rawPostData"))
        .and_then(Value::as_str)?
        .trim();
    if post_data.is_empty() {
        return None;
    }
    let form = url::form_urlencoded::parse(post_data.as_bytes())
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    let f_req = form.iter().find(|(key, _)| key == "f.req")?.1.as_str();
    let outer = serde_json::from_str::<Vec<Value>>(f_req).ok()?;
    let inner_payload = outer.get(1)?.as_str()?;
    let inner = serde_json::from_str::<Vec<Value>>(inner_payload).ok()?;
    Some(GeminiCanvasStreamGenerateSeed {
        opaque_state: inner
            .get(3)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string),
        request_hex: inner
            .get(4)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string),
        request_uuid: inner
            .get(59)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string),
    })
}

fn build_stream_generate_request_from_template_keys(
    storage_state: &Value,
    template_keys: &[&str],
    prompt: &str,
    request_uuid: Option<&str>,
    uploaded_files: Option<&[GeminiCanvasUploadedFileRef]>,
) -> Result<Option<GeminiCanvasTextStreamGenerateTemplate>, GatewayError> {
    let template = template_keys.iter().find_map(|key| storage_state.get(*key));
    let Some(template) = template else {
        return Ok(None);
    };
    let Some(template_obj) = template.as_object() else {
        return Err(GatewayError::server_error(
            "Gemini Canvas stream template must be a JSON object.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_text_stream_template_invalid"));
    };

    let prompt = prompt.trim();
    if prompt.is_empty() {
        return Err(GatewayError::bad_request(
            "Gemini Canvas StreamGenerate requests require a prompt.",
        )
        .with_code("missing_gemini_canvas_stream_generate_prompt"));
    }

    let url = template_obj
        .get("url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            GatewayError::server_error(
                "Gemini Canvas text stream template did not include a non-empty url.",
            )
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_text_stream_template_invalid")
        })?;
    let parsed_url = Url::parse(url).map_err(|error| {
        GatewayError::server_error(format!(
            "parse Gemini Canvas text stream template url: {error}"
        ))
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_text_stream_template_invalid")
    })?;
    let post_data = template_obj
        .get("postData")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            GatewayError::server_error(
                "Gemini Canvas text stream template did not include non-empty postData.",
            )
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_text_stream_template_invalid")
        })?;
    let headers_obj = template_obj
        .get("headers")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            GatewayError::server_error(
                "Gemini Canvas text stream template did not include a headers object.",
            )
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_text_stream_template_invalid")
        })?;

    let headers = headers_obj
        .iter()
        .filter_map(|(name, value)| {
            value
                .as_str()
                .map(|entry| (name.to_ascii_lowercase(), entry.to_string()))
        })
        .collect::<HashMap<_, _>>();
    let query = parsed_url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    let mut form = url::form_urlencoded::parse(post_data.as_bytes())
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    let Some(f_req_index) = form.iter().position(|(key, _)| key == "f.req") else {
        return Err(GatewayError::server_error(
            "Gemini Canvas text stream template did not include f.req.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_text_stream_template_invalid"));
    };

    let mut outer = serde_json::from_str::<Vec<Value>>(&form[f_req_index].1).map_err(|error| {
        GatewayError::server_error(format!(
            "parse Gemini Canvas text stream template f.req outer payload: {error}"
        ))
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_text_stream_template_invalid")
    })?;
    let Some(inner_payload) = outer.get(1).and_then(Value::as_str) else {
        return Err(GatewayError::server_error(
            "Gemini Canvas text stream template outer payload did not contain the inner payload string.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_text_stream_template_invalid"));
    };
    let mut inner = serde_json::from_str::<Vec<Value>>(inner_payload).map_err(|error| {
        GatewayError::server_error(format!(
            "parse Gemini Canvas text stream template inner payload: {error}"
        ))
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_text_stream_template_invalid")
    })?;
    let Some(prompt_parts) = inner.get_mut(0).and_then(Value::as_array_mut) else {
        return Err(GatewayError::server_error(
            "Gemini Canvas text stream template inner payload did not contain the prompt array.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_text_stream_template_invalid"));
    };
    if prompt_parts.is_empty() {
        return Err(GatewayError::server_error(
            "Gemini Canvas text stream template prompt array was empty.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_text_stream_template_invalid"));
    }
    prompt_parts[0] = Value::String(prompt.to_string());
    if let Some(uploaded_files) = uploaded_files {
        while prompt_parts.len() <= 6 {
            prompt_parts.push(Value::Null);
        }
        let attachments = uploaded_files
            .iter()
            .map(|file| {
                json!([
                    [file.resource_path, 1, null, file.mime_type],
                    file.file_name,
                    null,
                    null,
                    null,
                    null,
                    null,
                    null,
                    [0]
                ])
            })
            .collect::<Vec<_>>();
        prompt_parts[3] = Value::Array(attachments);
        prompt_parts[6] = Value::from(0);
        if inner.len() > 6 {
            inner[6] = json!([1]);
        }
        if inner.len() > 68 {
            inner[68] = Value::from(2);
        }
    }
    if let Some(request_uuid) = request_uuid
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if inner.len() > 59 {
            inner[59] = Value::String(request_uuid.to_string());
        }
    }
    outer[1] = Value::String(serde_json::to_string(&inner).map_err(|error| {
        GatewayError::server_error(format!(
            "serialize Gemini Canvas text stream template inner payload: {error}"
        ))
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_text_stream_template_invalid")
    })?);
    form[f_req_index].1 = serde_json::to_string(&outer).map_err(|error| {
        GatewayError::server_error(format!(
            "serialize Gemini Canvas text stream template outer payload: {error}"
        ))
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_text_stream_template_invalid")
    })?;

    let mut headers = headers;
    if let Some(request_uuid) = request_uuid
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Some(existing_header) = headers
            .get("x-goog-ext-525005358-jspb")
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let updated_header = serde_json::from_str::<Vec<Value>>(existing_header)
                .ok()
                .and_then(|mut parsed| {
                    if parsed.is_empty() {
                        return None;
                    }
                    parsed[0] = Value::String(request_uuid.to_string());
                    serde_json::to_string(&parsed).ok()
                })
                .unwrap_or_else(|| format!("[\"{request_uuid}\",1]"));
            headers.insert("x-goog-ext-525005358-jspb".to_string(), updated_header);
        }
    }

    let mut request_url = parsed_url.clone();
    request_url.set_query(None);
    request_url.set_fragment(None);
    let raw_post_data = serialize_stream_generate_form_pairs(&form);

    Ok(Some(GeminiCanvasTextStreamGenerateTemplate {
        url: request_url.to_string(),
        query,
        form,
        raw_post_data,
        headers,
    }))
}
