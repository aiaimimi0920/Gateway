use serde_json::{json, Map, Value};

use super::{
    build_conversation_url, extract_clip_id, extract_image_prompt, extract_image_type,
    extract_prompt, extract_video_prompt, maybe_insert_bool, maybe_insert_number,
    maybe_insert_string, read_bool_fields, read_number_fields, read_string_fields,
};
use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;

pub fn build_send_message_request(
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<Value, GatewayError> {
    let body_obj = req.raw_body.as_object().ok_or_else(|| {
        GatewayError::bad_request("Producer.ai request body must be a JSON object")
    })?;

    let mut body = body_obj.clone();
    body.remove("model");
    body.remove("stream");
    body.remove("prompt");
    body.remove("input");
    body.remove("lyrics");
    body.remove("user");

    if !body.contains_key("parts") {
        let prompt = extract_prompt(body_obj).ok_or_else(|| {
            GatewayError::bad_request(
                "Producer.ai music requests require one of: prompt, input, lyrics, or parts[].content",
            )
            .with_code("missing_music_prompt")
        })?;
        body.insert(
            "parts".to_string(),
            Value::Array(vec![json!({
                "part_kind": "user-prompt",
                "content": prompt,
            })]),
        );
    }

    if !body.contains_key("client_context") {
        body.insert(
            "client_context".to_string(),
            json!({
                "current_song_id": null,
                "song_queue": [],
                "project_id": body_obj.get("project_id").cloned().unwrap_or(Value::Null),
                "selected_model": Value::String(model.to_string()),
                "lyrics_id_map": {},
                "ghostwriter_version": "standard",
            }),
        );
    }

    body.entry("model_name".to_string())
        .or_insert_with(|| Value::String(model.to_string()));
    body.entry("mode".to_string())
        .or_insert_with(|| Value::String("standard".to_string()));

    Ok(Value::Object(body))
}

pub fn build_send_message_plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<RequestPlan, GatewayError> {
    Ok(RequestPlan {
        method: rquest::Method::POST,
        url: build_conversation_url(&payload.base_url),
        query: Vec::new(),
        body: Some(build_send_message_request(req, model)?),
        response_kind: req.endpoint_kind,
    })
}

pub fn build_conversation_request_body(
    prompt: &str,
    conversation_id: Option<&str>,
    client_context: &Value,
    model_name: &str,
) -> Value {
    let mut body = json!({
        "parts": [{ "content": prompt, "part_kind": "user-prompt" }],
        "client_context": client_context,
        "model_name": model_name,
        "mode": "standard",
    });
    if let Some(conversation_id) = conversation_id {
        body["conversation_id"] = Value::String(conversation_id.to_string());
    }
    body
}

pub fn build_image_generation_request(
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<Value, GatewayError> {
    let body_obj = req.raw_body.as_object().ok_or_else(|| {
        GatewayError::bad_request("Producer.ai image request body must be a JSON object")
    })?;

    let prompt = extract_image_prompt(body_obj).ok_or_else(|| {
        GatewayError::bad_request("Producer.ai image requests require `prompt` or `input`.")
            .with_code("missing_image_prompt")
    })?;
    let image_type = extract_image_type(body_obj).unwrap_or_else(|| "clip".to_string());

    let mut body = body_obj.clone();
    body.remove("model");
    body.remove("stream");
    body.remove("response_format");
    body.remove("n");
    body.remove("user");
    body.remove("image_type");
    body.remove("imageType");
    body.remove("input");
    body.insert("prompt".to_string(), Value::String(prompt));
    body.insert("type".to_string(), Value::String(image_type));
    body.entry("model_name".to_string())
        .or_insert_with(|| Value::String(model.to_string()));

    Ok(Value::Object(body))
}

pub fn build_video_tool_call_request(
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<Value, GatewayError> {
    let body_obj = req.raw_body.as_object().ok_or_else(|| {
        GatewayError::bad_request("Producer.ai video request body must be a JSON object")
    })?;

    let conversation_id = read_string_fields(body_obj, &["conversation_id", "conversationId"]);
    let project_id = body_obj.get("project_id").cloned().unwrap_or(Value::Null);
    let clip_id = extract_clip_id(body_obj).ok_or_else(|| {
        GatewayError::bad_request(
            "Producer.ai music video requests require clip_id (or song_id/songId).",
        )
        .with_code("missing_video_clip_id")
    })?;
    let prompt = extract_video_prompt(body_obj);

    let mut args = body_obj
        .get("args")
        .and_then(|value| value.as_object())
        .cloned()
        .unwrap_or_default();

    args.entry("clip_id".to_string())
        .or_insert_with(|| Value::String(clip_id.clone()));

    if let Some(prompt) = prompt {
        args.entry("user_message".to_string())
            .or_insert_with(|| Value::String(prompt));
    }

    maybe_insert_string(
        &mut args,
        "style_image_url",
        read_string_fields(
            body_obj,
            &[
                "style_image_url",
                "styleImageUrl",
                "styleImage",
                "style_reference_image_url",
                "styleReferenceImageUrl",
            ],
        ),
    );
    maybe_insert_string(
        &mut args,
        "likeness_image_url",
        read_string_fields(
            body_obj,
            &[
                "likeness_image_url",
                "likenessImageUrl",
                "subject_image_url",
                "subjectImageUrl",
                "subject_image",
                "subjectImage",
            ],
        ),
    );
    maybe_insert_string(
        &mut args,
        "aspect_ratio",
        read_string_fields(body_obj, &["aspect_ratio", "aspectRatio", "size"]),
    );
    maybe_insert_string(
        &mut args,
        "resolution",
        read_string_fields(body_obj, &["resolution"]),
    );
    maybe_insert_bool(
        &mut args,
        "render_lyrics",
        read_bool_fields(
            body_obj,
            &[
                "render_lyrics",
                "display_lyrics",
                "renderLyrics",
                "displayLyrics",
            ],
        ),
    );
    maybe_insert_number(
        &mut args,
        "duration_s",
        read_number_fields(body_obj, &["duration_s", "durationSeconds", "duration"]),
    );

    args.entry("aspect_ratio".to_string())
        .or_insert_with(|| Value::String("9:16".to_string()));
    args.entry("resolution".to_string())
        .or_insert_with(|| Value::String("720p".to_string()));
    args.entry("render_lyrics".to_string())
        .or_insert(Value::Bool(true));

    let mut body = Map::new();
    if let Some(conversation_id) = conversation_id {
        body.insert(
            "conversation_id".to_string(),
            Value::String(conversation_id),
        );
    }
    body.insert(
        "part".to_string(),
        json!({
            "tool_name": "video__create_music_video",
            "args": Value::Object(args),
        }),
    );
    body.insert(
        "client_context".to_string(),
        body_obj.get("client_context").cloned().unwrap_or_else(|| {
            json!({
                "current_song_id": clip_id,
                "song_queue": [clip_id],
                "project_id": project_id,
                "selected_model": model,
                "lyrics_id_map": {},
                "ghostwriter_version": "standard",
            })
        }),
    );

    Ok(Value::Object(body))
}
