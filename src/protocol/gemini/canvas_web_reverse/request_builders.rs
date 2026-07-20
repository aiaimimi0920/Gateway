use serde_json::{json, Value};

use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::gemini_api;

use super::{aspect_ratio_from_request, requested_output_count};

pub fn build_image_request_body(req: &CanonicalRelayRequest, model: &str) -> Value {
    let mut body = gemini_api::pack_gemini(req, model, false);
    rewrite_data_url_file_parts_to_inline_data(&mut body);
    strip_common_generation_fields(&mut body);
    if let Some(config) = body
        .as_object_mut()
        .map(|map| map.entry("generationConfig").or_insert_with(|| json!({})))
        .and_then(Value::as_object_mut)
    {
        config.insert("responseModalities".to_string(), json!(["IMAGE"]));
        config.entry("imageConfig").or_insert_with(|| json!({}));
        if let Some(image_config) = config.get_mut("imageConfig").and_then(Value::as_object_mut) {
            image_config.insert(
                "aspectRatio".to_string(),
                json!(aspect_ratio_from_request(req)),
            );
        }
    }
    body
}

pub fn build_direct_http_image_request_body(req: &CanonicalRelayRequest, model: &str) -> Value {
    let mut body = build_image_request_body(req, model);
    if let Some(config) = body
        .get_mut("generationConfig")
        .and_then(Value::as_object_mut)
    {
        config.insert("responseModalities".to_string(), json!(["TEXT", "IMAGE"]));
    }
    body
}

pub fn build_imagen_predict_request(req: &CanonicalRelayRequest, prompt: &str) -> Value {
    json!({
        "instances": [{
            "prompt": prompt
        }],
        "parameters": {
            "sampleCount": requested_output_count(req).clamp(1, 4),
            "aspectRatio": aspect_ratio_from_request(req)
        }
    })
}

pub fn build_music_client_content(prompt: &str) -> Value {
    json!({
        "weightedPrompts": [{
            "text": prompt,
            "weight": 1.0
        }]
    })
}

pub fn build_text_request_body(req: &CanonicalRelayRequest, model: &str) -> Value {
    let mut body = gemini_api::pack_gemini(req, model, false);
    strip_common_generation_fields(&mut body);
    body
}

pub fn build_tts_request_body(req: &CanonicalRelayRequest, model: &str) -> Value {
    let mut body = build_text_request_body(req, model);
    let request_voice = req
        .raw_body
        .get("voice")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let generation_config = body
        .as_object_mut()
        .map(|map| map.entry("generationConfig").or_insert_with(|| json!({})))
        .and_then(Value::as_object_mut);
    if let Some(config) = generation_config {
        config.insert("responseModalities".to_string(), json!(["AUDIO"]));
        if let Some(voice_name) = request_voice {
            config.insert(
                "speechConfig".to_string(),
                json!({
                    "voiceConfig": {
                        "prebuiltVoiceConfig": {
                            "voiceName": voice_name,
                        }
                    }
                }),
            );
        }
    } else if let Some(voice_name) = request_voice {
        body["generationConfig"] = json!({
            "responseModalities": ["AUDIO"],
            "speechConfig": {
                "voiceConfig": {
                    "prebuiltVoiceConfig": {
                        "voiceName": voice_name,
                    }
                }
            }
        });
    }
    body
}

fn strip_common_generation_fields(body: &mut Value) {
    if let Some(map) = body.as_object_mut() {
        map.remove("model");
        map.remove("stream");
        map.remove("response_format");
        map.remove("voice");
    }
}

fn parse_base64_data_url(data_url: &str) -> Option<(String, String)> {
    let trimmed = data_url.trim();
    if !trimmed.starts_with("data:") {
        return None;
    }
    let (meta, data) = trimmed.split_once(',')?;
    if !meta.to_ascii_lowercase().contains(";base64") {
        return None;
    }
    let mime_type = meta
        .trim_start_matches("data:")
        .split(';')
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let encoded = data.trim();
    if encoded.is_empty() {
        return None;
    }
    Some((mime_type.to_string(), encoded.to_string()))
}

fn rewrite_data_url_file_parts_to_inline_data(body: &mut Value) {
    let Some(contents) = body.get_mut("contents").and_then(Value::as_array_mut) else {
        return;
    };

    for content in contents {
        let Some(parts) = content.get_mut("parts").and_then(Value::as_array_mut) else {
            continue;
        };
        for part in parts {
            let Some(file_uri) = part
                .get("fileData")
                .or_else(|| part.get("file_data"))
                .and_then(|value| {
                    value
                        .get("fileUri")
                        .or_else(|| value.get("file_uri"))
                        .and_then(Value::as_str)
                })
            else {
                continue;
            };
            let Some((mime_type, data)) = parse_base64_data_url(file_uri) else {
                continue;
            };
            *part = json!({
                "inlineData": {
                    "mimeType": mime_type,
                    "data": data,
                }
            });
        }
    }
}
