use serde_json::{json, Value};
use url::Url;

use super::defaults::*;

pub fn harvest_text_batchexecute_header_id(storage_state: &Value) -> Option<String> {
    let template = storage_state
        .get("textStreamGenerateTemplate")
        .or_else(|| storage_state.get("streamGenerateTemplate"))?;
    let headers = template.get("headers")?.as_object()?;
    let header = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("x-goog-ext-525001261-jspb"))
        .and_then(|(_, value)| value.as_str())?
        .trim();
    if header.is_empty() {
        return None;
    }
    let parsed = serde_json::from_str::<Vec<Value>>(header).ok()?;
    parsed
        .get(16)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub fn harvest_batchexecute_header_id_for_mode(
    storage_state: &Value,
    mode_index: i64,
) -> Option<String> {
    let template = if mode_index == GEMINI_CANVAS_TEXT_STREAM_GENERATE_TEXT_MODE_INDEX {
        storage_state
            .get("textStreamGenerateTemplate")
            .or_else(|| storage_state.get("streamGenerateTemplate"))?
    } else {
        storage_state
            .get("imageStreamGenerateTemplate")
            .or_else(|| storage_state.get("mediaStreamGenerateTemplate"))
            .or_else(|| storage_state.get("textStreamGenerateTemplate"))
            .or_else(|| storage_state.get("streamGenerateTemplate"))?
    };
    let headers = template.get("headers")?.as_object()?;
    let header = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("x-goog-ext-525001261-jspb"))
        .and_then(|(_, value)| value.as_str())?
        .trim();
    if header.is_empty() {
        return None;
    }
    let parsed = serde_json::from_str::<Vec<Value>>(header).ok()?;
    parsed
        .get(16)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub fn harvest_image_edit_template_locale(storage_state: &Value) -> Option<String> {
    for key in [
        "imageEditStreamGenerateTemplate",
        "imageEditStreamTemplate",
        "imageStreamGenerateTemplate",
        "mediaStreamGenerateTemplate",
        "textStreamGenerateTemplate",
        "streamGenerateTemplate",
    ] {
        let Some(template) = storage_state.get(key).and_then(Value::as_object) else {
            continue;
        };
        if let Some(locale) = template
            .get("url")
            .and_then(Value::as_str)
            .and_then(|value| Url::parse(value).ok())
            .and_then(|url| {
                url.query_pairs()
                    .find(|(name, _)| name == "hl")
                    .map(|(_, value)| value.trim().to_string())
            })
            .filter(|value| !value.is_empty())
        {
            return Some(locale);
        }
        if let Some(locale) = template
            .get("headers")
            .and_then(Value::as_object)
            .and_then(|headers| {
                headers
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case("accept-language"))
                    .and_then(|(_, value)| value.as_str())
            })
            .and_then(|value| value.split(',').next())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
        {
            return Some(locale);
        }
    }
    None
}

pub fn build_stream_generate_model_header_from_storage_state(
    storage_state: &Value,
    mode_index: i64,
    prefer_image_edit_template: bool,
) -> Option<String> {
    let (template, image_specific_template) =
        if mode_index == GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX {
            if prefer_image_edit_template {
                if let Some(template) = storage_state.get("imageEditStreamGenerateTemplate") {
                    (template, true)
                } else if let Some(template) = storage_state.get("imageEditStreamTemplate") {
                    (template, true)
                } else if let Some(template) = storage_state.get("imageStreamGenerateTemplate") {
                    (template, true)
                } else if let Some(template) = storage_state.get("mediaStreamGenerateTemplate") {
                    (template, true)
                } else if let Some(template) = storage_state.get("textStreamGenerateTemplate") {
                    (template, false)
                } else {
                    (storage_state.get("streamGenerateTemplate")?, false)
                }
            } else if let Some(template) = storage_state.get("imageStreamGenerateTemplate") {
                (template, true)
            } else if let Some(template) = storage_state.get("mediaStreamGenerateTemplate") {
                (template, true)
            } else if let Some(template) = storage_state.get("textStreamGenerateTemplate") {
                (template, false)
            } else {
                (storage_state.get("streamGenerateTemplate")?, false)
            }
        } else {
            (
                storage_state
                    .get("textStreamGenerateTemplate")
                    .or_else(|| storage_state.get("streamGenerateTemplate"))?,
                false,
            )
        };
    let headers = template.get("headers")?.as_object()?;
    let header = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("x-goog-ext-525001261-jspb"))
        .and_then(|(_, value)| value.as_str())?
        .trim();
    if header.is_empty() {
        return None;
    }

    let mut parsed = serde_json::from_str::<Vec<Value>>(header).ok()?;
    if parsed.len() <= 11 {
        return None;
    }

    let is_text_mode = mode_index == GEMINI_CANVAS_TEXT_STREAM_GENERATE_TEXT_MODE_INDEX;
    if is_text_mode {
        parsed[4] = Value::String(GEMINI_CANVAS_TEXT_LAST_SELECTED_MODE_ID.to_string());
    } else {
        let current = parsed.get(4).and_then(Value::as_str).map(str::trim);
        if !image_specific_template || current.map(|value| value.is_empty()).unwrap_or(true) {
            parsed[4] = Value::String(GEMINI_CANVAS_TEXT_SELECTED_MODEL_HEADER_ID.to_string());
        }
    }
    parsed[11] = Value::from(if is_text_mode { 1 } else { 2 });
    serde_json::to_string(&parsed).ok()
}

pub fn build_text_batchexecute_model_header_variant(
    model_id: Option<&str>,
    header_id: Option<&str>,
    include_mode_flag: bool,
    force_empty_model_id: bool,
) -> String {
    let mut header = vec![
        Value::from(1),
        Value::Null,
        Value::Null,
        Value::Null,
        Value::Null,
        Value::Null,
        Value::Null,
        Value::Null,
        json!([4]),
        Value::Null,
        Value::Null,
        Value::Null,
        Value::Null,
        Value::Null,
        if include_mode_flag {
            Value::from(1)
        } else {
            Value::Null
        },
        Value::Null,
        Value::String(
            header_id
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(GEMINI_CANVAS_TEXT_PREFLIGHT_HEADER_ID_DEFAULT)
                .to_string(),
        ),
    ];
    if force_empty_model_id {
        header[4] = Value::String(String::new());
    } else if let Some(model_id) = model_id.map(str::trim).filter(|value| !value.is_empty()) {
        header[4] = Value::String(model_id.to_string());
    }
    serde_json::to_string(&header).unwrap_or_else(|_| {
        if model_id.is_some() {
            GEMINI_CANVAS_TEXT_BOOTSTRAP_MODEL_HEADER.to_string()
        } else {
            GEMINI_CANVAS_TEXT_MODE_SELECTION_MODEL_HEADER.to_string()
        }
    })
}

pub fn build_text_batchexecute_model_header(
    model_id: Option<&str>,
    header_id: Option<&str>,
) -> String {
    build_text_batchexecute_model_header_variant(model_id, header_id, true, false)
}
