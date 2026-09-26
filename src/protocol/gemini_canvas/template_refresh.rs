use serde_json::Value;
use url::Url;

use super::types::GeminiCanvasTextStreamGenerateTemplate;
use crate::error::GatewayError;
use crate::protocol::gemini_web;

pub fn refresh_stream_generate_template_with_bootstrap(
    template: &GeminiCanvasTextStreamGenerateTemplate,
    bootstrap: &gemini_web::GeminiWebBootstrap,
    include_access_token: bool,
) -> Result<GeminiCanvasTextStreamGenerateTemplate, GatewayError> {
    let mut parsed_url = Url::parse(&template.url).map_err(|error| {
        GatewayError::server_error(format!(
            "parse Gemini Canvas stream template url for bootstrap refresh: {error}"
        ))
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_text_stream_template_invalid")
    })?;
    parsed_url.set_query(None);
    parsed_url.set_fragment(None);

    let mut query = template.query.clone();
    upsert_stream_generate_query_param(
        &mut query,
        "bl",
        bootstrap
            .build_label
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty()),
    );
    upsert_stream_generate_query_param(
        &mut query,
        "f.sid",
        bootstrap
            .session_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty()),
    );
    upsert_stream_generate_query_param(&mut query, "hl", Some(bootstrap.language.as_str()));

    let mut form = template.form.clone();
    if include_access_token {
        upsert_stream_generate_form_param(
            &mut form,
            "at",
            bootstrap
                .access_token
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        );
    }
    let raw_post_data = serialize_stream_generate_form_pairs(&form);

    Ok(GeminiCanvasTextStreamGenerateTemplate {
        url: parsed_url.to_string(),
        query,
        form,
        raw_post_data,
        headers: template.headers.clone(),
    })
}

pub fn refresh_stream_generate_template_access_token(
    template: &GeminiCanvasTextStreamGenerateTemplate,
    access_token: &str,
) -> GeminiCanvasTextStreamGenerateTemplate {
    let mut form = template.form.clone();
    upsert_stream_generate_form_param(&mut form, "at", Some(access_token.trim()));
    let raw_post_data = serialize_stream_generate_form_pairs(&form);
    GeminiCanvasTextStreamGenerateTemplate {
        url: template.url.clone(),
        query: template.query.clone(),
        form,
        raw_post_data,
        headers: template.headers.clone(),
    }
}

pub fn refresh_stream_generate_template_model_header_id(
    template: &mut GeminiCanvasTextStreamGenerateTemplate,
    header_id: &str,
) -> bool {
    let header_id = header_id.trim();
    if header_id.is_empty() {
        return false;
    }
    let Some(existing) = template
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("x-goog-ext-525001261-jspb"))
        .map(|(_, value)| value.clone())
    else {
        return false;
    };
    let Ok(mut parsed) = serde_json::from_str::<Vec<Value>>(&existing) else {
        return false;
    };
    if parsed.len() <= 16 {
        return false;
    }
    parsed[16] = Value::String(header_id.to_string());
    let Ok(updated) = serde_json::to_string(&parsed) else {
        return false;
    };
    if let Some((_, value)) = template
        .headers
        .iter_mut()
        .find(|(name, _)| name.eq_ignore_ascii_case("x-goog-ext-525001261-jspb"))
    {
        *value = updated;
        true
    } else {
        false
    }
}

pub fn refresh_stream_generate_template_request_hex(
    template: &mut GeminiCanvasTextStreamGenerateTemplate,
    request_hex: &str,
) -> bool {
    let request_hex = request_hex.trim();
    if request_hex.is_empty() {
        return false;
    }
    if request_hex.len() != 32 || !request_hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return false;
    }
    let Some(f_req_index) = template.form.iter().position(|(key, _)| key == "f.req") else {
        return false;
    };
    let Ok(mut outer) = serde_json::from_str::<Vec<Value>>(&template.form[f_req_index].1) else {
        return false;
    };
    let Some(inner_payload) = outer.get(1).and_then(Value::as_str) else {
        return false;
    };
    let Ok(mut inner) = serde_json::from_str::<Vec<Value>>(inner_payload) else {
        return false;
    };
    if inner.len() <= 4 {
        return false;
    }
    inner[4] = Value::String(request_hex.to_string());
    let Ok(inner_json) = serde_json::to_string(&inner) else {
        return false;
    };
    outer[1] = Value::String(inner_json);
    let Ok(outer_json) = serde_json::to_string(&outer) else {
        return false;
    };
    template.form[f_req_index].1 = outer_json;
    template.raw_post_data = serialize_stream_generate_form_pairs(&template.form);
    true
}

pub(super) fn serialize_stream_generate_form_pairs(form: &[(String, String)]) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in form {
        serializer.append_pair(key, value);
    }
    serializer.finish()
}

fn upsert_stream_generate_query_param(
    query: &mut Vec<(String, String)>,
    key: &str,
    value: Option<&str>,
) {
    if let Some(existing) = query.iter_mut().find(|(name, _)| name == key) {
        if let Some(value) = value {
            existing.1 = value.to_string();
        }
    } else if let Some(value) = value {
        query.push((key.to_string(), value.to_string()));
    }
}

fn upsert_stream_generate_form_param(
    form: &mut Vec<(String, String)>,
    key: &str,
    value: Option<&str>,
) {
    if let Some(value) = value {
        if let Some(existing) = form.iter_mut().find(|(name, _)| name == key) {
            existing.1 = value.to_string();
            return;
        }
        form.push((key.to_string(), value.to_string()));
        return;
    }
    form.retain(|(name, _)| name != key);
}
