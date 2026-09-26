//! Rebuild captured request envelopes while preserving form and query ordering.

use std::collections::HashMap;

use super::GeminiCanvasProgramAppInvokeContract;
use crate::error::GatewayError;
use crate::protocol::gemini::web_reverse as gemini_web;
use crate::protocol::gemini_canvas;

fn current_reqid() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64 % 1_000_000)
        .unwrap_or(1)
}

fn parse_form_urlencoded_pairs(input: &str) -> Vec<(String, String)> {
    url::form_urlencoded::parse(input.as_bytes())
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect()
}

fn serialize_form_urlencoded_pairs(pairs: &[(String, String)]) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in pairs {
        serializer.append_pair(key, value);
    }
    serializer.finish()
}

fn upsert_pair(pairs: &mut Vec<(String, String)>, key: &str, value: String) {
    if let Some(entry) = pairs.iter_mut().find(|(entry_key, _)| entry_key == key) {
        entry.1 = value;
    } else {
        pairs.push((key.to_string(), value));
    }
}

fn replace_contract_prompt(
    form: &mut Vec<(String, String)>,
    original_prompt: Option<&str>,
    requested_prompt: Option<&str>,
) {
    let Some(original_prompt) = original_prompt
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let Some(requested_prompt) = requested_prompt
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    if requested_prompt == original_prompt {
        return;
    }
    for (_, value) in form.iter_mut() {
        if value.contains(original_prompt) {
            *value = value.replacen(original_prompt, requested_prompt, 1);
        }
    }
}

pub fn build_program_batchexecute_request_from_invoke_contract(
    contract: &GeminiCanvasProgramAppInvokeContract,
    bootstrap: &gemini_web::GeminiWebBootstrap,
    locale: &str,
    requested_prompt: Option<&str>,
) -> Result<Option<gemini_web::GeminiWebRequest>, GatewayError> {
    let Some(request_url) = contract
        .request_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let Some(request_body) = contract
        .request_body
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let parsed_url = url::Url::parse(request_url).map_err(|error| {
        GatewayError::server_error(format!(
            "parse Gemini Canvas program batchexecute requestUrl failed: {error}"
        ))
        .with_provider("gemini_canvas_program_web_reverse_compatible")
        .with_code("gemini_canvas_program_batchexecute_request_url_invalid")
    })?;
    let mut query: Vec<(String, String)> = parsed_url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    let mut form = parse_form_urlencoded_pairs(request_body);

    if let Some(build_label) = bootstrap.build_label.as_deref() {
        upsert_pair(&mut query, "bl", build_label.trim().to_string());
    }
    if let Some(session_id) = bootstrap.session_id.as_deref() {
        upsert_pair(&mut query, "f.sid", session_id.trim().to_string());
    }
    upsert_pair(&mut query, "hl", locale.trim().to_string());
    upsert_pair(&mut query, "_reqid", current_reqid().to_string());
    upsert_pair(&mut query, "rt", "c".to_string());

    if let Some(source_path) = contract
        .source_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        upsert_pair(&mut query, "source-path", source_path.to_string());
    }

    if let Some(access_token) = bootstrap.access_token.as_deref() {
        upsert_pair(&mut form, "at", access_token.trim().to_string());
    }
    replace_contract_prompt(&mut form, contract.prompt.as_deref(), requested_prompt);

    Ok(Some(gemini_web::GeminiWebRequest { query, form }))
}

pub fn build_program_stream_generate_request_from_invoke_contract(
    contract: &GeminiCanvasProgramAppInvokeContract,
    requested_prompt: Option<&str>,
) -> Result<Option<gemini_canvas::GeminiCanvasTextStreamGenerateTemplate>, GatewayError> {
    let Some(request_url) = contract
        .request_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    if !request_url.contains("/StreamGenerate") {
        return Ok(None);
    }
    let Some(request_body) = contract
        .request_body
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let parsed_url = url::Url::parse(request_url).map_err(|error| {
        GatewayError::server_error(format!(
            "parse Gemini Canvas program StreamGenerate requestUrl failed: {error}"
        ))
        .with_provider("gemini_canvas_program_web_reverse_compatible")
        .with_code("gemini_canvas_program_stream_generate_request_url_invalid")
    })?;
    let query: Vec<(String, String)> = parsed_url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    let mut form = parse_form_urlencoded_pairs(request_body);
    replace_contract_prompt(&mut form, contract.prompt.as_deref(), requested_prompt);
    let mut template_url = parsed_url.clone();
    template_url.set_query(None);
    template_url.set_fragment(None);
    let mut headers = HashMap::new();
    headers.insert(
        "content-type".to_string(),
        "application/x-www-form-urlencoded;charset=UTF-8".to_string(),
    );
    headers.insert("accept".to_string(), "*/*".to_string());
    Ok(Some(
        gemini_canvas::GeminiCanvasTextStreamGenerateTemplate {
            url: template_url.to_string(),
            query,
            form: form.clone(),
            raw_post_data: serialize_form_urlencoded_pairs(&form),
            headers,
        },
    ))
}
