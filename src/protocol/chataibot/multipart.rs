use base64::Engine;

use crate::error::GatewayError;

use super::request::ChataibotUpload;
use super::{DEFAULT_CHAT_CONTEXT_ID, DEFAULT_FROM, DEFAULT_LANGUAGE};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultipartBody {
    pub content_type: String,
    pub bytes: Vec<u8>,
}

pub fn build_edit_multipart_body(
    prompt: &str,
    upload: &ChataibotUpload,
    mode: &str,
) -> Result<MultipartBody, GatewayError> {
    let boundary = format!("----neuro-chataibot-{}", uuid::Uuid::new_v4().simple());
    let mut body = Vec::new();

    push_text_part(&mut body, &boundary, "mode", mode);
    push_text_part(
        &mut body,
        &boundary,
        "chatContextId",
        DEFAULT_CHAT_CONTEXT_ID,
    );
    push_text_part(&mut body, &boundary, "lang", DEFAULT_LANGUAGE);
    push_text_part(&mut body, &boundary, "from", DEFAULT_FROM);
    push_text_part(&mut body, &boundary, "isInternational", "true");
    push_text_part(&mut body, &boundary, "caption", prompt);
    push_file_part(
        &mut body,
        &boundary,
        "images",
        &file_name_for_mime(&upload.mime_type),
        &upload.mime_type,
        &decode_upload_bytes(upload)?,
    );
    finish_multipart_body(&mut body, &boundary);

    Ok(MultipartBody {
        content_type: format!("multipart/form-data; boundary={boundary}"),
        bytes: body,
    })
}

pub fn build_merge_multipart_body(
    prompt: &str,
    uploads: &[ChataibotUpload],
    merge_type: &str,
) -> Result<MultipartBody, GatewayError> {
    let boundary = format!("----neuro-chataibot-{}", uuid::Uuid::new_v4().simple());
    let mut body = Vec::new();

    push_text_part(&mut body, &boundary, "type", merge_type);
    push_text_part(&mut body, &boundary, "lang", DEFAULT_LANGUAGE);
    push_text_part(&mut body, &boundary, "from", DEFAULT_FROM);
    push_text_part(&mut body, &boundary, "isInternational", "true");
    push_text_part(&mut body, &boundary, "caption", prompt);

    for (index, upload) in uploads.iter().enumerate() {
        push_file_part(
            &mut body,
            &boundary,
            "images",
            &format!(
                "upload_{index}.{}",
                file_extension_for_mime(&upload.mime_type)
            ),
            &upload.mime_type,
            &decode_upload_bytes(upload)?,
        );
    }

    finish_multipart_body(&mut body, &boundary);

    Ok(MultipartBody {
        content_type: format!("multipart/form-data; boundary={boundary}"),
        bytes: body,
    })
}

fn decode_upload_bytes(upload: &ChataibotUpload) -> Result<Vec<u8>, GatewayError> {
    base64::engine::general_purpose::STANDARD
        .decode(upload.base64_data.as_bytes())
        .map_err(|_| {
            GatewayError::bad_request("Chataibot image uploads must contain valid base64 data.")
                .with_code("invalid_image_upload_base64")
        })
}

fn push_text_part(body: &mut Vec<u8>, boundary: &str, name: &str, value: &str) {
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
    );
    body.extend_from_slice(value.as_bytes());
    body.extend_from_slice(b"\r\n");
}

fn push_file_part(
    body: &mut Vec<u8>,
    boundary: &str,
    name: &str,
    file_name: &str,
    mime_type: &str,
    bytes: &[u8],
) {
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"{name}\"; filename=\"{file_name}\"\r\n")
            .as_bytes(),
    );
    body.extend_from_slice(format!("Content-Type: {mime_type}\r\n\r\n").as_bytes());
    body.extend_from_slice(bytes);
    body.extend_from_slice(b"\r\n");
}

fn finish_multipart_body(body: &mut Vec<u8>, boundary: &str) {
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
}

fn file_name_for_mime(mime_type: &str) -> String {
    format!("upload.{}", file_extension_for_mime(mime_type))
}

fn file_extension_for_mime(mime_type: &str) -> &'static str {
    match mime_type {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => "png",
    }
}
