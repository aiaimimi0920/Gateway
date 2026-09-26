use serde_json::{json, Value};

use super::{
    defaults::*,
    prompt_for_text_request,
    request_ids::{current_reqid, random_hex_32},
    types::*,
};
use crate::error::GatewayError;
use crate::protocol::{canonical::CanonicalRelayRequest, gemini_web};

pub fn stream_generate_mode_index(operation: GeminiCanvasMediaOperation) -> i64 {
    match operation {
        GeminiCanvasMediaOperation::Image => GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX,
        GeminiCanvasMediaOperation::Music => GEMINI_CANVAS_STREAM_GENERATE_MUSIC_MODE_INDEX,
        GeminiCanvasMediaOperation::Video => GEMINI_CANVAS_STREAM_GENERATE_VIDEO_MODE_INDEX,
    }
}

pub fn media_operation_selection_preflight_mode_index(mode_index: i64) -> i64 {
    if mode_index == GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX
        || mode_index == GEMINI_CANVAS_STREAM_GENERATE_MUSIC_MODE_INDEX
    {
        // Captured image and music flows both still enter qpEbW through
        // selection mode 11 before StreamGenerate itself switches into their
        // final media mode.
        GEMINI_CANVAS_STREAM_GENERATE_VIDEO_MODE_INDEX
    } else {
        mode_index
    }
}

pub fn media_primary_ku4jyf_mode_index(mode_index: i64) -> i64 {
    if mode_index == GEMINI_CANVAS_STREAM_GENERATE_MUSIC_MODE_INDEX
        || mode_index == GEMINI_CANVAS_STREAM_GENERATE_VIDEO_MODE_INDEX
    {
        GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX
    } else {
        mode_index
    }
}

pub fn build_stream_generate_heavy_request(
    prompt: &str,
    bootstrap: &gemini_web::GeminiWebBootstrap,
    request_uuid: &str,
    mode_index: i64,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    build_stream_generate_heavy_request_with_uploaded_files_seeded(
        prompt,
        bootstrap,
        request_uuid,
        mode_index,
        &[],
        None,
    )
}

pub fn build_stream_generate_heavy_request_with_uploaded_files(
    prompt: &str,
    bootstrap: &gemini_web::GeminiWebBootstrap,
    request_uuid: &str,
    mode_index: i64,
    uploaded_files: &[GeminiCanvasUploadedFileRef],
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    build_stream_generate_heavy_request_with_uploaded_files_seeded(
        prompt,
        bootstrap,
        request_uuid,
        mode_index,
        uploaded_files,
        None,
    )
}

pub fn build_stream_generate_heavy_request_with_uploaded_files_seeded(
    prompt: &str,
    bootstrap: &gemini_web::GeminiWebBootstrap,
    request_uuid: &str,
    mode_index: i64,
    uploaded_files: &[GeminiCanvasUploadedFileRef],
    seed: Option<&GeminiCanvasStreamGenerateSeed>,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    let prompt = prompt.trim();
    if prompt.is_empty() {
        return Err(GatewayError::bad_request(
            "Gemini Canvas StreamGenerate requests require a prompt.",
        )
        .with_code("missing_gemini_canvas_stream_generate_prompt"));
    }
    let language = bootstrap.language.trim();
    let language = if language.is_empty() { "en" } else { language };
    let request_uuid = request_uuid.trim();
    if request_uuid.is_empty() {
        return Err(GatewayError::server_error(
            "Gemini Canvas direct HTTP StreamGenerate request UUID was empty.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_stream_generate_missing_request_uuid"));
    }

    let is_text_mode = mode_index == GEMINI_CANVAS_TEXT_STREAM_GENERATE_TEXT_MODE_INDEX;
    let is_image_mode = mode_index == GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX;
    let is_image_edit_mode = is_image_mode && !uploaded_files.is_empty();
    let seed_opaque_state = seed
        .and_then(|value| value.opaque_state.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let seed_request_hex = seed
        .and_then(|value| value.request_hex.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if !uploaded_files.is_empty() && !is_image_mode {
        return Err(GatewayError::server_error(
            "Gemini Canvas uploaded file refs are only valid for image StreamGenerate mode.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_invalid_uploaded_file_mode"));
    }

    let mut inner_req_list = vec![Value::Null; 80];
    inner_req_list[0] = if uploaded_files.is_empty() {
        json!([prompt, 0, null, null, null, null, 0])
    } else {
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
        json!([prompt, 0, null, attachments, null, null, 0])
    };
    inner_req_list[1] = json!([language]);
    inner_req_list[2] = json!(["", "", "", null, null, null, null, null, null, ""]);
    inner_req_list[3] = Value::String(
        if is_text_mode {
            GEMINI_CANVAS_TEXT_STREAM_GENERATE_OPAQUE_STATE
        } else if is_image_edit_mode {
            seed_opaque_state.unwrap_or(GEMINI_CANVAS_IMAGE_STREAM_GENERATE_OPAQUE_STATE)
        } else if is_image_mode {
            GEMINI_CANVAS_IMAGE_STREAM_GENERATE_OPAQUE_STATE
        } else {
            GEMINI_CANVAS_MEDIA_STREAM_GENERATE_OPAQUE_STATE
        }
        .to_string(),
    );
    inner_req_list[4] = Value::String(
        seed_request_hex
            .map(ToString::to_string)
            .unwrap_or_else(random_hex_32),
    );
    inner_req_list[6] = if is_text_mode {
        json!([0])
    } else if is_image_edit_mode {
        json!([1])
    } else if is_image_mode {
        json!([0])
    } else {
        json!([1])
    };
    inner_req_list[7] = Value::from(1);
    inner_req_list[10] = Value::from(1);
    inner_req_list[11] = Value::from(0);
    inner_req_list[17] = json!([[0]]);
    inner_req_list[18] = Value::from(0);
    inner_req_list[27] = Value::from(1);
    inner_req_list[30] = json!([4]);
    inner_req_list[41] = if is_text_mode { json!([2]) } else { json!([1]) };
    inner_req_list[49] = Value::from(mode_index);
    inner_req_list[53] = Value::from(0);
    inner_req_list[59] = Value::String(request_uuid.to_string());
    inner_req_list[61] = Value::Array(Vec::new());
    inner_req_list[68] = if is_text_mode {
        Value::from(1)
    } else if is_image_edit_mode {
        Value::from(2)
    } else if is_image_mode {
        Value::from(1)
    } else {
        Value::from(2)
    };
    inner_req_list[79] = Value::from(1);

    let inner_payload = serde_json::to_string(&inner_req_list).map_err(|error| {
        GatewayError::server_error(format!(
            "serialize Gemini Canvas StreamGenerate inner payload: {error}"
        ))
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_stream_generate_serialize_failed")
    })?;
    let f_req = serde_json::to_string(&vec![Value::Null, Value::String(inner_payload)]).map_err(
        |error| {
            GatewayError::server_error(format!(
                "serialize Gemini Canvas StreamGenerate outer payload: {error}"
            ))
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_stream_generate_serialize_failed")
        },
    )?;

    let mut query = vec![
        ("hl".to_string(), language.to_string()),
        ("_reqid".to_string(), current_reqid().to_string()),
        ("rt".to_string(), "c".to_string()),
    ];
    if let Some(build_label) = bootstrap
        .build_label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query.push(("bl".to_string(), build_label.to_string()));
    }
    if let Some(session_id) = bootstrap
        .session_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query.push(("f.sid".to_string(), session_id.to_string()));
    }

    let mut form = vec![("f.req".to_string(), f_req)];
    if !is_text_mode {
        if let Some(access_token) = bootstrap
            .access_token
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            form.insert(0, ("at".to_string(), access_token.to_string()));
        }
    }

    Ok(gemini_web::GeminiWebRequest { query, form })
}

pub fn build_text_stream_generate_heavy_request(
    req: &CanonicalRelayRequest,
    bootstrap: &gemini_web::GeminiWebBootstrap,
    request_uuid: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    let prompt = prompt_for_text_request(
        req,
        "Gemini Canvas requests require a prompt.",
        "missing_gemini_canvas_text_prompt",
    )?;
    build_stream_generate_heavy_request(
        &prompt,
        bootstrap,
        request_uuid,
        GEMINI_CANVAS_TEXT_STREAM_GENERATE_TEXT_MODE_INDEX,
    )
}
